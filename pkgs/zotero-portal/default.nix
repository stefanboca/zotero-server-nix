{
  lib,
  rustPlatform,
  tailwindcss_4,
  topcoat-cli,
}:
rustPlatform.buildRustPackage {
  pname = "zotero-portal";
  version = "0.1.0";

  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [tailwindcss_4 topcoat-cli];
  env.TAILWIND_CLI = lib.getExe tailwindcss_4;

  # Topcoat's asset IDs include the build output path. Preserve and install
  # the exact executable that the Topcoat bundler scanned.
  postBuild = ''
    topcoat asset bundle --release --bin zotero-portal --out target/assets
    cp target/release/zotero-portal target/zotero-portal-with-assets
  '';

  postInstall = ''
    install -m755 target/zotero-portal-with-assets $out/bin/zotero-portal
    mkdir -p $out/bin/assets
    cp -r target/assets/. $out/bin/assets/
  '';

  meta = {
    description = "Local account and administration portal for Zotero Dataserver";
    license = lib.licenses.agpl3Only;
    mainProgram = "zotero-portal";
  };
}
