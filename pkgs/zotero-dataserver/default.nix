{
  fetchFromGitHub,
  php,
  ...
}:
php.buildComposerProject2 {
  pname = "zotero-dataserver";
  version = "2026-04-10";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "dataserver";
    rev = "62bac4504da9d264458809f812c27a9837ada1a2";
    hash = "sha256-Q/meymrx89TvYTG1vBSQ1ERqLFLFr9G4BrsBk/HNk/4=";
  };

  composerStrictValidation = false;
  vendorHash = "sha256-vpv7yqQ9AJCBlyjun1dfNpd+qz9cRJSIuzErOcjAG1U=";
}
