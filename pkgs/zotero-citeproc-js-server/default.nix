{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  nodejs,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-citeproc-js-server";
  version = "2.0.0+date=2026-02-14";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "citeproc-js-server";
    rev = "a2d88b800a9f4be9246a889e0ba49dbfd0c0142a";
    hash = "sha256-5SjF+L7xAeWydq4lOyy6CbDstfq3zD1zufRUVI4HFeo=";
    fetchSubmodules = true;
  };

  npmDepsHash = "sha256-j4ePfSOaG1eO/2KovH2Z4cd/7G8t6TnoP43zBtftOU0=";

  dontBuild = true;

  postInstall = ''
    mkdir -p $out/bin
    cat > $out/bin/${finalAttrs.meta.mainProgram} <<EOF
    #!${nodejs}/bin/node
    require("$out/lib/node_modules/citeproc-js-server/lib/citeServer.js")
    EOF
    chmod +x $out/bin/${finalAttrs.meta.mainProgram}
  '';

  meta = {
    description = "Web service to generate citations and bibliographies using citeproc-js";
    homepage = finalAttrs.src.url;
    mainProgram = finalAttrs.pname;
    license = lib.licenses.agpl3Only;
  };
})
