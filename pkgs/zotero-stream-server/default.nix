{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  nodejs,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-stream-server";
  version = "0.1.0+date=2026-03-24";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "stream-server";
    rev = "55cb51c85f6787261d878529aaa440cbc08b8f15";
    hash = "sha256-hVDdCwJaOrlpdD43ZXvpqk1NCOjD0dV6FpgbOUnA0Xw=";
  };

  npmDepsHash = "sha256-sIasdXvj+vZidDDaVdb+sdhE58dW2p36MUoyIs7/lMY=";

  dontBuild = true;

  postInstall = ''
    mkdir -p $out/bin
    cat > $out/bin/${finalAttrs.meta.mainProgram} <<EOF
    #!${nodejs}/bin/node
    require("$out/lib/node_modules/zotero-stream-server/index.js")
    EOF
    chmod +x $out/bin/${finalAttrs.meta.mainProgram}
  '';

  meta = {
    description = "Zotero Streaming API Server";
    homepage = finalAttrs.src.url;
    mainProgram = finalAttrs.pname;
    license = lib.licenses.agpl3Only;
  };
})
