{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  nodejs,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-translation-server";
  version = "2.0.5+date=2026-03-03";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "translation-server";
    rev = "2fce0493723f3523094dead78e7e897e14a070c6";
    hash = "sha256-BN3rhScI6bYz7un+ESVKCY2niXgw2JzYTpDVYIG1JA0=";
  };

  npmDepsFetcherVersion = 2;
  npmDepsHash = "sha256-pU7CZ4Cx9fu/Z/cBCCwewLQZDne+LPooZCsJIDXP6bI=";

  dontBuild = true;

  postInstall = ''
    mkdir -p $out/bin
    cat > $out/bin/${finalAttrs.meta.mainProgram} <<EOF
    #!${nodejs}/bin/node
    require("$out/lib/node_modules/translation-server/src/server.js")
    EOF
    chmod +x $out/bin/${finalAttrs.meta.mainProgram}
  '';

  meta = {
    description = "A Node.js-based server to run Zotero translators";
    homepage = finalAttrs.src.url;
    mainProgram = finalAttrs.pname;
    license = lib.licenses.agpl3Only;
  };
})
