{
  config,
  lib,
  pkgs,
  ...
}: let
  inherit (lib.meta) getExe;
  inherit (lib.lists) optional;
  inherit (lib.modules) mkIf;
  inherit (lib.options) mkEnableOption mkOption mkPackageOption;
  inherit (lib.types) attrsOf bool int oneOf port str nullOr;

  json = pkgs.formats.json {};

  cfg = config.services.zotero-dataserver;
in {
  options.services.zotero-dataserver = {
    enable = mkEnableOption "zotero-dataserver";
    package = mkPackageOption pkgs "zotero-dataserver" {};

    phpfpm.settings = mkOption {
      type = attrsOf (oneOf [str int bool]);
      default = {
        "pm" = "dynamic";
        "pm.max_children" = 8;
        "pm.start_servers" = 2;
        "pm.min_spare_servers" = 2;
        "pm.max_spare_servers" = 4;
        "pm.max_requests" = 500;
      };
    };

    citeproc-js-server = {
      package = mkPackageOption pkgs "zotero-citeproc-js-server" {};
      settings = mkOption {
        inherit (json) type;
        default = {
          port = 8085;
        };
      };
    };
    stream-server = {
      package = mkPackageOption pkgs "zotero-stream-server" {};
      settings = mkOption {
        inherit (json) type;
        default = {
          httpPort = 8086;
        };
      };
    };
    tinymce-clean-server = {
      package = mkPackageOption pkgs "zotero-tinymce-clean-server" {};
      settings = {
        log = mkOption {
          type = bool;
          default = true;
        };
        host = mkOption {
          type = nullOr str;
          default = null;
        };
        port = mkOption {
          type = port;
          default = 16342;
        };
      };
    };
    translation-server = {
      package = mkPackageOption pkgs "zotero-translation-server" {};
      settings = mkOption {
        inherit (json) type;
        default = {
          port = 1969;
          host = "127.0.0.1";
        };
      };
    };
  };

  config = mkIf cfg.enable {
    users.groups.zotero-dataserver = {};
    users.users.zotero-dataserver = {group = "zotero-dataserver";};

    services.services.pools.zotero-dataserver = {
      phpPackage = cfg.package.php;
      settings = cfg.phpfpm.settings;
    };

    environment.etc = {
      "zotero-dataserver/citeproc-js-server.json".source = json.generate "citeproc-js-server-config.json" cfg.citeproc-js-server.settings;
      "zotero-dataserver/stream-server.json".source = json.generate "stream-server-config.json" cfg.stream-server.settings;
      "zotero-dataserver/translation-server.json".source = json.generate "translation-server-config.json" cfg.translation-server.settings;
    };

    systemd.services = {
      zotero-citeproc-js-server = {
        description = "Zotero citeproc-js-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        serviceConfig = {
          ExecStart = "${getExe cfg.citeproc-js-server.package}";
          Restart = "always";
          RestartSec = 5;
          Environment = [
            "NODE_ENV=citeproc-js-server"
            "NODE_CONFIG_DIR=/etc/zotero-dataserver:${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/config"
          ];
        };
      };
      zotero-stream-server = {
        description = "Zotero stream-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        serviceConfig = {
          ExecStart = "${getExe cfg.stream-server.package}";
          Restart = "always";
          RestartSec = 5;
          Environment = [
            "NODE_ENV=stream-server"
            "NODE_CONFIG_DIR=/etc/zotero-dataserver:${cfg.stream-server.package}/lib/node_modules/zotero-stream-server/config"
          ];
        };
      };
      zotero-tinymce-clean-server = {
        description = "Zotero tinymce-clean-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        serviceConfig = {
          ExecStart = "${getExe cfg.tinymce-clean-server.package}";
          Restart = "always";
          RestartSec = 5;
          ENVIRONMENT = let
            cfg' = cfg.tinymce-clean-server.settings;
          in
            ["TINYMCE_CLEAN_PORT=${cfg'.port}"]
            ++ optional (cfg'.log == true) "TINYMCE_CLEAN_LOG=1"
            ++ optional (cfg'.host != null) "TINYMCE_CLEAN_HOST=${cfg'.host}";
        };
      };
      zotero-translation-server = {
        description = "Zotero translation-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        serviceConfig = {
          ExecStart = "${getExe cfg.translation-server.package}";
          Restart = "always";
          RestartSec = 5;
          Environment = [
            "NODE_ENV=translation-server"
            "NODE_CONFIG_DIR=/etc/zotero-dataserver:${cfg.stream-server.package}/lib/node_modules/translation-server/config"
          ];
        };
      };
    };
  };
}
