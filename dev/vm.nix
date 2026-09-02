self: {
  config,
  lib,
  pkgs,
  ...
}: let
  zoteroPort = 8080;
  oidcPort = 9091;
  databasePort = 3306;
  streamPort = 8089;
  translationPort = 1969;
  zoteroUri = "http://localhost:${toString zoteroPort}";
in {
  imports = [
    self.nixosModules.default
    (import ./oidc-provider-mock.nix {
      inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) oidc-provider-mock;
      port = oidcPort;
    })
  ];

  nixpkgs.overlays = [self.overlays.default];

  services.zotero-dataserver = {
    enable = true;
    virtualHost = zoteroUri;
    portal.enable = false;
    stream-server.settings = {
      httpPort = streamPort;
      listenAddress = "0.0.0.0";
      statusInterval = 10;
    };
    translation-server.settings = {
      host = "0.0.0.0";
      port = translationPort;
    };
  };

  services.phpfpm.pools.zotero-dataserver.settings = {
    "decorate_workers_output" = false;
    "php_admin_flag[log_errors]" = true;
    "php_admin_value[error_log]" = "syslog";
    "php_admin_value[error_reporting]" = "E_ALL";
    "php_admin_value[syslog.ident]" = "zotero-dataserver";
  };

  services.caddy.virtualHosts.${zoteroUri}.extraConfig = lib.mkAfter ''
    reverse_proxy 10.0.2.2:3000
  '';

  services.mysql.settings.mysqld = {
    "bind-address" = "0.0.0.0";
    port = databasePort;
  };

  systemd.services.zotero-development-database-access = {
    description = "Create the Zotero development database user";
    wantedBy = ["multi-user.target"];
    requires = ["mysql.service"];
    after = ["mysql.service"];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      ${config.services.mysql.package}/bin/mariadb --execute="
        CREATE USER IF NOT EXISTS 'development'@'%' IDENTIFIED BY 'development';
        ALTER USER 'development'@'%' IDENTIFIED BY 'development';
        GRANT ALL PRIVILEGES ON zotero_ids.* TO 'development'@'%';
        GRANT ALL PRIVILEGES ON zotero_master.* TO 'development'@'%';
        GRANT ALL PRIVILEGES ON zotero_shard.* TO 'development'@'%';
        GRANT ALL PRIVILEGES ON zotero_www.* TO 'development'@'%';
      "
    '';
  };

  systemd.services.zotero-dataserver-provision.preStart = ''
    printf '%s\n' development > "$STATE_DIRECTORY/DATASERVER_SUPER_PASSWORD"
    chmod 0600 "$STATE_DIRECTORY/DATASERVER_SUPER_PASSWORD"
  '';

  virtualisation.vmVariant.virtualisation = {
    cores = 4;
    graphics = false;
    memorySize = 6 * 1024;
    forwardPorts = [
      {
        from = "host";
        host.port = zoteroPort;
        guest.port = zoteroPort;
      }
      {
        from = "host";
        host.port = oidcPort;
        guest.port = oidcPort;
      }
      {
        from = "host";
        host.port = streamPort;
        guest.port = streamPort;
      }
      {
        from = "host";
        host.port = translationPort;
        guest.port = translationPort;
      }
      {
        from = "host";
        host.port = databasePort;
        guest.port = databasePort;
      }
    ];
  };

  networking.firewall.allowedTCPPorts = [
    zoteroPort
    oidcPort
    databasePort
    streamPort
    translationPort
  ];

  users.users.nixos = {
    isNormalUser = true;
    initialPassword = "nixos";
    extraGroups = ["wheel"];
  };

  services.getty.autologinUser = "nixos";
  security.sudo.wheelNeedsPassword = false;

  environment.systemPackages = [pkgs.curl];

  system.stateVersion = "26.11";
}
