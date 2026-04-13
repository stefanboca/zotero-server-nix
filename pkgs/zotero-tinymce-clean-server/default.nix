{
  buildNpmPackage,
  fetchFromGitHub,
  lib,
  nodejs,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-tinymce-clean-server";
  version = "1.0.0+date=2017-05-29";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "tinymce-clean-server";
    rev = "5be2a0d367133b728872bee9975beed1c9e74898";
    hash = "sha256-EX466/T8OBFf3tw3SYhWdyA4D6W7hn90M7ZwaPvI30M=";
  };

  npmDepsHash = "sha256-j1mu81uwqCJZ3VWFd4uOOuG2M1ziIE4KFDM8cqQEqQg=";

  dontBuild = true;

  postPatch = ''
    cp ${./package-lock.json} package-lock.json
  '';

  postInstall = ''
    mkdir -p $out/bin
    cat > $out/bin/${finalAttrs.meta.mainProgram} <<EOF
    #!${nodejs}/bin/node
    require("$out/lib/node_modules/tinymce-clean-server/server.js")
    EOF
    chmod +x $out/bin/${finalAttrs.meta.mainProgram}
  '';

  meta = {
    description = "Node.js HTTP server to clean HTML using TinyMCE parsing/filtering logic";
    homepage = finalAttrs.src.url;
    mainProgram = finalAttrs.pname;
  };
})
