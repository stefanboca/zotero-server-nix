{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-translation-server";
  version = "2.0.3-unstable-2026-07-31";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "translation-server";
    rev = "3a9d17614896fc1fea73d7b880ea79273b605275";
    hash = "sha256-3gcWNl3IivSuMcySD4GoGBnLEepbw2XKTiOHAiMyzwk=";
    fetchSubmodules = true;
  };

  patches = [./0001-add-bin.patch];

  npmDepsFetcherVersion = 2;
  npmDepsHash = "sha256-a/Qlzn5Rs1PoFdh1zEm2gDUbcYvAgbDCzP/LhDX4Gds=";

  dontNpmBuild = true;

  meta = {
    description = "A Node.js-based server to run Zotero translators";
    homepage = finalAttrs.src.url;
    mainProgram = "translation-server";
    license = lib.licenses.agpl3Only;
  };
})
