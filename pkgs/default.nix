pkgs: {
  # keep-sorted start
  zotero-attachment-proxy = pkgs.callPackage ./zotero-attachment-proxy {};
  zotero-citeproc-js-server = pkgs.callPackage ./zotero-citeproc-js-server {};
  zotero-dataserver = pkgs.callPackage ./zotero-dataserver {};
  zotero-stream-server = pkgs.callPackage ./zotero-stream-server {};
  zotero-tinymce-clean-server = pkgs.callPackage ./zotero-tinymce-clean-server {};
  zotero-translation-server = pkgs.callPackage ./zotero-translation-server {};
  # keep-sorted end
}
