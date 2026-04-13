{
  fetchFromGitHub,
  lib,
  php,
  ...
}:
php.buildComposerProject2 (finalAttrs: {
  pname = "zotero-dataserver";
  version = "2026-04-10";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "dataserver";
    rev = "62bac4504da9d264458809f812c27a9837ada1a2";
    hash = "";
    fetchSubmodules = true;
  };

  composerStrictValidation = false;
  vendorHash = "sha256-vpv7yqQ9AJCBlyjun1dfNpd+qz9cRJSIuzErOcjAG1U=";

  passthru = {
    inherit php;
  };

  meta = {
    description = "Zotero Data Server";
    homepage = finalAttrs.src.url;
    license = lib.licenses.agpl3Only;
  };
})
