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
    TAILWIND_CLI = lib.getExe tailwindcss_4;

    postBuild = ''
      topcoat asset bundle --release --bin zotero-auth --out target/assets
    '';

    postInstall = ''
      mkdir -p $out/bin/assets
      cp -r target/assets/. $out/bin/assets/
    '';

    meta = {
      description = "OIDC login portal for Zotero Dataserver";
      license = lib.licenses.agpl3Only;
      mainProgram = "zotero-auth";
    };
  }
