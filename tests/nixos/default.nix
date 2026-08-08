{
  module,
  pkgs,
  versitygw,
  zotero-attachment-proxy,
  zotero-auth,
  zotero-citeproc-js-server,
  zotero-dataserver,
  zotero-stream-server,
  zotero-tinymce-clean-server,
  zotero-translation-server,
}: let
  testCertificates = import "${pkgs.path}/nixos/tests/common/acme/server/snakeoil-certs.nix";
  testDomain = testCertificates.domain;
  testPython = pkgs.python3.withPackages (pythonPackages: [
    pythonPackages.cryptography
    pythonPackages.pyjwt
    pythonPackages.websocket-client
  ]);
  oidcClientSecret = pkgs.writeText "oidc-client-secret" "oidc-test-secret";
in {
  nixos-dataserver = pkgs.testers.runNixOSTest {
    name = "zotero-dataserver";

    nodes.machine = {
      imports = [module];

      virtualisation.memorySize = 4 * 1024;

      environment.systemPackages = [testPython];

      services.zotero-dataserver = {
        enable = true;
        package = zotero-dataserver;
        virtualHost = testDomain;

        attachment-proxy = {
          package = zotero-attachment-proxy;
          settings.port = 13000;
        };
        auth = {
          enable = true;
          package = zotero-auth;
          issuer = "http://127.0.0.1:19090";
          clientId = "zotero-test";
          clientSecretFile = oidcClientSecret;
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
        redis.servers.zotero-dataserver.port = 16389;
        versitygw = {
          package = versitygw;
          port = 11000;
        };
      };

      systemd.services = {
        oidc-mock = {
          description = "OIDC test provider";
          wantedBy = ["multi-user.target"];
          before = ["zotero-auth.service"];
          serviceConfig = {
            DynamicUser = true;
            ExecStart = "${pkgs.lib.getExe testPython} ${./oidc-mock.py}";
            Restart = "on-failure";
          };
        };
        zotero-auth = {
          requires = ["oidc-mock.service"];
          after = ["oidc-mock.service"];
        };
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

      for unit in [
          "zotero-dataserver-credentials.service",
          "memcached.service",
          "redis-zotero-dataserver.service",
          "mysql.service",
          "versitygw.service",
          "oidc-mock.service",
          "zotero-auth-migrate.service",
          "zotero-auth.service",
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

      failed_units = machine.succeed(
          "systemctl list-units --state=failed --no-legend --plain"
      )
      assert failed_units.strip() == "", failed_units
      assert machine.succeed(
          "systemctl show -P User phpfpm-zotero-dataserver.service"
      ).strip() == "zotero-dataserver"
      assert machine.succeed(
          "systemctl show -P Group zotero-auth.service"
      ).strip() == "zotero-auth"
      assert "zotero-auth" in machine.succeed("id -nG caddy").split()

      for unit in [
          "zotero-dataserver-credentials.service",
          "zotero-auth-migrate.service",
          "zotero-dataserver-schema.service",
      ]:
          assert machine.succeed(
              f"systemctl show -P Restart {unit}"
          ).strip() == "on-failure"

      for unit in [
          "zotero-auth.service",
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

      auth_config = tomllib.loads(
          machine.succeed("cat /etc/zotero-dataserver/auth.toml")
      )
      assert "browser_session_lifetime_seconds" not in auth_config

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
          "oidc_browser_sessions",
          "oidc_flows",
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

      def oidc_start_url(session):
          return (
              public_url + "/auth/oidc/start?"
              + urllib.parse.urlencode({"session": session["sessionToken"]})
          )

      login_page_session = create_login_session()
      login_page = machine.succeed(
          "curl --silent --show-error --fail-with-body "
          + shlex.quote(login_page_session["loginURL"])
      )
      assert "Sign in to Zotero" in login_page, login_page
      assert "Continue with OpenID Connect" in login_page, login_page
      assert (
          f'action="/auth/oidc/start?session={login_page_session["sessionToken"]}"'
          in login_page
      ), login_page

      parallel_sessions = [create_login_session(), create_login_session()]
      parallel_commands = []
      for index, session in enumerate(parallel_sessions):
          parallel_commands.append(
              "curl --silent --show-error --fail-with-body "
              "--data= --location --cookie-jar "
              + shlex.quote(f"/tmp/oidc-parallel-{index}.cookies")
              + " "
              + shlex.quote(oidc_start_url(session))
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
          print(machine.succeed("journalctl -u zotero-auth -u oidc-mock --no-pager -n 200"))
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
          "(SELECT COUNT(*) FROM users WHERE email = 'oidc@example.net'), "
          "(SELECT COUNT(*) FROM oidc_identities "
          "WHERE issuer = 'http://127.0.0.1:19090' AND subject = 'oidc-user')\""
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
          "--data= --output /dev/null --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(oidc_start_url(repeated_session))
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
          + shlex.quote(oidc_start_url(repeated_session))
      )
      second_location = next(
          line.split(":", 1)[1].strip()
          for line in second_headers.splitlines()
          if line.lower().startswith("location:")
      )
      assert first_location != second_location, (first_location, second_location)
      oidc_flow_cookie = browser_session_cookie()
      repeated_result = machine.succeed(
          "curl --silent --show-error --fail-with-body "
          "--location --cookie "
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
      assert durable_cookie != oidc_flow_cookie
      durable_hash = hashlib.sha256(
          base64.urlsafe_b64decode(durable_cookie)
      ).hexdigest().upper()
      durable_row = machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT userID, CAST(UNIX_TIMESTAMP(expiresAt) AS UNSIGNED) "
              "FROM oidc_browser_sessions "
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
              "AND TABLE_NAME = 'oidc_browser_sessions' "
              "ORDER BY ORDINAL_POSITION"
          )
      ).splitlines()
      assert browser_session_columns == [
          "tokenHash",
          "userID",
          "expiresAt",
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
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_www "
          "--execute='SELECT COUNT(*) FROM oidc_flows'"
      ).strip() == "0"

      account_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/")
      )
      assert "Signed in as OIDC User" in account_page, account_page
      assert "Manage groups and file storage for oidc@example.net" in account_page, account_page
      assert 'href="/groups"' in account_page, account_page
      assert 'action="/user/logout"' in account_page, account_page
      assert "Your groups" not in account_page, account_page

      trailing_group_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/groups/new/")
      )
      assert trailing_group_headers.startswith("HTTP/2 308"), trailing_group_headers
      assert "location: /groups/new" in trailing_group_headers.lower(), trailing_group_headers
      group_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --location --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/groups/new/")
      )
      assert "Create a Zotero group" in group_page, group_page
      group_name = "OIDC integration group"
      group_result = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " --data-urlencode "
          + shlex.quote("name=" + group_name)
          + " "
          + shlex.quote(public_url + "/groups/new")
      )
      assert f"Created “{group_name}”" in group_result, group_result
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT COUNT(*) FROM groups "
              "JOIN groupUsers USING (groupID) "
              f"WHERE name = '{group_name}' "
              f"AND userID = {oidc_user_id} AND role = 'owner'"
          )
      ).strip() == "1"
      group_id = int(machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              f"SELECT groupID FROM groups WHERE name = '{group_name}'"
          )
      ).strip())
      groups_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/groups")
      )
      assert "Your groups" in groups_page, groups_page
      assert group_name in groups_page, groups_page
      assert f'href="/groups/{group_id}"' in groups_page, groups_page
      assert "You are the group owner" in groups_page, groups_page
      assert "1 person total" in groups_page, groups_page

      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              "INSERT INTO users (username, password, email) VALUES "
              "('new-owner', '!oidc-only!', 'new-owner@example.net'); "
              "SET @candidate = LAST_INSERT_ID(); "
              "INSERT INTO users_email VALUES "
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
      group_management = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}")
      )
      assert 'name="user_id"' in group_management, group_management
      assert 'type="number"' in group_management, group_management
      assert 'data-member-search=""' not in group_management, group_management
      search_status = machine.succeed(
          "curl --silent --show-error --output /tmp/removed-search.html "
          "--write-out='%{http_code}' --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(f"{public_url}/users/search?q=new-owner")
      ).strip()
      assert search_status == "404", search_status
      removed_search_page = machine.succeed("cat /tmp/removed-search.html")
      assert "Page not found" in removed_search_page, removed_search_page

      add_member_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null "
          "--cookie "
          + shlex.quote(cookie_file)
          + " --data operation=add --data role=member --data "
          + shlex.quote(f"user_id={candidate_user_id}")
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/members")
      )
      assert add_member_headers.startswith("HTTP/2 303"), add_member_headers
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT role FROM groupUsers "
              f"WHERE groupID = {group_id} AND userID = {candidate_user_id}"
          )
      ).strip() == "member"

      group_management = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}")
      )
      assert "New Group Owner" in group_management, group_management
      assert f"Zotero user ID {candidate_user_id}" in group_management, group_management
      transfer_url = (
          f"{public_url}/groups/{group_id}/owner?user_id={candidate_user_id}"
      )
      assert f'href="/groups/{group_id}/owner?user_id={candidate_user_id}"' in group_management
      transfer_confirmation = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(transfer_url)
      )
      assert "Transfer group ownership" in transfer_confirmation, transfer_confirmation
      assert "You will become an administrator" in transfer_confirmation, transfer_confirmation
      assert f'Zotero user ID {candidate_user_id}' in transfer_confirmation

      rejected_transfer_status = machine.succeed(
          "curl --silent --show-error --output /tmp/rejected-transfer.html "
          "--write-out='%{http_code}' --cookie "
          + shlex.quote(cookie_file)
          + " --data "
          + shlex.quote(f"user_id={candidate_user_id}")
          + " --data confirmation=wrong "
          + shlex.quote(f"{public_url}/groups/{group_id}/owner")
      ).strip()
      assert rejected_transfer_status == "400", rejected_transfer_status
      rejected_transfer = machine.succeed("cat /tmp/rejected-transfer.html")
      assert "That request could not be completed" in rejected_transfer, rejected_transfer
      assert "group name confirmation did not match" in rejected_transfer, rejected_transfer

      transfer_headers = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null "
          "--cookie "
          + shlex.quote(cookie_file)
          + " --data "
          + shlex.quote(f"user_id={candidate_user_id}")
          + " --data-urlencode "
          + shlex.quote("confirmation=" + group_name)
          + " "
          + shlex.quote(f"{public_url}/groups/{group_id}/owner")
      )
      assert transfer_headers.startswith("HTTP/2 303"), transfer_headers
      ownership = machine.succeed(
          "mariadb --batch --skip-column-names zotero_master --execute="
          + shlex.quote(
              "SELECT userID, role FROM groupUsers "
              f"WHERE groupID = {group_id} ORDER BY userID"
          )
      ).splitlines()
      assert f"{oidc_user_id}\tadmin" in ownership, ownership
      assert f"{candidate_user_id}\towner" in ownership, ownership

      storage_page = machine.succeed(
          "curl --silent --show-error --fail-with-body --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/account")
      )
      assert "Your identity is managed by the OpenID Connect provider" in storage_page, storage_page
      assert "Unlimited" in storage_page, storage_page
      assert f"Zotero user ID</dt><dd>{oidc_user_id}" in storage_page, storage_page
      assert "Used</dt><dd" in storage_page, storage_page
      storage_redirect = machine.succeed(
          "curl --silent --show-error --dump-header - --output /dev/null --cookie "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(public_url + "/settings/storage")
      )
      assert storage_redirect.startswith("HTTP/2 308"), storage_redirect
      assert "location: /account#storage" in storage_redirect.lower(), storage_redirect

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
          + shlex.quote(public_url + "/account")
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
              "UPDATE oidc_browser_sessions "
              "SET expiresAt = FROM_UNIXTIME(UNIX_TIMESTAMP() + 60) "
              f"WHERE tokenHash = UNHEX('{durable_hash}')"
          )
      )
      automatic_session = create_login_session()
      automatic_headers = machine.succeed(
          "curl --silent --show-error --dump-header - "
          "--output /dev/null --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(automatic_session["loginURL"])
      )
      automatic_location = next(
          line.split(":", 1)[1].strip()
          for line in automatic_headers.splitlines()
          if line.lower().startswith("location:")
      )
      assert automatic_location.startswith(
          "http://127.0.0.1:19090/authorize?"
      ), automatic_location
      refreshed_expiry = int(machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT CAST(UNIX_TIMESTAMP(expiresAt) AS UNSIGNED) "
              "FROM oidc_browser_sessions "
              f"WHERE tokenHash = UNHEX('{durable_hash}')"
          )
      ).strip())
      assert refreshed_expiry > int(time.time()) + 29 * 24 * 60 * 60
      flow_expiry = int(machine.succeed(
          "mariadb --batch --skip-column-names zotero_www --execute="
          + shlex.quote(
              "SELECT expires FROM oidc_flows "
              f"WHERE tokenHash = UNHEX('{durable_hash}')"
          )
      ).strip())
      assert flow_expiry > int(time.time()) + 14 * 60
      assert flow_expiry < refreshed_expiry
      automatic_result = machine.succeed(
          "curl --silent --show-error --fail-with-body --location "
          "--cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(automatic_location)
      )
      assert "Login complete" in automatic_result, automatic_result
      assert login_session_status(
          automatic_session["sessionToken"]
      )["status"] == "completed"

      expired_hash = hashlib.sha256(
          base64.urlsafe_b64decode(browser_session_cookie())
      ).hexdigest().upper()
      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              "UPDATE oidc_browser_sessions "
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
              "SELECT COUNT(*) FROM oidc_browser_sessions "
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
      banned_status = machine.succeed(
          "curl --silent --show-error --location "
          "--data= --output /tmp/oidc-banned.html --write-out='%{http_code}' "
          "--cookie-jar /tmp/oidc-banned.cookies "
          + shlex.quote(oidc_start_url(banned_session))
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
      deleted_status = machine.succeed(
          "curl --silent --show-error --location "
          "--data= --output /tmp/oidc-deleted.html --write-out='%{http_code}' "
          "--cookie-jar /tmp/oidc-deleted.cookies "
          + shlex.quote(oidc_start_url(deleted_session))
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

      attachment_proxy = json.loads(
          machine.succeed(
              "cat /etc/zotero-dataserver/attachment-proxy.json"
          )
      )
      assert attachment_proxy["port"] == 13000, attachment_proxy
      assert attachment_proxy["s3"]["endpoint"] == "http://127.0.0.1:11000"

      citeproc_server = json.loads(
          machine.succeed("cat /etc/zotero-dataserver/citeproc-js-server.json")
      )
      assert citeproc_server["host"] == "127.0.0.1", citeproc_server

      stream_server = json.loads(
          machine.succeed("cat /etc/zotero-dataserver/stream-server.json")
      )
      assert stream_server["listenAddress"] == "127.0.0.1", stream_server
      assert stream_server["statusInterval"] == 0, stream_server
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
          "zotero-dataserver-credentials.service"
      ).strip()
      credential_directory = f"/var/lib/{credential_state}/credentials"
      for name, length in {
          "auth-salt": 64,
          "attachment-proxy-secret": 64,
          "dataserver-super-password": 64,
          "s3-access-key-id": 32,
          "s3-secret-access-key": 64,
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
              f"--user \"$(cat {credential_directory}/s3-access-key-id):"
              f"$(cat {credential_directory}/s3-secret-access-key)\" "
              f"--request {method} {options} "
              f"http://127.0.0.1:11000/{path}"
          )

      s3("PUT", "zotero/testhash", "--data-binary 'attachment data'")
      assert s3("GET", "zotero/testhash") == "attachment data"

      proxy_secret = machine.succeed(
          f"cat {credential_directory}/attachment-proxy-secret"
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

      old_api_response = machine.succeed(
          "curl --silent --show-error --output /dev/null "
          "--write-out='%{http_code}' " + shlex.quote(public_url + "/keys")
      ).strip()
      assert old_api_response == "404", old_api_response

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

      stream_log = machine.succeed(
          "journalctl -u zotero-stream-server.service --no-pager -o cat"
      )
      assert re.search(
          r"\[\d+ connections, \d+ subscriptions\]", stream_log
      ) is None, stream_log
    '';
  };

  nixos-versitygw = pkgs.testers.runNixOSTest {
    name = "versitygw";

    nodes.machine = {
      imports = [module];

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
