{
  self,
  pkgs,
}: let
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

        portal = {
          package = zotero-portal;
          clientSecretFile = oidcClientSecret;
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

    testScript = ''
      import base64
      import hashlib
      import hmac
      import json
      import re
      import shlex
      import time
      import tomllib
      import urllib.parse

      start_all()
      public_url = "https://${testDomain}"
      api_url = public_url + "/api"
      opensearch_port = 19200

      for unit in [
          "zotero.target",
          "zotero-dataserver-provision.service",
          "memcached.service",
          "redis-zotero-dataserver.service",
          "mysql.service",
          "opensearch.service",
          "versitygw.service",
          "oidc-provider-mock.service",
          "zotero-fulltext-index.service",
          "zotero-portal.service",
          "phpfpm-zotero-dataserver.service",
          "zotero-attachment-proxy.service",
          "zotero-citeproc-js-server.service",
          "zotero-stream-server.service",
          "zotero-tinymce-clean-server.service",
          "zotero-translation-server.service",
          "caddy.service",
      ]:
          machine.wait_for_unit(unit)

      for port in [
          443,
          opensearch_port,
          11000,
          11212,
          11969,
          13000,
          16343,
          16389,
          18080,
          18085,
          19090,
      ]:
          machine.wait_for_open_port(port)

      fulltext_index = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              f"http://127.0.0.1:{opensearch_port}/item_fulltext_index"
          )
      )["item_fulltext_index"]
      assert fulltext_index["settings"]["index"]["number_of_shards"] == "1"
      assert fulltext_index["settings"]["index"]["number_of_replicas"] == "0"
      assert fulltext_index["mappings"]["_routing"]["required"] is True
      assert fulltext_index["mappings"]["properties"]["libraryID"]["type"] == "integer"
      assert fulltext_index["mappings"]["properties"]["content"]["type"] == "text"
      assert machine.succeed(
          "ls -1 /var/lib/opensearch/plugins"
      ).splitlines() == ["opensearch-security"]
      opensearch_nodes = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              f"http://127.0.0.1:{opensearch_port}/_nodes/_local/jvm"
          )
      )["nodes"]
      opensearch_heap = next(iter(opensearch_nodes.values()))["jvm"]["mem"]
      assert opensearch_heap["heap_init_in_bytes"] == 128 * 1024 * 1024
      assert opensearch_heap["heap_max_in_bytes"] == 512 * 1024 * 1024

      failed_units = machine.succeed(
          "systemctl list-units --state=failed --no-legend --plain"
      )
      assert failed_units.strip() == "", failed_units
      assert machine.succeed(
          "systemctl show -P User phpfpm-zotero-dataserver.service"
      ).strip() == "zotero-dataserver"
      assert machine.succeed(
          "systemctl show -P Group zotero-portal.service"
      ).strip() == "zotero-portal"
      assert "zotero-portal" in machine.succeed("id -nG caddy").split()
      assert machine.succeed(
          "stat -c %a /run/zotero-portal/zotero-portal.sock"
      ).strip() == "660"
      web_library_css = machine.succeed(
          "curl --silent --show-error --fail-with-body "
          "https://${testDomain}/static/web-library/zotero-web-library.css"
      )
      assert web_library_css.startswith('@charset "UTF-8";'), web_library_css[:200]
      zotero_target_wants = machine.succeed(
          "systemctl show -P Wants zotero.target"
      ).split()
      assert set(zotero_target_wants) == {
          "zotero-dataserver-provision.service",
          "zotero-dataserver-schema.service",
          "zotero-fulltext-index.service",
          "zotero-portal.service",
          "zotero-attachment-proxy.service",
          "zotero-citeproc-js-server.service",
          "zotero-stream-server.service",
          "zotero-tinymce-clean-server.service",
          "zotero-translation-server.service",
      }, zotero_target_wants

      portal_wants = machine.succeed(
          "systemctl show -P Wants zotero-portal.service"
      ).split()
      for unit in [
          "mysql.service",
          "zotero-translation-server.service",
          "caddy.service",
          "phpfpm-zotero-dataserver.service",
      ]:
          assert unit in portal_wants, (unit, portal_wants)

      phpfpm_wants = machine.succeed(
          "systemctl show -P Wants phpfpm-zotero-dataserver.service"
      ).split()
      for unit in [
          "zotero-dataserver-provision.service",
          "mysql.service",
          "redis-zotero-dataserver.service",
          "memcached.service",
          "versitygw.service",
          "zotero-attachment-proxy.service",
          "zotero-citeproc-js-server.service",
          "zotero-stream-server.service",
          "zotero-tinymce-clean-server.service",
          "zotero-translation-server.service",
      ]:
          assert unit in phpfpm_wants, (unit, phpfpm_wants)

      phpfpm_requires = machine.succeed(
          "systemctl show -P Requires phpfpm-zotero-dataserver.service"
      ).split()
      for unit in [
          "opensearch.service",
          "zotero-fulltext-index.service",
      ]:
          assert unit in phpfpm_requires, (unit, phpfpm_requires)

      for unit in [
          "zotero-dataserver-provision.service",
          "zotero-dataserver-schema.service",
          "zotero-fulltext-index.service",
      ]:
          assert machine.succeed(
              f"systemctl show -P Restart {unit}"
          ).strip() == "on-failure"

      for unit in [
          "zotero-portal.service",
          "phpfpm-zotero-dataserver.service",
          "versitygw.service",
          "zotero-attachment-proxy.service",
          "zotero-citeproc-js-server.service",
          "zotero-stream-server.service",
          "zotero-tinymce-clean-server.service",
          "zotero-translation-server.service",
      ]:
          assert machine.succeed(
              f"systemctl show -P Restart {unit}"
          ).strip() == "always"

      attachment_proxy_wants = machine.succeed(
          "systemctl show -P Wants zotero-attachment-proxy.service"
      ).split()
      attachment_proxy_requires = machine.succeed(
          "systemctl show -P Requires zotero-attachment-proxy.service"
      ).split()
      assert "versitygw.service" in attachment_proxy_wants
      assert "versitygw.service" not in attachment_proxy_requires

      portal_config = tomllib.loads(
          machine.succeed("cat /etc/zotero-dataserver/portal.toml")
      )
      assert portal_config == {
          "server_address": "unix:///run/zotero-portal/zotero-portal.sock",
          "oidc": {
              "issuer": "${oidcIssuer}",
              "client_id": "zotero-test",
              "redirect_url": public_url + "/auth/oidc/callback",
          },
          "dataserver_url": api_url + "/",
          "stream_server_url": public_url.replace("https://", "wss://") + "/stream/",
          "translation_server_url": "http://127.0.0.1:11969",
      }, portal_config
      portal_environment = machine.succeed(
          "systemctl show -P Environment zotero-portal.service"
      )
      assert "RUST_LOG=zotero_portal=debug,tower_http=info" in portal_environment
      provision_environment = machine.succeed(
          "systemctl show -P Environment zotero-dataserver-provision.service"
      )
      assert "RUST_LOG=zotero_portal=debug,tower_http=info" in provision_environment

      databases = machine.succeed(
          "mariadb --batch --skip-column-names "
          "--execute='SHOW DATABASES LIKE \"zotero_%\"'"
      ).splitlines()
      assert databases == [
          "zotero_ids",
          "zotero_master",
          "zotero_shard",
          "zotero_www",
      ], databases

      item_types = machine.succeed(
          "mariadb --batch --skip-column-names zotero_master "
          "--execute='SELECT COUNT(*) FROM itemTypes'"
      ).strip()
      assert int(item_types) > 0, item_types

      triggers = machine.succeed(
          "mariadb --batch --skip-column-names information_schema "
          "--execute=\"SELECT COUNT(*) FROM TRIGGERS "
          "WHERE TRIGGER_SCHEMA = 'zotero_shard'\""
      ).strip()
      assert int(triggers) > 0, triggers

      www_tables = set(
          machine.succeed(
              "mariadb --batch --skip-column-names information_schema "
              "--execute=\"SELECT TABLE_NAME FROM TABLES "
              "WHERE TABLE_SCHEMA = 'zotero_www'\""
          ).splitlines()
      )
      assert www_tables == {
          "_sqlx_migrations",
          "GDN_User",
          "browser_sessions",
          "group_invitations",
          "oidc_identities",
          "sessions",
          "storage_institution_email",
          "storage_institutions",
          "users",
          "users_email",
          "users_meta",
      }, www_tables

      machine.succeed(
          "mariadb zotero_www "
          "--execute=\"INSERT INTO users "
          "(userID, username, password, email) VALUES "
          "(1, 'testuser', MD5('letmein'), 'test@example.com'); "
          "INSERT INTO users_email (userID, email, validated) VALUES "
          "(1, 'secondary@example.com', 1); "
          "INSERT INTO users_meta VALUES "
          "(1, 'profile_realname', 'Test User'), "
          "(1, 'privacy_publishLibrary', '1'); "
          "INSERT INTO GDN_User VALUES (1, 0); "
          "INSERT INTO sessions VALUES "
          "('test-session', 1, UNIX_TIMESTAMP(), 3600); "
          "INSERT INTO storage_institutions "
          "(institutionID, storageQuota, domain) VALUES "
          "(1, 2048, 'example.com'); "
          "INSERT INTO storage_institution_email VALUES "
          "(1, 'secondary@example.com')\""
      )

      def create_login_session():
          return json.loads(
              machine.succeed(
                  "curl --silent --show-error --fail-with-body "
                  "--request POST --header "
                  "'User-Agent: Mozilla/5.0 (Linux) Zotero/7.0' "
                  + shlex.quote(api_url + "/keys/sessions")
              )
          )

      def login_session_status(token):
          return json.loads(
              machine.succeed(
                  "curl --silent --show-error --fail-with-body "
                  + shlex.quote(f"{api_url}/keys/sessions/{token}")
              )
          )

      def login_start_url(session):
          return (
              public_url + "/user/login?"
              + urllib.parse.urlencode({"session": session["sessionToken"]})
          )

      def oidc_authorization_url(session, cookie_file):
          headers = machine.succeed(
              "curl --silent --show-error --dump-header - "
              "--data= --output /dev/null --cookie "
              + shlex.quote(cookie_file)
              + " --cookie-jar "
              + shlex.quote(cookie_file)
              + " "
              + shlex.quote(login_start_url(session))
          )
          location = next(
              line.split(":", 1)[1].strip()
              for line in headers.splitlines()
              if line.lower().startswith("location:")
          )
          assert location.startswith("${oidcIssuer}/oauth2/authorize?"), location
          return location

      login_page_session = create_login_session()
      register_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null "
          + shlex.quote(public_url + "/user/register")
      ).lower()
      assert register_headers.startswith("http/2 307"), register_headers
      assert "location: /user/login" in register_headers, register_headers
      login_page = machine.succeed(
          "curl --silent --show-error --fail-with-body "
          + shlex.quote(login_page_session["loginURL"])
      )
      assert "Sign in to Zotero" in login_page, login_page
      assert "Continue with OpenID Connect" in login_page, login_page
      assert "Use your OpenID Connect account to continue" not in login_page, login_page
      assert 'aria-label="Primary navigation"' in login_page, login_page
      assert (
          f'action="/user/login?session={login_page_session["sessionToken"]}"'
          in login_page
      ), login_page

      parallel_sessions = [create_login_session(), create_login_session()]
      parallel_commands = []
      for index, session in enumerate(parallel_sessions):
          cookie_file = f"/tmp/oidc-parallel-{index}.cookies"
          authorization_url = oidc_authorization_url(session, cookie_file)
          parallel_commands.append(
              "curl --silent --show-error --fail-with-body "
              "--data-urlencode sub=alice --location --cookie "
              + shlex.quote(cookie_file)
              + " --cookie-jar "
              + shlex.quote(cookie_file)
              + " "
              + shlex.quote(authorization_url)
              + " >"
              + shlex.quote(f"/tmp/oidc-parallel-{index}.html")
          )
      machine.succeed(
          "(" + parallel_commands[0] + ") & "
          "(" + parallel_commands[1] + ") & wait"
      )
      parallel_statuses = [
          login_session_status(session["sessionToken"])
          for session in parallel_sessions
      ]
      if not all(status["status"] == "completed" for status in parallel_statuses):
          print(machine.succeed("sed -n '1,120p' /tmp/oidc-parallel-0.html"))
          print(machine.succeed("sed -n '1,120p' /tmp/oidc-parallel-1.html"))
          print(machine.succeed("journalctl -u zotero-portal -u oidc-provider-mock --no-pager -n 200"))
      assert all(status["status"] == "completed" for status in parallel_statuses), parallel_statuses
      assert parallel_statuses[0]["userID"] == parallel_statuses[1]["userID"], parallel_statuses
      oidc_user_id = parallel_statuses[0]["userID"]
      oidc_storage = machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT quota, expiration IS NULL FROM storageAccounts "
              f"WHERE userID = {oidc_user_id}"
          )
      ).strip().split("\t")
      assert oidc_storage == ["0", "1"], oidc_storage
      oidc_counts = machine.succeed(
          "mariadb --batch --skip-column-names zotero_www "
          "--execute=\"SELECT "
          "(SELECT COUNT(*) FROM users WHERE email = 'alice@example.com'), "
          "(SELECT COUNT(*) FROM oidc_identities "
          "WHERE issuer = '${oidcIssuer}')\""
      ).strip().split("\t")
      assert oidc_counts == ["1", "1"], oidc_counts
      oidc_account = machine.succeed(
          "mariadb --batch --skip-column-names zotero_www "
          + "--execute="
          + shlex.quote(
              "SELECT users.role, GDN_User.Banned FROM users "
              "JOIN GDN_User ON GDN_User.UserID = users.userID "
              f"WHERE users.userID = {oidc_user_id}"
          )
      ).strip().split("\t")
      assert oidc_account == ["member", "0"], oidc_account

      repeated_session = create_login_session()
      cookie_file = "/tmp/oidc-repeated.cookies"
      machine.succeed(
          "mariadb zotero_master --execute="
          + shlex.quote(
              "UPDATE storageAccounts SET quota = 1 "
              f"WHERE userID = {oidc_user_id}"
          )
      )

      def browser_session_cookie():
          return urllib.parse.unquote(
              machine.succeed(
                  "awk '$6 == \"__Host-session\" {print $7}' "
                  + shlex.quote(cookie_file)
              ).strip()
          )

      first_headers = machine.succeed(
          "curl --silent --show-error --dump-header - "
          "--data= --output /dev/null --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(login_start_url(repeated_session))
      )
      first_location = next(
          line.split(":", 1)[1].strip()
          for line in first_headers.splitlines()
          if line.lower().startswith("location:")
      )
      second_headers = machine.succeed(
          "curl --silent --show-error --dump-header - "
          "--data= --output /dev/null --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(login_start_url(repeated_session))
      )
      second_location = next(
          line.split(":", 1)[1].strip()
          for line in second_headers.splitlines()
          if line.lower().startswith("location:")
      )
      assert first_location != second_location, (first_location, second_location)
      repeated_result = machine.succeed(
          "curl --silent --show-error --fail-with-body "
          "--data-urlencode sub=alice --location --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(second_location)
      )
      assert "Login complete" in repeated_result, repeated_result
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT quota FROM storageAccounts "
              f"WHERE userID = {oidc_user_id}"
          )
      ).strip() == "0"
      durable_cookie = browser_session_cookie()
      assert durable_cookie
      durable_hash = hashlib.sha256(
          base64.urlsafe_b64decode(durable_cookie)
      ).hexdigest().upper()
      durable_row = machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT userID, CAST(UNIX_TIMESTAMP(expiresAt) AS UNSIGNED) "
              "FROM browser_sessions "
              f"WHERE tokenHash = UNHEX('{durable_hash}')"
          )
      ).strip().split("\t")
      assert durable_row[0] == str(oidc_user_id), durable_row
      assert int(durable_row[1]) > int(time.time()) + 29 * 24 * 60 * 60
      browser_session_columns = machine.succeed(
          "mariadb --batch --skip-column-names information_schema --execute="
          + shlex.quote(
              "SELECT COLUMN_NAME FROM COLUMNS "
              "WHERE TABLE_SCHEMA = 'zotero_www' "
              "AND TABLE_NAME = 'browser_sessions' "
              "ORDER BY ORDINAL_POSITION"
          )
      ).splitlines()
      assert browser_session_columns == [
          "tokenHash",
          "userID",
          "expiresAt",
          "createdAt",
          "lastSeenAt",
          "userAgent",
      ], browser_session_columns
      stylesheet = re.search(
          r'href="(/_topcoat/assets/[^"]+)"', repeated_result
      )
      assert stylesheet is not None, repeated_result
      stylesheet_body = machine.succeed(
          "curl --silent --show-error --fail-with-body "
          + shlex.quote(public_url + stylesheet.group(1))
      )
      assert len(stylesheet_body) > 1000, len(stylesheet_body)
      assert login_session_status(repeated_session["sessionToken"])["status"] == "completed"

      library_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/")
      )
      library_config_match = re.search(
          r'id="zotero-web-library-config">([^<]+)</script>',
          library_page,
      )
      assert library_config_match is not None, library_page
      library_config = json.loads(library_config_match.group(1))
      assert library_config["userSlug"] == "alice", library_config
      assert "portalMode" not in library_config, library_config
      web_ui_key = library_config["apiKey"]
      web_ui_key_row = machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT name, webUI FROM `keys` "
              f"WHERE `key` = '{web_ui_key}' AND userID = {oidc_user_id}"
          )
      ).strip().split("\t")
      assert web_ui_key_row == [durable_hash.lower(), "1"], web_ui_key_row
      library_menu_match = re.search(
          r'id="zotero-web-library-menu-config">([^<]+)</script>',
          library_page,
      )
      assert library_menu_match is not None, library_page
      library_menu = json.loads(library_menu_match.group(1))
      account_menu = library_menu["desktop"][-1]
      assert account_menu["label"] == "alice", account_menu
      assert {"separator": True} in account_menu["entries"], account_menu

      personal_library = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/alice/library")
      )
      assert 'id="zotero-web-library"' in personal_library, personal_library
      assert '"userSlug":"alice"' in personal_library, personal_library

      collection_deep_link = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(
              public_url + "/alice/collections/KLC7QV4L/collection"
          )
      )
      assert 'id="zotero-web-library"' in collection_deep_link, collection_deep_link
      assert '"userSlug":"alice"' in collection_deep_link, collection_deep_link

      invalid_personal_library_status = machine.succeed(
          "curl --silent --show-error --output /tmp/invalid-personal-library.html "
          "--write-out='%{http_code}' --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/alice/not-a-web-library-route")
      ).strip()
      assert invalid_personal_library_status == "404", invalid_personal_library_status

      legacy_library_status = machine.succeed(
          "curl --silent --show-error --output /tmp/legacy-library.html "
          "--write-out='%{http_code}' --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/library")
      ).strip()
      assert legacy_library_status == "404", legacy_library_status
      legacy_library = machine.succeed("cat /tmp/legacy-library.html")
      assert "The page may have moved" in legacy_library, legacy_library

      missing_status = machine.succeed(
          "curl --silent --show-error --output /tmp/missing.html "
          "--write-out='%{http_code}' --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/foo/bar")
      ).strip()
      assert missing_status == "404", missing_status
      missing_page = machine.succeed("cat /tmp/missing.html")
      assert "<!DOCTYPE html>" in missing_page, missing_page
      assert "The page may have moved" in missing_page, missing_page
      assert "/_topcoat/assets/" in missing_page, missing_page

      profile_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/alice")
      )
      assert "Alice Developer" in profile_page, profile_page
      assert re.search(r"<span[^>]*>alice</span>", profile_page), profile_page
      assert 'href="/groups"' in profile_page, profile_page
      assert 'action="/user/logout"' in profile_page, profile_page

      trailing_group_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/groups/new/")
      )
      assert trailing_group_headers.startswith("HTTP/2 308"), trailing_group_headers
      assert "location: /groups/new" in trailing_group_headers.lower(), trailing_group_headers
      new_group_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/groups/new")
      )
      assert "Create a Group" in new_group_page, new_group_page

      group_name = "OIDC integration group"
      group_result = machine.succeed(
          "curl --silent --show-error --fail-with-body --location --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " --data-urlencode "
          + shlex.quote("name=" + group_name)
          + " "
          + shlex.quote(public_url + "/groups/new")
      )
      assert "Library Editing" in group_result, group_result
      group_id = int(machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              f"SELECT groupID FROM groups WHERE name = '{group_name}'"
          )
      ).strip())
      group_slug = "oidc_integration_group"
      group_path = f"/groups/{group_id}/{group_slug}"
      groups_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/groups")
      )
      assert "Zotero Groups" in groups_page, groups_page
      assert "Your Groups" not in groups_page, groups_page
      assert group_name in groups_page, groups_page
      assert f'href="{group_path}"' in groups_page, groups_page
      assert f'href="/groups/{group_id}/settings"' in groups_page, groups_page
      assert f'href="/groups/{group_id}/settings/members"' in groups_page, groups_page
      assert f'href="/groups/{group_id}/settings/library"' in groups_page, groups_page

      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              "INSERT INTO users (username, password, email) VALUES "
              "('new-owner', '!oidc-only!', 'new-owner@example.net'); "
              "SET @candidate = LAST_INSERT_ID(); "
              "INSERT INTO users_email (userID, email, validated) VALUES "
              "(@candidate, 'new-owner@example.net', 1); "
              "INSERT INTO users_meta VALUES "
              "(@candidate, 'profile_realname', 'New Group Owner'); "
              "INSERT INTO GDN_User VALUES (@candidate, 0); "
              "INSERT INTO oidc_identities (issuer, subject, userID) VALUES "
              "('http://127.0.0.1:19090', 'new-owner', @candidate)"
          )
      )
      candidate_user_id = int(machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT userID FROM users WHERE username = 'new-owner'"
          )
      ).strip())

      created_member = json.loads(machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " --request POST --header 'Content-Type: application/json' --data "
          + shlex.quote(json.dumps({
              "identifier": "new-owner",
              "role": "member",
          }))
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/members")
      ))
      assert created_member == {
          "userId": candidate_user_id,
          "username": "new-owner",
          "role": "member",
      }, created_member

      members_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/settings/members")
      )
      assert "Current Members" in members_page, members_page
      assert "new-owner" in members_page, members_page
      assert "Member Since" in members_page, members_page
      assert "Update Roles" in members_page, members_page
      assert 'value="admin"' in members_page, members_page
      assert 'value="remove"' in members_page, members_page

      role_update = json.loads(machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " --request PATCH --header 'Content-Type: application/json' --data "
          + shlex.quote(json.dumps({
              "members": [{
                  "userId": candidate_user_id,
                  "role": "admin",
              }],
          }))
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/members")
      ))
      assert role_update == {"updated": 1}, role_update
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT role FROM groupUsers "
              f"WHERE groupID = {group_id} AND userID = {candidate_user_id}"
          )
      ).strip() == "admin"

      renamed_group_name = "Renamed OIDC integration group"
      renamed_group_slug = "renamed_oidc_integration_group"
      renamed_result = machine.succeed(
          "curl --silent --show-error --fail-with-body --location --cookie "
          + shlex.quote(cookie_file)
          + " --data-urlencode "
          + shlex.quote("name=" + renamed_group_name)
          + " --data-urlencode "
          + shlex.quote("description=Integration description")
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/settings")
      )
      assert "Group Settings Saved" in renamed_result, renamed_result
      assert "Integration description" in renamed_result, renamed_result
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(f"SELECT name FROM groups WHERE groupID = {group_id}")
      ).strip() == renamed_group_name

      library_settings = machine.succeed(
          "curl --silent --show-error --fail-with-body --location --cookie "
          + shlex.quote(cookie_file)
          + " --data library_editing=admins --data file_editing=none "
          + shlex.quote(f"{public_url}/groups/{group_id}/settings/library")
      )
      assert "Settings saved" in library_settings, library_settings
      assert "Only group admins" in library_settings, library_settings
      assert "No group file storage" in library_settings, library_settings

      transfer_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null "
          "--cookie "
          + shlex.quote(cookie_file)
          + " --data username=new-owner "
          + shlex.quote(f"{public_url}/groups/{group_id}/owner")
      )
      assert transfer_headers.startswith("HTTP/2 303"), transfer_headers
      assert (
          f"location: /groups/{group_id}/settings/members"
          in transfer_headers.lower()
      ), transfer_headers
      ownership = machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT userID, role FROM groupUsers "
              f"WHERE groupID = {group_id} ORDER BY userID"
          )
      ).splitlines()
      assert f"{oidc_user_id}\tadmin" in ownership, ownership
      assert f"{candidate_user_id}\towner" in ownership, ownership

      self_demote = json.loads(machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " --request PATCH --header 'Content-Type: application/json' --data "
          + shlex.quote(json.dumps({
              "members": [{
                  "userId": oidc_user_id,
                  "role": "member",
              }],
          }))
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/members")
      ))
      assert self_demote == {"updated": 1}, self_demote

      forbidden_status = machine.succeed(
          "curl --silent --show-error --output /tmp/group-forbidden.html "
          "--write-out='%{http_code}' --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/settings")
      ).strip()
      assert forbidden_status == "403", forbidden_status
      forbidden_page = machine.succeed("cat /tmp/group-forbidden.html")
      assert "<!DOCTYPE html>" in forbidden_page, forbidden_page
      assert "Your account is not allowed" in forbidden_page, forbidden_page

      overview_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(
              public_url
              + f"/groups/{group_id}/{renamed_group_slug}"
          )
      )
      assert "Group Library" in overview_page, overview_page
      assert "Members" in overview_page, overview_page
      assert "Leave Group" in overview_page, overview_page
      assert "Group Settings" not in overview_page, overview_page
      leave_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null "
          "--cookie "
          + shlex.quote(cookie_file)
          + " --request POST "
          + shlex.quote(f"{public_url}/groups/{group_id}/members/leave")
      )
      assert leave_headers.startswith("HTTP/2 303"), leave_headers

      settings_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/settings")
      )
      assert "Account" in settings_page, settings_page
      assert "Your username and email are managed" in settings_page, settings_page
      security_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/settings/security")
      )
      assert "Browser Sessions" in security_page, security_page
      assert "Applications" in security_page, security_page
      assert "Sign out all sessions" not in security_page, security_page
      assert "Disconnect all apps" not in security_page, security_page

      key_payload = {
          "key": None,
          "name": "VM integration key",
          "access": {
              "user": {
                  "library": True,
                  "notes": False,
                  "files": True,
                  "write": False,
              },
              "groups": {
                  "all": {
                      "library": True,
                      "write": False,
                  },
              },
          },
      }
      saved_key = json.loads(machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " --request POST --header 'Content-Type: application/json' --data "
          + shlex.quote(json.dumps(key_payload))
          + " "
          + shlex.quote(public_url + "/settings/savekey")
      ))
      assert saved_key["created"] is True, saved_key
      assert saved_key["name"] == "VM integration key", saved_key
      assert len(saved_key["key"]) == 24, saved_key
      security_with_key = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/settings/security")
      )
      assert "VM integration key" in security_with_key, security_with_key
      assert "Create new private key" in security_with_key, security_with_key
      revoke_key_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null "
          "--cookie "
          + shlex.quote(cookie_file)
          + " --data "
          + shlex.quote("key=" + saved_key["key"])
          + " "
          + shlex.quote(public_url + "/settings/keys/revoke")
      )
      assert revoke_key_headers.startswith("HTTP/2 303"), revoke_key_headers

      other_session_handle = "ab" * 32
      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              "INSERT INTO browser_sessions "
              "(tokenHash, userID, expiresAt, userAgent) VALUES "
              f"(UNHEX('{other_session_handle}'), {oidc_user_id}, "
              "DATE_ADD(NOW(), INTERVAL 30 DAY), "
              "'Mozilla/5.0 (X11; Linux x86_64; rv:141.0) Gecko/20100101 Firefox/141.0')"
          )
      )
      sessions_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/settings/security")
      )
      assert sessions_page.count('data-browser-session-item=""') >= 2, sessions_page
      assert "Firefox on Linux" in sessions_page, sessions_page
      assert "Current" in sessions_page, sessions_page
      assert "Remove Session" in sessions_page, sessions_page
      revoke_session_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null "
          "--cookie "
          + shlex.quote(cookie_file)
          + " --data "
          + shlex.quote("handle=" + other_session_handle)
          + " "
          + shlex.quote(public_url + "/settings/sessions")
      )
      assert revoke_session_headers.startswith("HTTP/2 303"), revoke_session_headers
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT COUNT(*) FROM browser_sessions "
              f"WHERE tokenHash = UNHEX('{other_session_handle}')"
          )
      ).strip() == "0"

      storage_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/settings/storage")
      )
      assert "Current Plan" in storage_page, storage_page
      assert "Current Usage" in storage_page, storage_page
      assert "Purge Storage in My Library" in storage_page, storage_page

      portal_log = machine.succeed(
          "journalctl -u zotero-portal.service --no-pager -o cat"
      )
      assert "finished processing request" in portal_log, portal_log
      assert "OIDC login completed" in portal_log, portal_log
      assert "group created" in portal_log, portal_log
      oidc_attachment_template = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              + shlex.quote(
                  api_url
                  + "/items/new?itemType=attachment&linkMode=imported_file"
              )
          )
      )
      oidc_created = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              "--request POST --header 'Content-Type: application/json' "
              + "--header "
              + shlex.quote(
                  f"Zotero-API-Key: {parallel_statuses[0]['apiKey']}"
              )
              + " --data "
              + shlex.quote(json.dumps([oidc_attachment_template]))
              + " "
              + shlex.quote(
                  f"{api_url}/users/{oidc_user_id}/items"
              )
          )
      )
      oidc_success = oidc_created["success"]
      oidc_attachment_key = (
          oidc_success[0]
          if isinstance(oidc_success, list)
          else next(iter(oidc_success.values()))
      )
      oidc_library_id = machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              f"SELECT libraryID FROM users WHERE userID = {oidc_user_id}"
          )
      ).strip()
      machine.succeed(
          "mariadb zotero_shard --execute="
          + shlex.quote(
              "UPDATE shardLibraries SET storageUsage = 2199023255552 "
              f"WHERE libraryID = {oidc_library_id}"
          )
      )
      machine.succeed("systemctl restart memcached.service")
      machine.wait_for_unit("memcached.service")
      large_storage_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/settings/storage")
      )
      assert "2.00 TiB" in large_storage_page, large_storage_page
      unlimited_parameters = urllib.parse.urlencode(
          {
              "md5": hashlib.md5(b"unlimited").hexdigest(),
              "filename": "unlimited.txt",
              "filesize": 1,
              "mtime": int(time.time() * 1000),
              "contentType": "text/plain",
          }
      )
      unlimited_authorization = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              "--request POST "
              "--header 'Content-Type: application/x-www-form-urlencoded' "
              "--header 'If-None-Match: *' --header "
              + shlex.quote(
                  f"Zotero-API-Key: {parallel_statuses[0]['apiKey']}"
              )
              + " --data "
              + shlex.quote(unlimited_parameters)
              + " "
              + shlex.quote(
                  f"{api_url}/users/{oidc_user_id}/items/"
                  f"{oidc_attachment_key}/file"
              )
          )
      )
      assert unlimited_authorization["uploadKey"], unlimited_authorization
      machine.succeed(
          "mariadb zotero_shard --execute="
          + shlex.quote(
              "UPDATE shardLibraries SET storageUsage = 0 "
              f"WHERE libraryID = {oidc_library_id}"
          )
      )
      machine.succeed("systemctl restart memcached.service")
      machine.wait_for_unit("memcached.service")

      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              "UPDATE browser_sessions "
              "SET expiresAt = FROM_UNIXTIME(UNIX_TIMESTAMP() + 60) "
              f"WHERE tokenHash = UNHEX('{durable_hash}')"
          )
      )
      automatic_session = create_login_session()
      automatic_result = machine.succeed(
          "curl --silent --show-error --fail-with-body --location --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(automatic_session["loginURL"])
      )
      assert "Login complete" in automatic_result, automatic_result
      refreshed_expiry = int(machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT CAST(UNIX_TIMESTAMP(expiresAt) AS UNSIGNED) "
              "FROM browser_sessions "
              f"WHERE tokenHash = UNHEX('{durable_hash}')"
          )
      ).strip())
      assert refreshed_expiry > int(time.time()) + 29 * 24 * 60 * 60
      assert login_session_status(
          automatic_session["sessionToken"]
      )["status"] == "completed"

      expired_hash = hashlib.sha256(
          base64.urlsafe_b64decode(browser_session_cookie())
      ).hexdigest().upper()
      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              "UPDATE browser_sessions "
              "SET expiresAt = FROM_UNIXTIME(UNIX_TIMESTAMP() - 1) "
              f"WHERE tokenHash = UNHEX('{expired_hash}')"
          )
      )
      expired_session = create_login_session()
      expired_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(expired_session["loginURL"])
      )
      assert "Continue with OpenID Connect" in expired_page, expired_page
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT COUNT(*) FROM browser_sessions "
              f"WHERE tokenHash = UNHEX('{expired_hash}')"
          )
      ).strip() == "0"

      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              f"UPDATE GDN_User SET Banned = 1 WHERE UserID = {oidc_user_id}"
          )
      )
      banned_session = create_login_session()
      banned_authorization = oidc_authorization_url(
          banned_session, "/tmp/oidc-banned.cookies"
      )
      banned_status = machine.succeed(
          "curl --silent --show-error --location "
          "--data-urlencode sub=alice "
          "--output /tmp/oidc-banned.html --write-out='%{http_code}' "
          "--cookie /tmp/oidc-banned.cookies "
          "--cookie-jar /tmp/oidc-banned.cookies "
          + shlex.quote(banned_authorization)
      ).strip()
      assert banned_status == "403", banned_status
      assert login_session_status(banned_session["sessionToken"])["status"] == "pending"
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              f"SELECT Banned FROM GDN_User WHERE UserID = {oidc_user_id}"
          )
      ).strip() == "1"
      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              f"UPDATE GDN_User SET Banned = 0 WHERE UserID = {oidc_user_id}; "
              f"UPDATE users SET role = 'deleted' WHERE userID = {oidc_user_id}"
          )
      )
      deleted_session = create_login_session()
      deleted_authorization = oidc_authorization_url(
          deleted_session, "/tmp/oidc-deleted.cookies"
      )
      deleted_status = machine.succeed(
          "curl --silent --show-error --location "
          "--data-urlencode sub=alice "
          "--output /tmp/oidc-deleted.html --write-out='%{http_code}' "
          "--cookie /tmp/oidc-deleted.cookies "
          "--cookie-jar /tmp/oidc-deleted.cookies "
          + shlex.quote(deleted_authorization)
      ).strip()
      assert deleted_status == "403", deleted_status
      assert login_session_status(deleted_session["sessionToken"])["status"] == "pending"
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              f"SELECT role FROM users WHERE userID = {oidc_user_id}"
          )
      ).strip() == "deleted"

      key = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              "--request POST "
              "--header 'Content-Type: application/json' "
              "--header 'Zotero-API-Version: 3' "
              "--data '{\"username\":\"testuser\","
              "\"password\":\"letmein\",\"name\":\"VM test\","
              "\"access\":{\"user\":{\"library\":true,"
              "\"notes\":true,\"write\":true},\"groups\":{\"all\":{"
              "\"library\":true,\"write\":true}}}}' "
              + shlex.quote(api_url + "/keys")
          )
      )
      assert key["userID"] == 1, key
      assert key["key"], key
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_master "
          "--execute='SELECT username FROM users WHERE userID = 1'"
      ).strip() == "testuser"
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_shard "
          "--execute='SELECT libraryType FROM shardLibraries WHERE libraryID = 1'"
      ).strip() == "user"

      def post_sync_objects(object_type, objects):
          response = json.loads(
              machine.succeed(
                  "curl --silent --show-error --fail-with-body "
                  "--request POST --header 'Content-Type: application/json' "
                  "--header 'Zotero-API-Version: 3' --header "
                  + shlex.quote(f"Zotero-API-Key: {key['key']}")
                  + " --data "
                  + shlex.quote(json.dumps(objects))
                  + " "
                  + shlex.quote(f"{api_url}/users/1/{object_type}")
              )
          )
          assert not response["failed"], response
          assert response["successful"], response
          return response

      custom_collection_uri = public_url + "/groups/3/collections/IC9VBS74"
      post_sync_objects(
          "collections",
          [{
              "name": "Custom-base relation",
              "parentCollection": False,
              "relations": {"owl:sameAs": [custom_collection_uri]},
          }],
      )

      item_template = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              + shlex.quote(api_url + "/items/new?itemType=book")
          )
      )
      custom_item_uri = public_url + "/users/1/items/ABCDEFGH"
      item_template["title"] = "Custom-base relation"
      item_template["relations"] = {"owl:sameAs": [custom_item_uri]}
      post_sync_objects("items", [item_template])

      attachment_proxy = json.loads(
          machine.succeed(
              "cat /etc/zotero-dataserver/attachment-proxy/attachment-proxy.json"
          )
      )
      assert attachment_proxy["port"] == 13000, attachment_proxy
      assert attachment_proxy["s3"]["endpoint"] == "http://127.0.0.1:11000"

      citeproc_server = json.loads(
          machine.succeed("cat /etc/zotero-dataserver/citeproc-js-server/citeproc-js-server.json")
      )
      assert citeproc_server == {"port": 18085}, citeproc_server

      stream_server = json.loads(
          machine.succeed("cat /etc/zotero-dataserver/stream-server/stream-server.json")
      )
      assert "listenAddress" not in stream_server, stream_server
      assert "statusInterval" not in stream_server, stream_server
      assert stream_server["redis"] == {
          "url": "redis://127.0.0.1:16389",
          "prefix": "zotero:",
      }, stream_server
      assert stream_server["apiURL"] == api_url + "/", stream_server

      stream_client = """
      import json
      from pathlib import Path
      import websocket

      connection = websocket.create_connection(
          "wss://${testDomain}/stream/",
          timeout=30,
          sslopt={"ca_certs": "${testCertificates.ca.cert}"},
      )
      connection.send(json.dumps({
          "action": "createSubscriptions",
          "subscriptions": [{
              "apiKey": %s,
              "topics": ["/users/1"],
          }],
      }))
      while True:
          message = json.loads(connection.recv())
          if message.get("event") == "subscriptionsCreated":
              assert message["subscriptions"] == [{
                  "apiKey": %s,
                  "topics": ["/users/1"],
              }], message
              Path("/tmp/stream-ready").touch()
          elif message.get("event") == "topicUpdated":
              assert message["topic"] == "/users/1", message
              Path("/tmp/stream-event").write_text(json.dumps(message))
              break
      """ % (json.dumps(key["key"]), json.dumps(key["key"]))
      machine.succeed(
          "python -c " + shlex.quote(stream_client)
          + " >/tmp/stream-client.log 2>&1 &"
      )
      machine.wait_until_succeeds("test -e /tmp/stream-ready")

      credential_state = machine.succeed(
          "systemctl show -P StateDirectory "
          "zotero-dataserver-provision.service"
      ).strip()
      credential_directory = f"/var/lib/{credential_state}"
      for name, length in {
          "AUTH_SALT": 64,
          "ATTACHMENT_PROXY_SECRET": 64,
          "DATASERVER_SUPER_PASSWORD": 64,
          "PORTAL_COOKIE_KEY": 88,
          "S3_ACCESS_KEY_ID": 32,
          "S3_SECRET_ACCESS_KEY": 64,
      }.items():
          value = machine.succeed(f"cat {credential_directory}/{name}").strip()
          assert len(value) == length, (name, len(value))

      for unit in [
          "phpfpm-zotero-dataserver.service",
          "versitygw.service",
          "zotero-attachment-proxy.service",
      ]:
          definition = machine.succeed(f"systemctl cat {unit}")
          assert "LoadCredential=" in definition, definition
          assert "/var/lib/zotero-dataserver/credentials" not in definition, definition

      def s3(method, path, options=""):
          return machine.succeed(
              "curl --silent --show-error --fail-with-body "
              "--aws-sigv4 'aws:amz:us-east-1:s3' "
              f"--user \"$(cat {credential_directory}/S3_ACCESS_KEY_ID):"
              f"$(cat {credential_directory}/S3_SECRET_ACCESS_KEY)\" "
              f"--request {method} {options} "
              f"http://127.0.0.1:11000/{path}"
          )

      s3("PUT", "zotero/testhash", "--data-binary 'attachment data'")
      assert s3("GET", "zotero/testhash") == "attachment data"

      proxy_secret = machine.succeed(
          f"cat {credential_directory}/ATTACHMENT_PROXY_SECRET"
      ).strip()
      payload = base64.b64encode(
          json.dumps(
              {
                  "expires": int(time.time()) + 300,
                  "hash": "testhash",
                  "filename": "attachment.txt",
                  "contentType": "text/plain",
              },
              separators=(",", ":"),
          ).encode()
      ).decode()
      signature = hmac.new(
          proxy_secret.encode(), payload.encode(), hashlib.sha256
      ).hexdigest()
      attachment_url = (
          public_url + "/files/"
          + urllib.parse.quote(payload, safe="")
          + "/"
          + signature
          + "/attachment.txt"
      )
      assert machine.succeed(
          "curl --silent --show-error --fail-with-body "
          + shlex.quote(attachment_url)
      ) == "attachment data"

      attachment_template = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              + shlex.quote(
                  api_url
                  + "/items/new?itemType=attachment&linkMode=imported_file"
              )
          )
      )
      created = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              "--request POST --header 'Content-Type: application/json' "
              + "--header "
              + shlex.quote(f"Zotero-API-Key: {key['key']}")
              + " --data "
              + shlex.quote(json.dumps([attachment_template]))
              + " "
              + shlex.quote(api_url + "/users/1/items")
          )
      )
      success = created["success"]
      attachment_key = (
          success[0] if isinstance(success, list) else next(iter(success.values()))
      )

      client_file = "dataserver attachment data"
      client_file_hash = hashlib.md5(client_file.encode()).hexdigest()
      file_parameters = urllib.parse.urlencode(
          {
              "md5": client_file_hash,
              "filename": "dataserver.txt",
              "filesize": len(client_file),
              "mtime": int(time.time() * 1000),
              "contentType": "text/plain",
              "charset": "utf-8",
          }
      )
      authorization = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body "
              "--request POST "
              "--header 'Content-Type: application/x-www-form-urlencoded' "
              "--header 'If-None-Match: *' --header "
              + shlex.quote(f"Zotero-API-Key: {key['key']}")
              + " --data "
              + shlex.quote(file_parameters)
              + " "
              + shlex.quote(
                  f"{api_url}/users/1/items/{attachment_key}/file"
              )
          )
      )
      upload_body = base64.b64encode(
          (
              authorization["prefix"]
              + client_file
              + authorization["suffix"]
          ).encode()
      ).decode()
      upload_response = machine.succeed(
          "printf %s "
          + shlex.quote(upload_body)
          + " | base64 --decode | "
          "curl --silent --show-error --write-out='\\n%{http_code}' "
          "--request POST --header "
          + shlex.quote(f"Content-Type: {authorization['contentType']}")
          + " --data-binary @- "
          + shlex.quote(authorization["url"])
      ).rstrip()
      upload_body, upload_status = upload_response.rsplit("\n", 1)
      assert upload_status == "201", (upload_status, upload_body)

      registration_status = machine.succeed(
          "curl --silent --show-error --fail-with-body --output /dev/null "
          "--write-out='%{http_code}' --request POST "
          "--header 'Content-Type: application/x-www-form-urlencoded' "
          "--header 'If-None-Match: *' --header "
          + shlex.quote(f"Zotero-API-Key: {key['key']}")
          + " --data "
          + shlex.quote(f"upload={authorization['uploadKey']}")
          + " "
          + shlex.quote(f"{api_url}/users/1/items/{attachment_key}/file")
      ).strip()
      assert registration_status == "204", registration_status

      downloaded = machine.succeed(
          "curl --silent --show-error --fail-with-body --location --header "
          + shlex.quote(f"Zotero-API-Key: {key['key']}")
          + " "
          + shlex.quote(f"{api_url}/users/1/items/{attachment_key}/file")
      )
      assert downloaded == client_file, downloaded

      fulltext_phrase = "quasar marmalade integration marker"
      fulltext_payload = {
          "content": fulltext_phrase,
          "indexedChars": len(fulltext_phrase),
          "totalChars": len(fulltext_phrase),
      }
      fulltext_status = machine.succeed(
          "curl --silent --show-error --fail-with-body --output /dev/null "
          "--write-out='%{http_code}' --request PUT "
          "--header 'Content-Type: application/json' --header "
          + shlex.quote(f"Zotero-API-Key: {key['key']}")
          + " --data "
          + shlex.quote(json.dumps(fulltext_payload))
          + " "
          + shlex.quote(
              f"{api_url}/users/1/items/{attachment_key}/fulltext"
          )
      ).strip()
      assert fulltext_status == "204", fulltext_status

      fulltext_search_url = api_url + "/users/1/items?" + urllib.parse.urlencode(
          {"q": "quasar marmalade", "qmode": "everything"}
      )
      for _ in range(30):
          fulltext_results = json.loads(
              machine.succeed(
                  "curl --silent --show-error --fail-with-body --header "
                  + shlex.quote(f"Zotero-API-Key: {key['key']}")
                  + " "
                  + shlex.quote(fulltext_search_url)
              )
          )
          if any(item["key"] == attachment_key for item in fulltext_results):
              break
          time.sleep(1)
      else:
          raise AssertionError(fulltext_results)

      fulltext_index_status = json.loads(
          machine.succeed(
              "curl --silent --show-error --fail-with-body --header "
              + shlex.quote(f"Zotero-API-Key: {key['key']}")
              + " "
              + shlex.quote(api_url + "/users/1/fulltext/index")
          )
      )
      assert fulltext_index_status == {"status": "indexed"}, fulltext_index_status

      try:
          machine.wait_until_succeeds("test -e /tmp/stream-event", timeout=20)
      except Exception:
          print(machine.succeed("cat /tmp/stream-client.log || true"))
          print(machine.succeed(
              "journalctl -u phpfpm-zotero-dataserver "
              "-u zotero-stream-server --no-pager -n 100"
          ))
          raise
      stream_event = json.loads(machine.succeed("cat /tmp/stream-event"))
      assert stream_event["event"] == "topicUpdated", stream_event

      response = machine.succeed(
          "curl --silent --show-error --output /dev/null "
          "--write-out='%{http_code}' " + shlex.quote(public_url + "/")
      ).strip()
      assert response == "307", response

      legacy_keys_response = machine.succeed(
          "curl --silent --show-error --output /dev/null "
          "--write-out='%{http_code}' " + shlex.quote(public_url + "/keys")
      ).strip()
      assert legacy_keys_response == "307", legacy_keys_response

      attachment_proxy_response = machine.succeed(
          "curl --silent --show-error --output /dev/null "
          "--write-out='%{http_code}' " + shlex.quote(public_url + "/files/")
      ).strip()
      assert attachment_proxy_response == "200", attachment_proxy_response

      stream_response = machine.succeed(
          "curl --silent --show-error --output /dev/null "
          "--write-out='%{http_code}' "
          + shlex.quote(public_url + "/stream/health")
      ).strip()
      assert stream_response == "200", stream_response

      citation = machine.succeed(
          "curl --silent --show-error --fail-with-body "
          "--header 'Content-Type: application/json' "
          "--data '{\"items\":[{\"id\":\"ITEM-1\",\"type\":\"book\","
          "\"title\":\"Test Book\",\"author\":[{\"family\":\"Doe\","
          "\"given\":\"Jane\"}],\"issued\":{\"date-parts\":[[2026]]}}]}' "
          "'http://127.0.0.1:18085?responseformat=html&style=apa'"
      )
      assert "Test Book" in citation, citation

      cleaned_note = machine.succeed(
          "printf '<p onclick=\"evil()\">Hello<script>x</script></p>' | "
          "curl --silent --show-error --fail-with-body "
          "--header 'Content-Type: text/plain' --data-binary @- "
          "http://127.0.0.1:16343"
      )
      assert cleaned_note == "<p>Hello</p>", cleaned_note

      imported = json.loads(
          machine.succeed(
              "printf 'TY  - JOUR\\nTI  - Test Article\\n"
              "AU  - Doe, Jane\\nER  -\\n' | "
              "curl --silent --show-error --fail-with-body "
              "--header 'Content-Type: text/plain' --data-binary @- "
              "http://127.0.0.1:11969/import"
          )
      )
      assert imported[0]["title"] == "Test Article", imported

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

    testScript = ''
      start_all()

      machine.wait_for_unit("versitygw.service")
      machine.wait_for_open_port(10000)
      unit = machine.succeed("systemctl cat versitygw.service")
      assert "LoadCredential=root-access-key-id:" in unit, unit
      assert "LoadCredential=root-secret-access-key:" in unit, unit
      assert "EnvironmentFile=" not in unit, unit
      command_line = machine.succeed(
          "xargs --null --max-args=1 < /proc/"
          "$(systemctl show -P MainPID versitygw.service)/cmdline"
      ).splitlines()
      assert "--disableotmp" in command_line, command_line
      assert "--disable-copy-file-range" in command_line, command_line
      assert "/var/lib/versitygw-sidecar" in command_line, command_line
      assert "/srv/versitygw-test" in command_line, command_line
      assert machine.succeed(
          "stat --format='%U:%G:%a' /var/lib/versitygw-sidecar"
      ).strip() == "versitygw:versitygw:750"
      machine.succeed(
          "curl --silent --show-error --fail http://127.0.0.1:10000/health"
      )

      def s3(method, path, options=""):
          return machine.succeed(
              "curl --silent --show-error --fail-with-body "
              "--aws-sigv4 'aws:amz:us-east-1:s3' "
              "--user 'myaccess:mysecret' "
              f"--request {method} {options} "
              f"http://127.0.0.1:10000/{path}"
          )

      s3("PUT", "zotero")
      s3("PUT", "zotero/attachment.txt", "--data-binary 'attachment data'")
      assert s3("GET", "zotero/attachment.txt") == "attachment data"

      machine.succeed("systemctl restart versitygw.service")
      machine.wait_for_open_port(10000)
      assert s3("GET", "zotero/attachment.txt") == "attachment data"
    '';
  };
}
