{
  module,
  overlay,
}: {
  config,
  pkgs,
  ...
}: let
  zoteroPort = 8080;
  autheliaPort = 9091;
  databasePort = 3306;
  streamPort = 8089;
  translationPort = 1969;
  portalPort = 3000;
  zoteroUri = "http://localhost:${toString zoteroPort}";
  autheliaUri = "https://localhost:${toString autheliaPort}";
  certificates = import "${pkgs.path}/nixos/tests/common/acme/server/snakeoil-certs.nix";
  localhostCertificate = pkgs.runCommand "zotero-development-localhost-certificate" {nativeBuildInputs = [pkgs.openssl];} ''
    mkdir "$out"
    openssl req -new -newkey rsa:2048 -nodes \
      -keyout "$out/key.pem" \
      -out request.pem \
      -subj /CN=localhost \
      -addext 'subjectAltName=DNS:localhost,IP:127.0.0.1'
    openssl x509 -req \
      -in request.pem \
      -CA ${certificates.ca.cert} \
      -CAkey ${certificates.ca.key} \
      -set_serial 1 \
      -days 36500 \
      -copy_extensions copyall \
      -out "$out/cert.pem"
  '';
in {
  imports = [
    module
    (import ./authelia.nix {
      address = "tcp://0.0.0.0:${toString autheliaPort}/";
      clientId = "zotero";
      redirectUri = "http://localhost:${toString portalPort}/auth/oidc/callback";
      session.cookies = [
        {
          domain = "localhost";
          authelia_url = autheliaUri;
        }
      ];
      signingKey = "${localhostCertificate}/key.pem";
      startBeforePortal = false;
      tls = {
        certificate = "${localhostCertificate}/cert.pem";
        key = "${localhostCertificate}/key.pem";
      };
    })
  ];

  nixpkgs.overlays = [overlay];

  services.zotero-dataserver = {
    enable = true;
    virtualHost = zoteroUri;
    portal.enable = false;
    stream-server.settings = {
      httpPort = streamPort;
      listenAddress = "0.0.0.0";
    };
    translation-server.settings = {
      host = "0.0.0.0";
      port = translationPort;
    };
  };

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

  systemd.services.zotero-dataserver-credentials.preStart = ''
    install -d -m 0700 "$STATE_DIRECTORY/credentials"
    printf '%s\n' development > "$STATE_DIRECTORY/credentials/dataserver-super-password"
    chmod 0600 "$STATE_DIRECTORY/credentials/dataserver-super-password"
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
        host.port = autheliaPort;
        guest.port = autheliaPort;
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
    autheliaPort
    databasePort
    streamPort
    translationPort
  ];

  users.users.development = {
    isNormalUser = true;
    initialPassword = "development";
    extraGroups = ["wheel"];
  };

  services.getty.autologinUser = "development";
  security.sudo.wheelNeedsPassword = false;

  security.pki.certificateFiles = [certificates.ca.cert];

  environment.systemPackages = [pkgs.curl];

  system.stateVersion = "26.11";
}
