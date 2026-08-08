{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-attachment-proxy";
  version = "0-unstable-2025-05-21";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "attachment-proxy";
    rev = "58bd8dd561442c7f564ab5b8f41b418a1a32be2a";
    hash = "sha256-AMQPEPpdbZgtNrCMKWGXhh0YB+n1huhk34bf/gKT6Sg=";
  };

  patches = [
    ./0001-add-bin.patch
    ./0002-custom-s3-endpoint.patch
  ];

  npmDepsHash = "sha256-auGS3ssa3QgO3J7Q54rj15itFjcnvjCs5KPSu8GkrkU=";

  dontNpmBuild = true;

  meta = {
    description = "Proxy for Zotero attachment uploads and downloads";
    homepage = finalAttrs.src.url;
    mainProgram = "attachment-proxy";
    license = lib.licenses.agpl3Only;
  };
})
