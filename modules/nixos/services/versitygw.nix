{
  config,
  lib,
  pkgs,
  ...
}: let
  inherit (lib.attrsets) optionalAttrs;
  inherit (lib.lists) optional;
  inherit (lib.meta) getExe;
  inherit (lib.modules) mkIf;
  inherit (lib.options) mkEnableOption mkPackageOption mkOption;
  inherit (lib.strings) escapeShellArg escapeShellArgs;
  inherit (lib.types) bool int listOf nullOr oneOf path port str;

  cfg = config.services.versitygw;

  posixArgs =
    optional cfg.disableOTmpfile "--disableotmp"
    ++ optional cfg.disableCopyFileRange "--disable-copy-file-range"
    ++ optional (cfg.metadataSidecarDir != null) "--sidecar"
    ++ optional (cfg.metadataSidecarDir != null) (toString cfg.metadataSidecarDir)
    ++ ["--concurrency" (toString cfg.concurrency) (toString cfg.dataDir)];
in {
  options.services.versitygw = {
    enable = mkEnableOption "versitygw";
    package = mkPackageOption pkgs "versitygw" {};

    address = mkOption {
      type = str;
      default = "127.0.0.1";
      description = "Address on which the S3 gateway listens.";
    };

    port = mkOption {
      type = port;
      default = 10000;
      description = "Port on which the S3 gateway listens.";
    };

    dataDir = mkOption {
      type = path;
      default = "/var/lib/versitygw";
      description = ''
        POSIX gateway root. This may be a network filesystem mount, provided
        that it supports extended attributes or metadataSidecarDir is set.
      '';
    };

    buckets = mkOption {
      type = listOf str;
      default = [];
      description = "Buckets to create as directories beneath dataDir.";
    };

    metadataSidecarDir = mkOption {
      type = nullOr path;
      default = null;
      description = ''
        Separate directory for object metadata when dataDir does not support
        extended attributes.
      '';
    };

    disableOTmpfile = mkOption {
      type = bool;
      default = false;
      description = "Disable O_TMPFILE for filesystems that do not support it.";
    };

    disableCopyFileRange = mkOption {
      type = bool;
      default = false;
      description = ''
        Disable copy_file_range when completing multipart uploads. This may be
        necessary for NFS servers on which copy_file_range hangs.
      '';
    };

    concurrency = mkOption {
      type = int;
      default = 5000;
      description = "Maximum number of concurrent POSIX backend operations.";
    };

    rootAccessKeyIdFile = mkOption {
      type = oneOf [path str];
      example = "/run/secrets/versitygw-root-access-key-id";
      description = "File containing the root access key ID. systemd specifiers such as %S are supported.";
    };

    rootSecretAccessKeyFile = mkOption {
      type = oneOf [path str];
      example = "/run/secrets/versitygw-root-secret-access-key";
      description = "File containing the root secret access key. systemd specifiers such as %S are supported.";
    };
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = builtins.all (bucket: builtins.match "[a-z0-9][a-z0-9.-]*[a-z0-9]" bucket != null) cfg.buckets;
        message = "services.versitygw.buckets must contain valid S3 bucket names";
      }
    ];

    users.groups.versitygw = {};
    users.users.versitygw = {
      isSystemUser = true;
      group = "versitygw";
    };

    systemd.services.versitygw = {
      description = "versity s3 gateway";
      wantedBy = ["multi-user.target"];
      unitConfig.RequiresMountsFor =
        [cfg.dataDir]
        ++ optional (cfg.metadataSidecarDir != null) cfg.metadataSidecarDir;

      preStart = ''
        ${builtins.concatStringsSep "\n" (map (bucket: "mkdir -p ${escapeShellArg "${cfg.dataDir}/${bucket}"}") cfg.buckets)}
      '';

      script = ''
        export ROOT_ACCESS_KEY_ID="$(< "$CREDENTIALS_DIRECTORY/root-access-key-id")"
        export ROOT_SECRET_ACCESS_KEY="$(< "$CREDENTIALS_DIRECTORY/root-secret-access-key")"

        exec ${getExe cfg.package} \
          --port ${escapeShellArg "${cfg.address}:${toString cfg.port}"} \
          --health /health \
          posix ${escapeShellArgs posixArgs}
      '';

      serviceConfig =
        {
          LoadCredential = [
            "root-access-key-id:${cfg.rootAccessKeyIdFile}"
            "root-secret-access-key:${cfg.rootSecretAccessKeyFile}"
          ];
          Restart = "always";
          RestartSec = 5;

          LimitNOFILE = 65536;
          TasksMax = "infinity";

          User = "versitygw";
          Group = "versitygw";
        }
        // optionalAttrs (cfg.dataDir == "/var/lib/versitygw") {
          StateDirectory = "versitygw";
        };
    };
  };

  _class = "nixos";
  _file = ./versitygw.nix;
}
