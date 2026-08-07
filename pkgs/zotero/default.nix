{
  lib,
  lndir,
  runCommandLocal,
  unzip,
  zip,
  zotero,
}: let
  inherit (builtins) elemAt match toJSON;
  inherit (lib) escapeShellArg removeSuffix replaceStrings;

  addTrailingSlash = url: "${removeSuffix "/" url}/";

  hostFromUrl = url: let
    parts = match "https?://([^/:]+)(:[0-9]+)?(/.*)?" url;
  in
    if parts == null
    then throw "zotero.withConfig: expected an HTTP(S) URL, got ${toJSON url}"
    else elemAt parts 0;

  withConfig = {
    apiBaseUri,
    baseUri ? apiBaseUri,
    wwwBaseUrl ? apiBaseUri,
    domainName ? hostFromUrl wwwBaseUrl,
    streamingUrl ? "${
      replaceStrings ["https://" "http://"] ["wss://" "ws://"] (removeSuffix "/" apiBaseUri)
    }/stream/",
  }: let
    config = {
      inherit domainName;
      apiBaseUri = addTrailingSlash apiBaseUri;
      baseUri = addTrailingSlash baseUri;
      streamingUrl = addTrailingSlash streamingUrl;
      wwwBaseUrl = addTrailingSlash wwwBaseUrl;
    };
  in
    runCommandLocal "${zotero.pname}-configured-${zotero.version}"
    {
      nativeBuildInputs = [
        lndir
        unzip
        zip
      ];

      inherit (zotero) meta;
      passthru =
        (zotero.passthru or {})
        // {
          inherit config withConfig;
          unconfigured = zotero;
        };
    }
    ''
      mkdir -p "$out"
      lndir -silent ${zotero} "$out"

      rm "$out/lib/app/omni.ja" "$out/lib/zotero"
      cp ${zotero}/lib/app/omni.ja "$out/lib/app/omni.ja"
      cp ${zotero}/lib/zotero "$out/lib/zotero"

      rm "$out/bin/zotero" "$out/bin/.zotero-wrapped"
      cp ${zotero}/bin/zotero "$out/bin/zotero"
      ln -s ../lib/zotero "$out/bin/.zotero-wrapped"

      # Keep this patch tied to Zotero's stable resource/config.mjs file.
      # --replace-fail makes an upstream rename or format change visible.
      mkdir config-patch
      cd config-patch
      unzip -q "$out/lib/app/omni.ja" resource/config.mjs
      substituteInPlace resource/config.mjs \
        --replace-fail ${escapeShellArg "DOMAIN_NAME: 'zotero.org',"} ${escapeShellArg "DOMAIN_NAME: ${toJSON config.domainName},"} \
        --replace-fail ${escapeShellArg "BASE_URI: 'http://zotero.org/',"} ${escapeShellArg "BASE_URI: ${toJSON config.baseUri},"} \
        --replace-fail ${escapeShellArg "WWW_BASE_URL: 'https://www.zotero.org/',"} ${escapeShellArg "WWW_BASE_URL: ${toJSON config.wwwBaseUrl},"} \
        --replace-fail ${escapeShellArg "API_URL: 'https://api.zotero.org/',"} ${escapeShellArg "API_URL: ${toJSON config.apiBaseUri},"} \
        --replace-fail ${escapeShellArg "STREAMING_URL: 'wss://stream.zotero.org/',"} ${escapeShellArg "STREAMING_URL: ${toJSON config.streamingUrl},"}

      chmod u+w "$out/lib/app" "$out/lib/app/omni.ja"
      zip -q -u "$out/lib/app/omni.ja" resource/config.mjs

      # wrapGApps embeds the unconfigured output in this outer launcher.
      substituteInPlace "$out/bin/zotero" \
        --replace-fail ${escapeShellArg (toString zotero)} "$out"
    '';
in
  zotero.overrideAttrs (old: {
    passthru = (old.passthru or {}) // {inherit withConfig;};
  })
