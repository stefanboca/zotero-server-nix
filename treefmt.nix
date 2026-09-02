{
  lib,
  pkgs,
  ...
}: let
  inherit (lib.meta) getExe;
in {
  projectRootFile = "flake.nix";
  settings.formatter.topcoat = {
    command = getExe pkgs.topcoat-cli;
    options = ["fmt"];
    includes = ["*.rs"];
  };
  settings.formatter.sqruff.excludes = ["pkgs/zotero-portal/migrations/*.sql"];
  programs = {
    # keep-sorted start block=yes
    alejandra.enable = true;
    deadnix.enable = true;
    keep-sorted.enable = true;
    rustfmt.enable = true;
    sqruff.enable = true;
    statix.enable = true;
    # keep-sorted end
  };
}
