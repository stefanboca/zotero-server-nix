pkgs: let
  versitygw = pkgs.versitygw.overrideAttrs (oldAttrs: {
    patches = (oldAttrs.patches or []) ++ [./versitygw/0001-disable-multipart-pre-parser.patch];
  });
in {
  inherit versitygw;
  # keep-sorted start
  oidc-provider-mock = pkgs.callPackage ./oidc-provider-mock {};
  zotero = pkgs.callPackage ./zotero {inherit (pkgs) zotero;};
  zotero-attachment-proxy = pkgs.callPackage ./zotero-attachment-proxy {};
  zotero-citeproc-js-server = pkgs.callPackage ./zotero-citeproc-js-server {};
  zotero-dataserver = pkgs.callPackage ./zotero-dataserver {};
  zotero-portal = pkgs.callPackage ./zotero-portal {};
  zotero-stream-server = pkgs.callPackage ./zotero-stream-server {};
  zotero-tinymce-clean-server = pkgs.callPackage ./zotero-tinymce-clean-server {};
  zotero-translation-server = pkgs.callPackage ./zotero-translation-server {};
  zotero-web-library = pkgs.callPackage ./zotero-web-library {};
  # keep-sorted end
}
