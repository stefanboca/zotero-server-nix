{
  lib,
  rustPlatform,
  tailwindcss_4,
  topcoat-cli,
}:
rustPlatform.buildRustPackage {
  pname = "zotero-auth";
  version = "0.1.0";

  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [tailwindcss_4 topcoat-cli];
  env.TAILWIND_CLI = lib.getExe tailwindcss_4;

  # Topcoat's asset IDs include the build output path. Preserve and install
  # the exact executable that the Topcoat bundler scanned.
  postBuild = ''
    topcoat asset bundle --release --bin zotero-auth --out target/assets
    cp target/release/zotero-auth target/zotero-auth-with-assets
  '';

  postInstall = ''
    install -m755 target/zotero-auth-with-assets $out/bin/zotero-auth
    mkdir -p $out/bin/assets
    cp -r target/assets/. $out/bin/assets/
  '';

  meta = {
    description = "OIDC login portal for Zotero Dataserver";
    license = lib.licenses.agpl3Only;
    mainProgram = "zotero-auth";
  };
}
