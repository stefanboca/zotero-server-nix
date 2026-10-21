{
  fetchFromGitHub,
  lib,
  lndir,
  nix-update-script,
  php,
  runCommandLocal,
  ...
}:
php.buildComposerProject2 (finalAttrs: {
  pname = "zotero-dataserver";
  version = "0-unstable-2026-09-20";

  src = fetchFromGitHub {
    owner = "zotero";
    repo = "dataserver";
    rev = "6e091852b8a3e369056f390ba69e5de90cda1134";
    hash = "sha256-9MC/VM/LEMjkb+k1EHw8cvYKzdFgHgXaj7Tf1ENXIo8=";
    fetchSubmodules = true;
  };

  zf1 = fetchFromGitHub {
    owner = "zendframework";
    repo = "zf1";
    tag = "release-1.12.20";
    hash = "sha256-EP6v1397v/qTlczo7XFO7Kyesl/k2ddla5JNj+BmN6s=";
  };

  php = php.withExtensions ({
    all,
    enabled,
  }:
    enabled ++ (with all; [memcached redis]));

  patches = [
    ./0001-tmp-path.patch
    ./0002-custom-s3-endpoint.patch
    ./0003-mariadb-upsert.patch
    ./0004-zero-unlimited-storage.patch
    ./0005-login-path.patch
    ./0006-local-fulltext-index.patch
    ./0007-web-ui-api-keys.patch
    ./0008-web-ui-key-lookup.patch
    ./0009-custom-base-uri-relations.patch
    ./0010-php-memcached-serializer.patch
  ];

  composerStrictValidation = false;
  vendorHash = "sha256-/QNcnQjCKoYwxuGUWLNDdIkZBO/S42+J0SKWcIbm41w=";

  # Composer does not accept nix-update's 0-unstable-YYYY-MM-DD version as
  # COMPOSER_ROOT_VERSION. The date alone is a valid Composer version.
  composerVendor = php.mkComposerVendor {
    inherit (finalAttrs) pname src vendorHash;
    version = lib.removePrefix "0-unstable-" finalAttrs.version;
    inherit (finalAttrs) php;
    composerStrictValidation = false;
  };

  # The project derivation's install hook sets COMPOSER_ROOT_VERSION from the
  # Nix package version again, so override it after that hook has run.
  postConfigure = ''
    export COMPOSER_ROOT_VERSION=${lib.removePrefix "0-unstable-" finalAttrs.version}
  '';

  preBuild = ''
    cp -a $zf1/library/Zend/. ./include/Zend
  '';

  passthru = {
    updateScript = nix-update-script {
      extraArgs = ["--flake" "--version=branch=master"];
    };

    withConfig = {
      config,
      dbconnect,
    }: let
      unconfigured = finalAttrs.finalPackage;
      app = "share/php/${finalAttrs.pname}";
    in
      runCommandLocal "${finalAttrs.pname}-configured-${finalAttrs.version}" {
        inherit config dbconnect;
        passAsFile = ["config" "dbconnect"];

        nativeBuildInputs = [lndir];

        inherit (unconfigured) meta;
        passthru =
          (unconfigured.passthru or {})
          // {
            inherit (unconfigured) php;
            inherit unconfigured;
          };
      } ''
        mkdir -p "$out"
        lndir -silent ${unconfigured} "$out"

        rm -f \
          "$out/${app}/include/config/config.inc.php" \
          "$out/${app}/include/config/dbconnect.inc.php"

        install -m 0444 "$configPath" \
          "$out/${app}/include/config/config.inc.php"
        install -m 0444 "$dbconnectPath" \
          "$out/${app}/include/config/dbconnect.inc.php"

        # CLI startup derives Z_ENV_BASE_PATH from header.inc.php's resolved
        # path, so this file must not remain a symlink to the unconfigured tree.
        rm "$out/${app}/include/header.inc.php"
        install -m 0444 \
          ${unconfigured}/${app}/include/header.inc.php \
          "$out/${app}/include/header.inc.php"
      '';
  };

  meta = {
    description = "Zotero Data Server";
    homepage = finalAttrs.src.url;
    license = lib.licenses.agpl3Only;
  };
})
