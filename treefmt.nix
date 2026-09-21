{
  lib,
  pkgs,
  ...
}: let
  inherit (lib.meta) getExe;
in {
  projectRootFile = "flake.nix";
  settings.formatter = {
    topcoat = {
      command = getExe pkgs.topcoat-cli;
      options = ["fmt"];
      includes = ["*.rs"];
    };
    sqruff.excludes = ["pkgs/zotero-portal/migrations/*.sql"];
    ty = {
      command = getExe pkgs.ty;
      options = ["check" "--project" "tests/nixos"];
      includes = ["tests/nixos/*.py" "tests/nixos/**/*.pyi"];
    };
  };
  programs = {
    # keep-sorted start block=yes
    alejandra.enable = true;
    black.enable = true;
    deadnix.enable = true;
    keep-sorted.enable = true;
    rustfmt.enable = true;
    sqruff.enable = true;
    statix.enable = true;
    # keep-sorted end
  };
}
