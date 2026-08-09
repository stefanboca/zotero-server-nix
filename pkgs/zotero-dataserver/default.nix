{
  fetchFromGitHub,
  lib,
  lndir,
  php,
  runCommandLocal,
  ...
}: let
  # TODO: find a better way to do this and/or upstream it
  igbinary = let
    prev = php.extensions.igbinary;
  in
    php.buildPecl {
      inherit (prev) pname version sha256 configureFlags makeFlags outputs meta;

      # override install phase in order to install headers to the correct directory
      installPhase = ''
        runHook preInstall

        install -Dm644 modules/igbinary.so -t $out/lib/php/extensions
        find . -type f -name '*.h' -exec install -Dm644 {} "$dev/include/{}" \;

        runHook postInstall
      '';
    };
  # upstream memcached doesn't support igbinary
  memcached = let
    prev = php.extensions.memcached;
  in
    php.buildPecl {
      inherit (prev) pname version src nativeBuildInputs buildInputs meta;
      internalDeps = prev.internalDeps ++ [igbinary];
      configureFlags = prev.configureFlags ++ ["--enable-memcached-igbinary"];
    };
in
  php.buildComposerProject2 (finalAttrs: {
    pname = "zotero-dataserver";
    version = "2026-08-02";

    src = fetchFromGitHub {
      owner = "zotero";
      repo = "dataserver";
      rev = "5cf550f3166e848981a8ec60696ae3a6a5d82bc7";
      hash = "sha256-20vNlTrc915lnIwUpm/CwbW50Xw2/RrwBCRhvqXl4aU=";
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
      enabled
      ++ [
        memcached
        all.redis
      ]);

    patches = [
      ./0001-tmp-path.patch
      ./0002-custom-s3-endpoint.patch
      ./0003-mariadb-upsert.patch
      ./0004-zero-unlimited-storage.patch
      ./0005-login-path.patch
      ./0006-local-fulltext-index.patch
    ];

    composerStrictValidation = false;
    vendorHash = "sha256-IZtXmv2xKEpfjHFi6rp+lZvzHAsMN1vWOWqb/DH5ViY=";

    preBuild = ''
      cp -a $zf1/library/Zend/. ./include/Zend
    '';

    passthru = {
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
