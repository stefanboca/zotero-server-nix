{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.zst";

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
    inherit (lib.strings) concatMapStringsSep escapeShellArg escapeShellArgs;
    inherit (lib.meta) getExe;

    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = genAttrs systems;
    nixpkgsFor = forAllSystems (system: nixpkgs.legacyPackages.${system});
    treefmtFor = forAllSystems (system: treefmt-nix.lib.evalModule nixpkgsFor.${system} ./treefmt.nix);
  in {
    packages = forAllSystems (system: import ./pkgs nixpkgsFor.${system});
    overlays.default = _: import ./pkgs;

    nixosModules.default = import ./modules/nixos;

    apps = forAllSystems (system: let
      pkgs = nixpkgsFor.${system};
      packages = self.packages.${system};
      sqlxPrepare = import ./dev/sqlx-prepare.nix {inherit lib pkgs;};
      update = pkgs.writeShellApplication {
        name = "update-zotero-server-packages";
        text = concatMapStringsSep "\n" (name: ''UPDATE_NIX_ATTR_PATH=${escapeShellArg name} ${escapeShellArgs packages.${name}.passthru.updateScript}'') [
          "zotero-attachment-proxy"
          "zotero-citeproc-js-server"
          "zotero-dataserver"
          "zotero-stream-server"
          "zotero-tinymce-clean-server"
          "zotero-translation-server"
          "zotero-web-library"
        ];
      };
      vm = lib.nixosSystem {
        inherit system;
        modules = [(import ./dev/vm.nix self)];
      };
    in {
      default = {
        type = "app";
        program = getExe vm.config.system.build.vm;
        meta = {
          passthru = {inherit (vm.config.system.build) vm;};
          description = "Run a development Zotero backend with a mock OIDC provider in a VM";
        };
      };
      sqlx-prepare = {
        type = "app";
        program = getExe sqlxPrepare;
        meta.description = "Prepare SQLx metadata with a temporary MariaDB database";
      };
      update = {
        type = "app";
        program = getExe update;
        meta.description = "Update all upstream Zotero server packages";
      };
    });

    devShells = forAllSystems (system: let
      pkgs = nixpkgsFor.${system};
    in {
      default = pkgs.mkShellNoCC {
        packages = with pkgs; [
          # sqlx-cli
          sqruff
          tailwindcss_4
          topcoat-cli
        ];
        env.TAILWIND_CLI = getExe pkgs.tailwindcss_4;
      };
    });

    formatter = forAllSystems (system: treefmtFor.${system}.config.build.wrapper);
    checks = forAllSystems (system: let
      packages = mapAttrs' (n: nameValuePair "package-${n}") self.packages.${system};
      configuredZotero = {
        package-zotero-configured = self.packages.${system}.zotero.withConfig {
          baseUri = "https://zotero.example.com/";
        };
      };
      formatting = {formatting = treefmtFor.${system}.config.build.check self;};
      nixos = import ./tests/nixos {
        inherit self;
        pkgs = nixpkgsFor.${system};
      };
    in
      packages // configuredZotero // formatting // nixos);
  };
}
