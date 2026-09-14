{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  nix-update-script,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-stream-server";
  version = "0-unstable-2026-09-02";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "stream-server";
    rev = "9dc1725f71f816a2821c6d8985bc88729b7a4947";
    hash = "sha256-j+Zxb6RHfQD7WTDH103yHKVtkJc4boJHJVAS3iT7TW4=";
  };

  patches = [
    ./0001-add-bin.patch
    ./0002-configurable-listen-address.patch
    ./0003-disable-status-logging.patch
  ];

  npmDepsHash = "sha256-sIasdXvj+vZidDDaVdb+sdhE58dW2p36MUoyIs7/lMY=";

  dontNpmBuild = true;

  passthru.updateScript = nix-update-script {
    extraArgs = ["--flake" "--version=branch=master"];
  };

  meta = {
    description = "Zotero Streaming API Server";
    homepage = finalAttrs.src.url;
    mainProgram = "zotero-stream-server";
    license = lib.licenses.agpl3Only;
  };
})
