{
  buildNpmPackage,
  fetchFromGitHub,
  fetchurl,
  fetchzip,
  lib,
  unzip,
  zotero,
}: let
  rev = "4f81cc1e4e03b272f6f360ff260d8c1571dd721e";

  readerRev = "f7a50ff838429602d21368f3f7ef186348782cfa";
  reader = fetchzip {
    url = "https://zotero-download.s3.amazonaws.com/ci/reader/${readerRev}.zip";
    hash = "sha256-tiNV5DIxEzNMV3zdXQjHwQqykLUIpRLV11KOtS2aNBg=";
    stripRoot = false;
  };

  pdfWorkerRev = "8e9577ec37dc4d5e74aea477d06090b4ead0c1ca";
  pdfWorker = fetchzip {
    url = "https://zotero-download.s3.amazonaws.com/ci/document-worker/${pdfWorkerRev}.zip";
    hash = "sha256-jrwcTkqLlEXYw0Ib0rqglfA85nks6N4s0logVqzGg2c=";
    stripRoot = false;
  };

  noteEditorRev = "ef34da0cddab05495008cf4f95d91c759003488c";
  noteEditor = fetchzip {
    url = "https://zotero-download.s3.amazonaws.com/ci/note-editor/${noteEditorRev}.zip";
    hash = "sha256-6brZ/RlcgQ25M/3Zq+PK/W3ruGIQiatsQ9L2GG+VmUM=";
    stripRoot = false;
  };

  fontHashes = {
    eot = "sha256-M28WViLFCV9TGBJhn4YIBweP9E45QisBcVGnyMhjtcQ=";
    svg = "sha256-SRFd/0fdlssrtcxXN+4kqaZwgoxXgq6bEdBQl1tFLxY=";
    ttf = "sha256-Z6fRcnU1peWvG3eczKS5kkjfDNqBLzBhucNgmHXDNHA=";
    woff = "sha256-w/W+HiOvQahFdJJE2+ZJ3IKdbXDPygnhREUaCrDUqQI=";
    woff2 = "sha256-DM7WI1eflHaSAAD1uKhflZFe9NiUgBbS0CyJlW4ck3w=";
  };
  font = extension:
    fetchurl {
      url = "https://www.zotero.org/static/web-library/fonts/36AC02_5_0.${extension}";
      hash = fontHashes.${extension};
    };

  websiteFontHashes = {
    "0_0" = {
      woff = "sha256-ngtOyqjbcuerf0CBhgkbYTGGr1xm45npnuQm3sqCG4E=";
      woff2 = "sha256-56M3/+SfYSJB6JcefsRnVArmuljc+WLkXXDI1jQdtrg=";
    };
    "1_0" = {
      woff = "sha256-WQjtCy6eN1dQ3QcbrFkQve5zg1Piy4X7hTO2XigBC34=";
      woff2 = "sha256-dU/V95wuyTHedJteazZcDtC03J3NoNgo0xvEL3KSGEM=";
    };
    "2_0" = {
      woff = "sha256-w8ycxUNHFTdxlzpOJqPfAScu9sGaR7kReDQtysYqZF8=";
      woff2 = "sha256-tTJTza0boVoKz903BlzObcwWIy3caMARxHAYhwNnQgM=";
    };
    "3_0" = {
      woff = "sha256-IW6ScsDMZpnkZ/zSvudwNqOM+N4XUzP92qWKbNw7Kmk=";
      woff2 = "sha256-THhUapgYVljXt4t2Ig780t40Z8giDmYV1YEvudXqI90=";
    };
    "4_0" = {
      woff = "sha256-hEhYnt3l12Y7YBXlxYPeluCvdUjRuZWjv2AJxwvO0I4=";
      woff2 = "sha256-QAuwq1qeVFkfMF3QKMU7QUF+e4vHIWrDRtwAknF5jZs=";
    };
    "5_0" = {
      inherit (fontHashes) woff woff2;
    };
  };
  websiteFont = variant: extension:
    fetchurl {
      url = "https://www.zotero.org/static/fonts/36AC02_${variant}.${extension}";
      hash = websiteFontHashes.${variant}.${extension};
    };

  styles = fetchurl {
    url = "https://www.zotero.org/styles-files/styles.json";
    hash = "sha256-ge+/0Khgn390e2XFMoav8Wz4NJKVcQ/Sxxkk9V1GB1c=";
  };
in
  buildNpmPackage {
    pname = "zotero-web-library";
    # Date of the pinned Web Library commit above.
    version = "1.8.1-unstable-2026-08-08";

    src = fetchFromGitHub {
      owner = "zotero";
      repo = "web-library";
      inherit rev;
      fetchSubmodules = true;
      hash = "sha256-DQq8X7ymMQfVWDhH6FJqgFXSICBiF9o1NIXEMaZZ3u8=";
    };

    npmDepsHash = "sha256-dLx1B7xSCDMy9rPQ6SbjejMx34DQzWYy/5edNdM15lI=";

    patches = [
      ./0001-default-missing-tag-colors-to-empty.patch
      ./0002-use-configured-api-for-item-templates.patch
      ./0003-add-read-only-mode.patch
      ./0004-use-configured-buy-storage-url.patch
      ./0005-remove-myfonts-tracking-import.patch
      ./0006-format-partial-write-errors.patch
    ];

    nativeBuildInputs = [unzip];

    preBuild = ''
      mkdir -p \
        data \
        src/static/fonts \
        src/static/js \
        src/static/note-editor \
        src/static/pdf-worker \
        src/static/reader \
        src/static/styles

      cp -r ${reader}/web/. src/static/reader/
      cp -r ${pdfWorker}/. src/static/pdf-worker/
      cp -r ${noteEditor}/web/. src/static/note-editor/

      cp ${font "eot"} src/static/fonts/36AC02_5_0.eot
      cp ${font "woff"} src/static/fonts/36AC02_5_0.woff
      cp ${font "woff2"} src/static/fonts/36AC02_5_0.woff2
      cp ${font "ttf"} src/static/fonts/36AC02_5_0.ttf
      cp ${font "svg"} src/static/fonts/36AC02_5_0.svg
      cp ${websiteFont "0_0" "woff"} src/static/fonts/36AC02_0_0.woff
      cp ${websiteFont "0_0" "woff2"} src/static/fonts/36AC02_0_0.woff2
      cp ${websiteFont "1_0" "woff"} src/static/fonts/36AC02_1_0.woff
      cp ${websiteFont "1_0" "woff2"} src/static/fonts/36AC02_1_0.woff2
      cp ${websiteFont "2_0" "woff"} src/static/fonts/36AC02_2_0.woff
      cp ${websiteFont "2_0" "woff2"} src/static/fonts/36AC02_2_0.woff2
      cp ${websiteFont "3_0" "woff"} src/static/fonts/36AC02_3_0.woff
      cp ${websiteFont "3_0" "woff2"} src/static/fonts/36AC02_3_0.woff2
      cp ${websiteFont "4_0" "woff"} src/static/fonts/36AC02_4_0.woff
      cp ${websiteFont "4_0" "woff2"} src/static/fonts/36AC02_4_0.woff2
      cp ${styles} data/styles.json
      cp ${styles} src/static/styles/styles.json
      unzip -p ${zotero}/lib/app/omni.ja chrome/content/zotero/xpcom/citeproc.js > citeproc.js
      unzip -j ${zotero}/lib/app/omni.ja 'styles/*.csl' -d src/static/styles
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

    meta = {
      description = "Zotero Web Library packaged for Zotero Local Cloud";
      homepage = "https://github.com/zotero/web-library";
      license = lib.licenses.agpl3Only;
    };
  }
