{
  config,
  lib,
  pkgs,
  ...
}: let
  inherit (lib.meta) getExe;
  inherit (lib.modules) mkIf;
  inherit (lib.options) mkEnableOption mkPackageOption mkOption;
  inherit (lib.types) port;

  cfg = config.services.versitygw;
in {
  options.services.versitygw = {
    enable = mkEnableOption "versitygw";
    package = mkPackageOption pkgs "versitygw" {};

    port = mkOption {
      type = port;
      default = 10000;
    };
  };

  config = mkIf cfg.enable {
    systemd.services.versitygw = {
      description = "versity s3 gateway";
      wantedBy = ["multi-user.target"];

      serviceConfig = {
        ExecStart = "${getExe cfg.package} --access myaccess --secret mysecret --port 127.0.0.1:${toString cfg.port} posix $STATE_DIRECTORY";
        Restart = "always";
        RestartSec = 5;

        LimitNOFILE = 65536;
        TasksMax = "infinity";

        DynamicUser = true;
        StateDirectory = "versitygw";
      };
    };
  };

  _class = "nixos";
  _file = ./versitygw.nix;
}
