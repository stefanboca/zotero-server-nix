{
  imports = [
    ./services/versitygw.nix
    ./services/zotero-dataserver
  ];

  _class = "nixos";
  _file = ./default.nix;
}
