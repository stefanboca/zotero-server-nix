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
        virtualHost = "localhost";
        apiBaseUri = "http://localhost";
        wwwBaseUri = "https://localhost";

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
          globalConfig = "local_certs";
          virtualHosts.localhost.serverAliases = ["http://localhost"];
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
      };
    };

    testScript = ''
      import base64
      import hashlib
      import hmac
      import json
      import shlex
      import time
      import urllib.parse

      start_all()

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
          80,
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
                  "http://localhost/keys/sessions"
              )
          )

      def login_session_status(token):
          return json.loads(
              machine.succeed(
                  "curl --silent --show-error --fail-with-body "
                  + shlex.quote(f"http://localhost/keys/sessions/{token}")
              )
          )

      parallel_sessions = [create_login_session(), create_login_session()]
      parallel_commands = []
      for index, session in enumerate(parallel_sessions):
          parallel_commands.append(
              "curl --insecure --silent --show-error --fail-with-body "
              "--location --cookie-jar "
              + shlex.quote(f"/tmp/oidc-parallel-{index}.cookies")
              + " "
              + shlex.quote(session["loginURL"])
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
      first_headers = machine.succeed(
          "curl --insecure --silent --show-error --dump-header - "
          "--output /dev/null --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(repeated_session["loginURL"])
      )
      first_location = next(
          line.split(":", 1)[1].strip()
          for line in first_headers.splitlines()
          if line.lower().startswith("location:")
      )
      second_headers = machine.succeed(
          "curl --insecure --silent --show-error --dump-header - "
          "--output /dev/null --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(repeated_session["loginURL"])
      )
      second_location = next(
          line.split(":", 1)[1].strip()
          for line in second_headers.splitlines()
          if line.lower().startswith("location:")
      )
      assert first_location != second_location, (first_location, second_location)
      repeated_result = machine.succeed(
          "curl --insecure --silent --show-error --fail-with-body "
          "--location --cookie "
          + shlex.quote(cookie_file)
          + " --cookie-jar "
          + shlex.quote(cookie_file)
          + " "
          + shlex.quote(second_location)
      )
      assert "Login complete" in repeated_result, repeated_result
      assert login_session_status(repeated_session["sessionToken"])["status"] == "completed"
      assert machine.succeed(
          "mariadb --batch --skip-column-names zotero_www "
          "--execute='SELECT COUNT(*) FROM oidc_flows'"
      ).strip() == "0"

      machine.succeed(
          "mariadb zotero_www --execute="
          + shlex.quote(
              f"UPDATE GDN_User SET Banned = 1 WHERE UserID = {oidc_user_id}"
          )
      )
      banned_session = create_login_session()
      banned_status = machine.succeed(
          "curl --insecure --silent --show-error --location "
          "--output /tmp/oidc-banned.html --write-out='%{http_code}' "
          "--cookie-jar /tmp/oidc-banned.cookies "
          + shlex.quote(banned_session["loginURL"])
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
          "curl --insecure --silent --show-error --location "
          "--output /tmp/oidc-deleted.html --write-out='%{http_code}' "
          "--cookie-jar /tmp/oidc-deleted.cookies "
          + shlex.quote(deleted_session["loginURL"])
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
              "http://localhost/keys"
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
      assert stream_server["redis"] == {
          "url": "redis://127.0.0.1:16389",
          "prefix": "zotero:",
      }, stream_server
      assert stream_server["apiURL"] == "http://localhost/", stream_server

      stream_client = """
      import json
      from pathlib import Path
      import websocket

      connection = websocket.create_connection("ws://localhost/stream/", timeout=30)
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
          "http://localhost/files/"
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
              "'http://localhost/items/new?itemType=attachment&"
              "linkMode=imported_file'"
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
              + " http://localhost/users/1/items"
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
              + f" http://localhost/users/1/items/{attachment_key}/file"
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
          + f" http://localhost/users/1/items/{attachment_key}/file"
      ).strip()
      assert registration_status == "204", registration_status

      downloaded = machine.succeed(
          "curl --silent --show-error --fail-with-body --location --header "
          + shlex.quote(f"Zotero-API-Key: {key['key']}")
          + f" http://localhost/users/1/items/{attachment_key}/file"
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
          "--write-out='%{http_code}' http://localhost/"
      ).strip()
      assert response == "200", response

      attachment_proxy_response = machine.succeed(
          "curl --silent --show-error --output /dev/null "
          "--write-out='%{http_code}' http://localhost/files/"
      ).strip()
      assert attachment_proxy_response == "200", attachment_proxy_response

      stream_response = machine.succeed(
          "curl --silent --show-error --output /dev/null "
          "--write-out='%{http_code}' http://localhost/stream/health"
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
      imports = [module];

      systemd.tmpfiles.rules = [
        "d /srv/versitygw-test 0750 versitygw versitygw -"
        "d /var/lib/versitygw-sidecar 0750 versitygw versitygw -"
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
