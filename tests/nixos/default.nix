{
  self,
  pkgs,
}: let
  inherit (builtins) readFile toJSON;

  inherit
    (self.packages.${pkgs.stdenv.hostPlatform.system})
    oidc-provider-mock
    versitygw
    zotero-attachment-proxy
    zotero-citeproc-js-server
    zotero-dataserver
    zotero-portal
    zotero-stream-server
    zotero-tinymce-clean-server
    zotero-translation-server
    zotero-web-library
    ;

  testCertificates = import "${pkgs.path}/nixos/tests/common/acme/server/snakeoil-certs.nix";
  testDomain = testCertificates.domain;
  testPython = pkgs.python3.withPackages (pythonPackages: [
    pythonPackages.websocket-client
  ]);
  streamClient = pkgs.writeText "stream-client.py" (readFile ./stream-client.py);
  oidcClientSecret = pkgs.writeText "OIDC_CLIENT_SECRET" "development";
  oidcIssuer = "http://127.0.0.1:19090";
in {
  nixos-dataserver = pkgs.testers.runNixOSTest {
    name = "zotero-dataserver";

    nodes.machine = {
      imports = [
        self.nixosModules.default
        (import ../../dev/oidc-provider-mock.nix {
          inherit oidc-provider-mock;
          port = 19090;
        })
      ];

      virtualisation.memorySize = 4 * 1024;

      environment.systemPackages = [testPython];

      services.zotero-dataserver = {
        enable = true;
        package = zotero-dataserver;
        virtualHost = testDomain;

        secrets.oidcClientSecretFile = oidcClientSecret;

        portal = {
          package = zotero-portal;
          config.oidc = {
            issuer = oidcIssuer;
            client_id = "zotero-test";
          };
          logging = {
            filter = "zotero_portal=debug,tower_http=info";
          };
        };
        web-library.package = zotero-web-library;

        attachment-proxy = {
          package = zotero-attachment-proxy;
          settings.port = 13000;
        };
        citeproc-js-server = {
          package = zotero-citeproc-js-server;
          settings.port = 18085;
        };
        stream-server = {
          package = zotero-stream-server;
          settings.httpPort = 18080;
        };
        tinymce-clean-server = {
          package = zotero-tinymce-clean-server;
          settings.port = 16343;
        };
        translation-server = {
          package = zotero-translation-server;
          settings.port = 11969;
        };
      };

      services = {
        caddy = {
          virtualHosts.${testDomain} = {
            extraConfig = ''
              tls ${testCertificates.${testDomain}.cert} ${testCertificates.${testDomain}.key}
            '';
          };
        };
        memcached.port = 11212;
        mysql.settings.mysqld.port = 13336;
        opensearch.settings."http.port" = 19200;
        redis.servers.zotero-dataserver.port = 16389;
        versitygw = {
          package = versitygw;
          port = 11000;
        };
      };

      systemd.services = {
        versitygw.preStart = pkgs.lib.mkBefore ''
          retry_marker=/tmp/versitygw-failed-once
          if ! test -e "$retry_marker"; then
            touch "$retry_marker"
            exit 1
          fi
        '';
        zotero-stream-server.environment.NODE_EXTRA_CA_CERTS = toString testCertificates.ca.cert;
      };

      networking.hosts."127.0.0.1" = [testDomain];
      security.pki.certificateFiles = [testCertificates.ca.cert];
    };

    testScript =
      readFile ./dataserver.py
      + ''
        start_all()
        run(
            machine,
            test_domain=${toJSON testDomain},
            oidc_issuer=${toJSON oidcIssuer},
            stream_client_path=${toJSON (toString streamClient)},
        )
      '';
  };

  nixos-versitygw = pkgs.testers.runNixOSTest {
    name = "versitygw";

    nodes.machine = {
      imports = [self.nixosModules.default];

      systemd.tmpfiles.rules = [
        "d /srv/versitygw-test 0750 versitygw versitygw -"
      ];

      services.versitygw = {
        enable = true;
        package = versitygw;
        dataDir = "/srv/versitygw-test";
        metadataSidecarDir = "/var/lib/versitygw-sidecar";
        disableOTmpfile = true;
        disableCopyFileRange = true;
        rootAccessKeyIdFile = pkgs.writeText "versitygw-root-access-key-id" "myaccess";
        rootSecretAccessKeyFile = pkgs.writeText "versitygw-root-secret-access-key" "mysecret";
      };
    };

    testScript =
      readFile ./versitygw.py
      + ''
        start_all()
        run(machine)
      '';
  };
}
