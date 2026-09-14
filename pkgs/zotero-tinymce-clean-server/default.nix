{
  buildNpmPackage,
  fetchFromGitHub,
  nix-update-script,
  ...
}:
buildNpmPackage (finalAttrs: {
  pname = "zotero-tinymce-clean-server";
  version = "0-unstable-2017-06-29";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "tinymce-clean-server";
    rev = "5be2a0d367133b728872bee9975beed1c9e74898";
    hash = "sha256-EX466/T8OBFf3tw3SYhWdyA4D6W7hn90M7ZwaPvI30M=";
  };

  patches = [./0001-add-bin.patch ./0002-package-lock.json.patch];

  npmDepsHash = "sha256-j1mu81uwqCJZ3VWFd4uOOuG2M1ziIE4KFDM8cqQEqQg=";

  dontNpmBuild = true;

  passthru.updateScript = nix-update-script {
    extraArgs = ["--flake" "--version=branch=master"];
  };

  meta = {
    description = "Node.js HTTP server to clean HTML using TinyMCE parsing/filtering logic";
    homepage = finalAttrs.src.url;
    mainProgram = "tinymce-clean-server";
  };
})
