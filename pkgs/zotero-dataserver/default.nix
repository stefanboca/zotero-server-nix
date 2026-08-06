{
  fetchFromGitHub,
  lib,
  php,
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
      }:
        finalAttrs.finalPackage.overrideAttrs {
          inherit config dbconnect;
          passAsFile = ["config" "dbconnect"];
          postPatch = ''
            cp $configPath include/config/config.inc.php
            cp $dbconnectPath include/config/dbconnect.inc.php
          '';
        };
    };

    meta = {
      description = "Zotero Data Server";
      homepage = finalAttrs.src.url;
      license = lib.licenses.agpl3Only;
    };
  })
