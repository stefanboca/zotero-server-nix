{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  nix-update-script,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-citeproc-js-server";
  version = "0-unstable-2026-02-14";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "citeproc-js-server";
    rev = "a2d88b800a9f4be9246a889e0ba49dbfd0c0142a";
    hash = "sha256-5SjF+L7xAeWydq4lOyy6CbDstfq3zD1zufRUVI4HFeo=";
    fetchSubmodules = true;
  };

  patches = [
    ./0001-add-bin.patch
    ./0002-configurable-listen-address.patch
  ];

  npmDepsHash = "sha256-j4ePfSOaG1eO/2KovH2Z4cd/7G8t6TnoP43zBtftOU0=";

  dontNpmBuild = true;

  passthru.updateScript = nix-update-script {
    extraArgs = ["--flake" "--version=branch=master"];
  };

  meta = {
    description = "Web service to generate citations and bibliographies using citeproc-js";
    homepage = finalAttrs.src.url;
    mainProgram = "citeproc-js-server";
    license = lib.licenses.agpl3Only;
  };
})
