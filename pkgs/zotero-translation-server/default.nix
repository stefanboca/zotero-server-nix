{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-translation-server";
  version = "2.0.3-unstable-2026-04-28";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "translation-server";
    rev = "d88a8d5384456439962edfef129b14841b09af6d";
    hash = "sha256-2W4tg0eP6kI0rWtn8CQ7gyfVHty6VdQje2UUvsnqEdQ=";
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
