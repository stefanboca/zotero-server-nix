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
    inherit (lib.meta) getExe;

    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = genAttrs systems;
    nixpkgsFor = forAllSystems (system:
      import nixpkgs {
        inherit system;
        overlays = [self.overlays.default];
      });
    treefmtFor = forAllSystems (system: treefmt-nix.lib.evalModule nixpkgsFor.${system} ./treefmt.nix);
    devBackendFor = forAllSystems (system:
      lib.nixosSystem {
        inherit system;
        modules = [
          (import ./dev/vm.nix {
            module = self.nixosModules.default;
            overlay = self.overlays.default;
          })
        ];
      });
    developmentCaBundleFor = forAllSystems (system: let
      pkgs = nixpkgsFor.${system};
      certificates = import "${pkgs.path}/nixos/tests/common/acme/server/snakeoil-certs.nix";
    in
      pkgs.runCommand "zotero-development-ca-bundle" {} ''
        mkdir "$out"
        cat ${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt ${certificates.ca.cert} > "$out/ca-bundle.crt"
      '');
  in {
    packages = forAllSystems (system:
      import ./pkgs nixpkgsFor.${system}
      // {development-ca-bundle = developmentCaBundleFor.${system};});
    overlays.default = _: import ./pkgs;

    nixosModules = {
      default = import ./modules/nixos;
    };

    apps = forAllSystems (system: {
      default = {
        type = "app";
        program = "${devBackendFor.${system}.config.system.build.vm}/bin/run-nixos-vm";
        meta.description = "Run a development Zotero backend with Authelia in a VM";
      };
    });

    devShells = forAllSystems (system: let
      pkgs = nixpkgsFor.${system};
    in {
      default = pkgs.mkShellNoCC {
        packages = [pkgs.sqlx-cli pkgs.tailwindcss_4 self.packages.${system}.topcoat-cli];
        env = {
          TAILWIND_CLI = getExe pkgs.tailwindcss_4;
          ZOTERO_WEB_LIBRARY = self.packages.${system}.zotero-web-library;
        };
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
        pkgs = nixpkgsFor.${system};
        inherit
          (self.packages.${system})
          versitygw
          zotero-attachment-proxy
          zotero-portal
          zotero-citeproc-js-server
          zotero-dataserver
          zotero-stream-server
          zotero-tinymce-clean-server
          zotero-translation-server
          ;
        module = self.nixosModules.default;
      };
    in
      packages // configuredZotero // formatting // nixos);
  };
}
