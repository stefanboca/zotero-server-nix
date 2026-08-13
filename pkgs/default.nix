pkgs: let
  topcoat-cli = pkgs.callPackage ./topcoat-cli {};
  zotero-web-library = pkgs.callPackage ./zotero-web-library {};
in {
  # keep-sorted start
  inherit topcoat-cli zotero-web-library;
  versitygw = pkgs.callPackage ./versitygw {};
  zotero = pkgs.callPackage ./zotero {inherit (pkgs) zotero;};
  zotero-attachment-proxy = pkgs.callPackage ./zotero-attachment-proxy {};
  zotero-citeproc-js-server = pkgs.callPackage ./zotero-citeproc-js-server {};
  zotero-dataserver = pkgs.callPackage ./zotero-dataserver {};
  zotero-portal = pkgs.callPackage ./zotero-portal {inherit topcoat-cli zotero-web-library;};
  zotero-stream-server = pkgs.callPackage ./zotero-stream-server {};
  zotero-tinymce-clean-server = pkgs.callPackage ./zotero-tinymce-clean-server {};
  zotero-translation-server = pkgs.callPackage ./zotero-translation-server {};
  # keep-sorted end
}
