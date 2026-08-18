{
  fetchCrate,
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage rec {
  pname = "topcoat-cli";
  version = "0.6.0";

  src = fetchCrate {
    inherit pname version;
    hash = "sha256-b1Jz6n2pJN6vZV4MVfdpHxrr8PYKap2PswIjZDG31PA=";
  };

  cargoHash = "sha256-e+R11tyG9NE7ma9jKQTWT/26eLv4N/eNA6QBIe7puQU=";
  patches = [./bundle-existing-artifact.patch];

  # The published crate has a doctest that imports its workspace-only
  # `topcoat` development dependency.
  doCheck = false;

  meta = {
    description = "Command-line development tools for Topcoat";
    homepage = "https://github.com/tokio-rs/topcoat";
    license = lib.licenses.mit;
    mainProgram = "topcoat";
  };
}
