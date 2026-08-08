{
  lib,
  makeRustPlatform,
  rust-bin,
  rustPlatform,
  rustc,
  tailwindcss_4,
  topcoat-cli,
}: let
  toolchain = rust-bin.beta.latest.default;
  buildRustPlatform =
    if lib.versionOlder rustc.version "1.95"
    then
      makeRustPlatform {
        cargo = toolchain;
        rustc = toolchain;
      }
    else
      lib.warn
      "zotero-auth: nixpkgs Rust ${rustc.version} satisfies Topcoat's 1.95 MSRV; remove the rust-overlay workaround"
      rustPlatform;
in
  buildRustPlatform.buildRustPackage {
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
