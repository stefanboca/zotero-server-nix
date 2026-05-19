{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  nodejs_20,
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

  patches = [./0001-add-bin.patch];

  npmDepsHash = "sha256-auGS3ssa3QgO3J7Q54rj15itFjcnvjCs5KPSu8GkrkU=";

  nodejs = nodejs_20;

  dontNpmBuild = true;

  meta = {
    description = "Web service to generate citations and bibliographies using citeproc-js";
    homepage = finalAttrs.src.url;
    mainProgram = "attachment-proxy";
    license = lib.licenses.agpl3Only;
  };
})
