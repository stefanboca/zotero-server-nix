{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-stream-server";
  version = "0-unstable-2026-03-24";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "stream-server";
    rev = "55cb51c85f6787261d878529aaa440cbc08b8f15";
    hash = "sha256-hVDdCwJaOrlpdD43ZXvpqk1NCOjD0dV6FpgbOUnA0Xw=";
  };

  patches = [
    ./0001-add-bin.patch
    ./0002-configurable-listen-address.patch
    ./0003-disable-status-logging.patch
  ];

  npmDepsHash = "sha256-sIasdXvj+vZidDDaVdb+sdhE58dW2p36MUoyIs7/lMY=";

  dontNpmBuild = true;

  meta = {
    description = "Zotero Streaming API Server";
    homepage = finalAttrs.src.url;
    mainProgram = "zotero-stream-server";
    license = lib.licenses.agpl3Only;
  };
})
