{
  lib,
  pkgs,
}: let
  inherit (lib.meta) getExe;
  gcc =
    if lib.versionOlder pkgs.gcc.version "16"
    then pkgs.gcc16
    else lib.warn "zotero-sqlx-prepare: remove the GCC 16 fallback" pkgs.gcc;
in
  pkgs.writeShellApplication {
    name = "zotero-sqlx-prepare";
    runtimeInputs = with pkgs; [
      gcc
      mariadb
      pkg-config
      sqlx-cli
    ];
    text = ''
      portal_directory="$PWD/pkgs/zotero-portal"
      if [[ ! -f "$portal_directory/Cargo.toml" \
        || ! -d "$portal_directory/migrations" \
        || ! -d "$portal_directory/queries" ]]; then
        echo "run this app from the zotero-server-nix repository root" >&2
        exit 1
      fi

      temporary_directory="$(mktemp -d /tmp/zotero-sqlx-prepare.XXXXXXXX)"
      data_directory="$temporary_directory/data"
      socket="$temporary_directory/mariadb.sock"
      mariadb_pid=

      cleanup() {
        if [[ -n "$mariadb_pid" ]]; then
          mariadb-admin --no-defaults --user=root --socket="$socket" shutdown >/dev/null 2>&1 \
            || kill "$mariadb_pid" >/dev/null 2>&1 \
            || true
          wait "$mariadb_pid" 2>/dev/null || true
        fi
        rm -rf -- "$temporary_directory"
      }
      trap cleanup EXIT

      mariadb-install-db \
        --no-defaults \
        --auth-root-authentication-method=normal \
        --basedir=${pkgs.mariadb} \
        --datadir="$data_directory" \
        --skip-test-db >/dev/null
      mariadbd \
        --no-defaults \
        --datadir="$data_directory" \
        --socket="$socket" \
        --pid-file="$temporary_directory/mariadb.pid" \
        --log-error="$temporary_directory/mariadb.log" \
        --skip-networking &
      mariadb_pid=$!

      for _ in {1..100}; do
        if mariadb-admin --no-defaults --user=root --socket="$socket" ping >/dev/null 2>&1; then
          break
        fi
        if ! kill -0 "$mariadb_pid" 2>/dev/null; then
          cat "$temporary_directory/mariadb.log" >&2
          exit 1
        fi
        sleep 0.1
      done
      mariadb-admin --no-defaults --user=root --socket="$socket" ping >/dev/null

      mariadb --no-defaults --user=root --socket="$socket" --execute='CREATE DATABASE zotero_www'
      encoded_socket="''${socket//\//%2F}"
      database_url="mysql://root@localhost/zotero_www?socket=$encoded_socket"

      sqlx migrate run \
        --source "$portal_directory/migrations" \
        --database-url "$database_url"

      unset SQLX_OFFLINE
      export TAILWIND_CLI=${getExe pkgs.tailwindcss_4}
      cd "$portal_directory"
      cargo sqlx prepare \
        --no-dotenv \
        --database-url "$database_url"
    '';
  }
