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
        (fileFilter (file: file.hasExt "rs" || file.hasExt "css" || file.hasExt "js") ./.)
        (fileFilter (file: hasPrefix "Cargo" file.name) ./.)
        ./.sqlx
        ./migrations
      ];
    };

    cargoLock.lockFile = ./Cargo.lock;
    nativeBuildInputs = [tailwindcss_4 topcoat-cli];
    env.SQLX_OFFLINE = true;
    env.TAILWIND_CLI = getExe tailwindcss_4;

    # Scan the artifact built by buildRustPackage instead of making Topcoat run
    # a second Cargo build.
    postBuild = ''
      topcoat asset bundle --artifact "target/${stdenv.targetPlatform.rust.cargoShortTarget}/$cargoBuildType/zotero-portal"
    '';

    postInstall = ''
      mkdir -p $out/bin/assets
      cp -r target/assets/. $out/bin/assets/
    '';

    meta = {
      description = "Local account and administration portal for Zotero Dataserver";
      license = lib.licenses.agpl3Only;
      mainProgram = "zotero-portal";
    };
  }
