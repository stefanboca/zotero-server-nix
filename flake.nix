{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";

    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    self,
    nixpkgs,
    treefmt-nix,
    ...
  }: let
    inherit (nixpkgs) lib;
    inherit (lib.attrsets) genAttrs mapAttrs' nameValuePair;

    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = genAttrs systems;
    nixpkgsFor = forAllSystems (system: nixpkgs.legacyPackages.${system});
    treefmtFor = forAllSystems (system: treefmt-nix.lib.evalModule nixpkgsFor.${system} ./treefmt.nix);
  in {
    packages = forAllSystems (system: import ./pkgs nixpkgsFor.${system});
    overlays.default = _: import ./pkgs;

    nixosModules.default = import ./modules/nixos;

    nixosConfigurations.vm = lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        self.nixosModules.default
        ({modulesPath, ...}: {imports = [(modulesPath + "/virtualisation/qemu-vm.nix")];})
        ({pkgs, ...}: {
          virtualisation.qemu.options = ["-nographic" "-serial" "mon:stdio"];
          virtualisation.cores = 4;
          virtualisation.memorySize = 4 * 1024;
          virtualisation.forwardPorts = [
            {
              from = "host";
              host.port = 8080;
              guest.port = 80;
            }
          ];
          services.qemuGuest.enable = true;
          services.spice-vdagentd.enable = true;

          system.stateVersion = "26.05";
          nixpkgs.config.allowUnfree = true;
          nixpkgs.overlays = [self.overlays.default];
          nix = {
            settings.trusted-users = ["root" "nixos"];
            extraOptions = ''
              experimental-features = nix-command flakes
            '';
          };

          programs.fish.enable = true;
          security.sudo.wheelNeedsPassword = false;
          users.users.nixos = {
            isNormalUser = true;
            hashedPassword = "";
            extraGroups = ["wheel"];
            shell = pkgs.fish;
          };

          services.zotero-dataserver.enable = true;
        })
      ];
    };

    formatter = forAllSystems (system: treefmtFor.${system}.config.build.wrapper);
    checks = forAllSystems (system: let
      packages = mapAttrs' (n: nameValuePair "package-${n}") self.packages.${system};
      formatting = {formatting = treefmtFor.${system}.config.build.check self;};
    in
      packages // formatting);
  };
}
