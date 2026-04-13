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
  inherit (lib.types) attrs attrsOf bool int listOf oneOf port str nullOr;

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

    attachment-proxy = {
      package = mkPackageOption pkgs "zotero-attachment-proxy" {};
      settings = {
        logLevel = mkOption {
          type = str;
          default = "debug";
        };
        logFile = mkOption {
          type = str;
          default = "";
        };
        port = mkOption {
          type = port;
          default = 3000;
        };
        s3.bucket = mkOption {
          type = str;
          default = ""; # TODO: remove default
        };
        s3.region = mkOption {
          type = str;
          default = "us-east-1"; # TODO: remove default
        };
        # TODO: secretFile
        secret = mkOption {
          type = str;
          default = ""; # TODO: remove default
        };
        zipCacheTime = mkOption {
          type = int;
          default = 60;
        };
        zipMaxFiles = mkOption {
          type = int;
          default = 1000;
        };
        zipMaxFileSize = mkOption {
          type = int;
          default = 128 * 1024 * 1024;
        };
        tmpDir = mkOption {
          type = str;
          default = "./tmp/"; # TODO: /var/tmp/...
        };
        connectionTimeout = mkOption {
          type = int;
          default = 30;
        };
        trustedProxies = mkOption {
          type = listOf str;
          default = [];
        };
      };
    };
    citeproc-js-server = {
      package = mkPackageOption pkgs "zotero-citeproc-js-server" {};
      settings = {
        port = mkOption {
          type = port;
          default = 8085;
        };
        logLevel = mkOption {
          type = port;
          default = 8085;
        };
        engineCacheSize = mkOption {
          type = int;
          default = 20;
        };
        timings = mkOption {
          type = bool;
          default = false;
        };
        debug = mkOption {
          type = bool;
          default = true;
        };
        userAgent = mkOption {
          type = str;
          default = "Zotero Citation Server (https://github.com/zotero/citeproc-js-server)";
        };
        healthCheckFile = mkOption {
          type = str; # TODO: nullOr path?
          default = "";
        };
      };
    };
    stream-server = {
      package = mkPackageOption pkgs "zotero-stream-server" {};
      settings = {
        dev = mkOption {
          type = bool;
          default = false;
        };
        logLevel = mkOption {
          type = str;
          default = "info";
        };
        hostname = mkOption {
          type = str;
          default = "";
        };
        httpPort = mkOption {
          type = port;
          default = 8080;
        };
        proxyProtocol = mkOption {
          type = bool;
          default = false;
        };
        https = mkOption {
          type = bool;
          default = false;
        };
        trustedProxies = mkOption {
          type = listOf str;
          default = [];
        };
        statusInterval = mkOption {
          type = int;
          default = 10;
        };
        keepaliveInterval = mkOption {
          type = int;
          default = 25;
        };
        retryTime = mkOption {
          type = int;
          default = 10;
        };
        redis.host = mkOption {
          type = str;
          default = "";
        };
        redis.prefix = mkOption {
          type = str;
          default = "";
        };
        apiURL = mkOption {
          type = str;
          default = "";
        };
        apiVersion = mkOption {
          type = int;
          default = 3;
        };
        apiRequestHeaders = mkOption {
          type = attrs;
          default = {};
        };
        globalTopics = mkOption {
          type = listOf str;
          default = ["styles" "translators"];
        };
        continuedDelay = mkOption {
          type = int;
          default = 30 * 1000;
        };
        notContinuedDelay = mkOption {
          type = int;
          default = 250;
        };
        rateLimitRequests = mkOption {
          type = int;
          default = 10;
        };
        rateLimitWindow = mkOption {
          type = int;
          default = 60;
        };
        statsD.host = mkOption {
          type = str;
          default = "";
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
      settings = {
        allowedOrigins = mkOption {
          type = listOf str;
          default = [];
        };
        blacklistedDomains = mkOption {
          type = listOf str;
          default = [];
        };
        deproxifyURLs = mkOption {
          type = bool;
          default = false;
        };
        identifierSearchLambda = mkOption {
          type = str;
          default = "";
        };
        port = mkOption {
          type = port;
          default = 1969;
        };
        host = mkOption {
          type = str;
          default = "127.0.0.1";
        };
        httpMaxResponseSize = mkOption {
          type = int;
          default = 10;
        };
        textSearchTimeout = mkOption {
          type = int;
          default = 5;
        };
        translators.CrossrefREST.email = mkOption {
          type = str;
          default = "";
        };
        trustProxyHeaders = mkOption {
          type = bool;
          default = false;
        };
        userAgent = mkOption {
          type = str;
          default = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/110.0.0.0 Safari/537.36";
        };
        translatorsDirectory = mkOption {
          type = str;
          default = "./modules/translators";
        };
      };
    };
  };

  config = mkIf cfg.enable {
    users.groups.zotero-dataserver = {};
    users.users.zotero-dataserver = {
      isSystemUser = true;
      group = "zotero-dataserver";
    };

    services = {
      memcached.enable = true;
      redis.servers.zotero-dataserver = {
        enable = true;
        port = 6389;
      };
      # garage.enable = true; # TODO: use varsity instead?
      elasticsearch.enable = true;

      phpfpm.pools.zotero-dataserver = {
        user = "zotero-dataserver";
        group = "zotero-dataserver";
        phpPackage = cfg.package.php;
        settings = cfg.phpfpm.settings;
      };
    };

    environment.etc = {
      "zotero-dataserver/attachment-proxy.json".source = json.generate "attachment-proxy.json" cfg.attachment-proxy.settings;
      "zotero-dataserver/citeproc-js-server.json".source = json.generate "citeproc-js-server-config.json" (cfg.citeproc-js-server.settings
        // {
          localesPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl-locales";
          cslPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl";
          renamedStylesPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl/renamed-styles.json";
          cslDependentPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl/dependent";
        });
      "zotero-dataserver/stream-server.json".source = json.generate "stream-server-config.json" cfg.stream-server.settings;
      "zotero-dataserver/translation-server.json".source = json.generate "translation-server-config.json" cfg.translation-server.settings;
    };

    systemd.services = {
      zotero-attachment-proxy = {
        description = "Zotero attachment-proxy";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        serviceConfig = {
          ExecStart = "${getExe cfg.attachment-proxy.package}";
          Restart = "always";
          RestartSec = 5;
          Environment = [
            "NODE_ENV=attachment-proxy"
            "NODE_CONFIG_DIR=/etc/zotero-dataserver"
          ];
        };
      };
      zotero-citeproc-js-server = {
        description = "Zotero citeproc-js-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        serviceConfig = {
          ExecStart = "${getExe cfg.citeproc-js-server.package}";
          Restart = "always";
          RestartSec = 5;
          Environment = [
            "NODE_ENV=citeproc-js-server"
            "NODE_CONFIG_DIR=/etc/zotero-dataserver"
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
            "NODE_CONFIG_DIR=/etc/zotero-dataserver"
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
            ["TINYMCE_CLEAN_PORT=${toString cfg'.port}"]
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
            "NODE_CONFIG_DIR=/etc/zotero-dataserver"
          ];
        };
      };
    };
  };
}
