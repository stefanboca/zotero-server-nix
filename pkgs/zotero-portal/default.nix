{
  lib,
  rustPlatform,
  stdenv,
  tailwindcss_4,
  topcoat-cli,
}: let
  inherit (lib.fileset) fileFilter toSource unions;
  inherit (lib.meta) getExe;
  inherit (lib.strings) hasPrefix;
in
  rustPlatform.buildRustPackage {
    pname = "zotero-portal";
    version = "0.1.0";

    src = toSource {
      root = ./.;
      fileset = unions [
        (fileFilter (file: file.hasExt "rs" || file.hasExt "css" || file.hasExt "js" || file.hasExt "svg") ./.)
        (fileFilter (file: hasPrefix "Cargo" file.name) ./.)
        ./.sqlx
        ./migrations
        ./queries
        ./secretspec.toml
      ];
    };

    cargoLock.lockFile = ./Cargo.lock;

    nativeBuildInputs = [tailwindcss_4 topcoat-cli];

    env = {
      SQLX_OFFLINE = true;
      TAILWIND_CLI = getExe tailwindcss_4;
    };

    postBuild = ''
      CARGO_BUILD_TARGET="${stdenv.hostPlatform.rust.rustcTarget}" topcoat asset bundle --release --bin zotero-portal --out $out/bin/assets
    '';

    postInstall = ''
      install -Dm644 secretspec.toml $out/share/zotero-portal/secretspec.toml
    '';

    meta = {
      description = "Local account and administration portal for Zotero Dataserver";
      license = lib.licenses.agpl3Only;
      mainProgram = "zotero-portal";
    };
  }
