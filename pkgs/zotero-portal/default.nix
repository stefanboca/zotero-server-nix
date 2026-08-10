{
  lib,
  rustPlatform,
  stdenv,
  tailwindcss_4,
  topcoat-cli,
  zotero-web-library,
}:
rustPlatform.buildRustPackage {
  pname = "zotero-portal";
  version = "0.1.0";

  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [tailwindcss_4 topcoat-cli];
  env = {
    TAILWIND_CLI = lib.getExe tailwindcss_4;
    ZOTERO_WEB_LIBRARY = zotero-web-library;
  };

  # Scan the artifact built by buildRustPackage instead of making Topcoat run
  # a second Cargo build.
  postBuild = ''
    topcoat asset bundle --artifact "target/${stdenv.targetPlatform.rust.cargoShortTarget}/$cargoBuildType/zotero-portal"
  '';

  postInstall = ''
    mkdir -p $out/bin/assets
    cp -r target/assets/. $out/bin/assets/
    install -Dm644 ${zotero-web-library}/share/zotero-web-library/static/icons/32/z.svg $out/share/zotero-portal/z.svg
  '';

  meta = {
    description = "Local account and administration portal for Zotero Dataserver";
    license = lib.licenses.agpl3Only;
    mainProgram = "zotero-portal";
  };
}
