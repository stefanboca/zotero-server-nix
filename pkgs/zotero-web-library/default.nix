{
  buildNpmPackage,
  buildRubyGem,
  fetchFromGitHub,
  fetchurl,
  lib,
  ruby,
  rubyPackages,
  stdenvNoCC,
  unzip,
  zotero,
}: let
  namae = buildRubyGem {
    inherit ruby;
    gemName = "namae";
    pname = "ruby-namae";
    version = "1.2.0";
    source.sha256 = "sha256-NUHOSwhv1JgdI3ZjDAPihEAr/hzbq05Q4iIqcq651Z0=";
    propagatedBuildInputs = [rubyPackages.racc];
  };

  csl = buildRubyGem {
    inherit ruby;
    gemName = "csl";
    pname = "ruby-csl";
    version = "2.2.1";
    source.sha256 = "sha256-YkNS8fb1Hx4LFVvSUBzkI/AyZElewlnmXCbgCHKhlaI=";
    propagatedBuildInputs = [
      namae
      rubyPackages.forwardable
      rubyPackages.rexml
      rubyPackages.singleton
    ];
  };
in
  buildNpmPackage (finalAttrs: {
    pname = "zotero-web-library";
    version = "1.8.2-unstable-2026-09-11";

    src = fetchFromGitHub {
      owner = "zotero";
      repo = "web-library";
      rev = "9f6cf7923fbab6f55da46586793ab0a2f9a2dc84";
      fetchSubmodules = true;
      hash = "sha256-J7wi6SlYhPnisdi0MH9AFkdaHhuRL85+e5LuFeh16e8=";
    };

    npmDepsHash = "sha256-OgmZ9aUQZDy0BFOuveW/t0u0Ng0syK7lDzOi+vi3di4=";

    patches = [
      ./0001-default-missing-tag-colors-to-empty.patch
      ./0002-use-configured-api-for-item-templates.patch
      ./0004-use-configured-buy-storage-url.patch
      ./0005-remove-myfonts-tracking-import.patch
      ./0006-format-partial-write-errors.patch
    ];

    nativeBuildInputs = [csl ruby unzip];

    env = {
      release = fetchurl {
        url = "https://github.com/zotero/web-library/releases/download/v${finalAttrs.version}/web-library-v${finalAttrs.version}.zip";
        hash = "sha256-4Sg8erq859DmJoMdIsMz0eh/ZXh/MYgQiJpFzBSRvvU=";
      };

      citationStyles = stdenvNoCC.mkDerivation {
        pname = "citation-styles";
        version = "0.2.210-unstable-2026-09-14";

        src = fetchFromGitHub {
          owner = "citation-style-language";
          repo = "styles";
          rev = "90e657c8622ec1c6f27788e041b355efac659397";
          hash = "sha256-M+oPFo7LUYvith1e+DvsCpc/yt+1vBZs6oLfZjGQmXM=";
        };

        installPhase = ''
          cp -r . "$out"
        '';
      };
    };

    preBuild = ''
      mkdir -p \
        data \
        src/static/fonts \
        src/static/js \
        src/static/note-editor \
        src/static/pdf-worker \
        src/static/reader \
        src/static/styles

      unzip -q $release -d release
      releaseStatic=release/web-library-v${finalAttrs.version}/static/web-library
      cp -r $releaseStatic/reader/. src/static/reader/
      cp -r $releaseStatic/pdf-worker/. src/static/pdf-worker/
      cp -r $releaseStatic/note-editor/. src/static/note-editor/
      cp $releaseStatic/fonts/36AC02_5_0.* src/static/fonts/
      cp ${zotero.src}/chrome/content/zotero/xpcom/citeproc.js citeproc.js
      cp $citationStyles/*.csl $citationStyles/dependent/*.csl src/static/styles/
      ruby ${./generate-styles-json.rb} $citationStyles data/styles.json
      cp data/styles.json src/static/styles/styles.json
      npx terser citeproc.js --output src/static/js/citeproc.js
    '';

    buildPhase = ''
      runHook preBuild

      npm run build:version
      npm run build:locale
      STYLES_CACHE_TIME=9999999999999 npm run build:styles-json
      npm run build:check-icons

      NODE_ENV=production npm run build:js
      NODE_ENV=production npm run build:scss
      npm run build:static
      npx postcss build/static/web-library/zotero-web-library.css --no-map -r
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall

      mkdir -p $out/share/zotero-web-library
      cp -r build/static/web-library $out/share/zotero-web-library/static

      runHook postInstall
    '';

    passthru.updateScript = [./update.sh];

    meta = {
      description = "Zotero Web Library packaged for Zotero Local Cloud";
      homepage = "https://github.com/zotero/web-library";
      license = lib.licenses.agpl3Only;
    };
  })
