{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-translation-server";
  version = "2.0.5+date=2026-03-03";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "translation-server";
    rev = "2fce0493723f3523094dead78e7e897e14a070c6";
    hash = "sha256-FiX6ZfQQkFBihI+DAbzhE2I+4xrtD+OSmN9NUa7zKjc=";
    fetchSubmodules = true;
  };

  patches = [./0001-add-bin.patch];

  npmDepsFetcherVersion = 2;
  npmDepsHash = "sha256-pU7CZ4Cx9fu/Z/cBCCwewLQZDne+LPooZCsJIDXP6bI=";

  dontNpmBuild = true;

  meta = {
    description = "A Node.js-based server to run Zotero translators";
    homepage = finalAttrs.src.url;
    mainProgram = "translation-server";
    license = lib.licenses.agpl3Only;
  };
})
