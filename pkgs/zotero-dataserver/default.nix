{
  cyrus_sasl,
  fetchFromGitHub,
  lib,
  libmemcached,
  php,
  pkg-config,
  rsync,
  zlib,
  ...
}: let
  # TODO: find a better way to do this and/or upstream it
  igbinary = php.buildPecl {
    pname = "igbinary";
    version = "3.2.14";
    sha256 = "sha256-YzcUek+4iAclZmdIN72pko7gbufwEUtDOLhsgWIykl0=";

    configureFlags = ["--enable-igbinary"];
    makeFlags = ["phpincludedir=$(dev)/include"];
    outputs = [
      "out"
      "dev"
    ];

    # override install phase in order to install headers to the correct directory
    installPhase = ''
      runHook preInstall

      mkdir -p $out/lib/php/extensions
      cp modules/igbinary.so $out/lib/php/extensions/igbinary.so
      mkdir -p $dev/include
      ${rsync}/bin/rsync -r --filter="+ */" \
                            --filter="+ *.h" \
                            --filter="- *" \
                            --prune-empty-dirs \
                            . $dev/include/

      runHook postInstall
    '';

    meta = {
      description = "Binary serialization for PHP";
      homepage = "https://github.com/igbinary/igbinary/";
      license = lib.licenses.bsd3;
      teams = [lib.teams.php];
      broken = lib.versionAtLeast php.version "8.5";
    };
  };
  # upstream memcached doesn't support igbinary
  memcached = php.buildPecl rec {
    pname = "memcached";
    version = "3.4.0";

    src = fetchFromGitHub {
      owner = "php-memcached-dev";
      repo = "php-memcached";
      rev = "v${version}";
      sha256 = "sha256-sweEM4TVId+6ySffulmebZpz390dZXb+G3zFZvc45L8=";
    };

    internalDeps = [php.extensions.session igbinary];

    configureFlags = [
      "--with-zlib-dir=${zlib.dev}"
      "--with-libmemcached-dir=${libmemcached}"
      "--enable-memcached-igbinary"
    ];

    nativeBuildInputs = [pkg-config];
    buildInputs = [
      cyrus_sasl
      zlib
    ];

    meta = {
      description = "PHP extension for interfacing with memcached via libmemcached library";
      license = lib.licenses.php301;
      homepage = "https://github.com/php-memcached-dev/php-memcached";
      teams = [lib.teams.php];
    };
  };
in
  php.buildComposerProject2 (finalAttrs: {
    pname = "zotero-dataserver";
    version = "2026-05-04";

    src = fetchFromGitHub {
      owner = "zotero";
      repo = "dataserver";
      rev = "f95398d2e9b56751165c97b3d7c8980fd2f0024e";
      hash = "sha256-arF6LAiCNTMieHXRJiISWUhRuVlUq90VqhthWWizGIc=";
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

    patches = [./0001-tmp-path.patch];

    composerStrictValidation = false;
    vendorHash = "sha256-acWdOQ2UyaUFbMCgdy1dD7BIFFpBInK6FDeYStFm1Mc=";

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
