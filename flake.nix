{
  description = "miro-pdf";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default";
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};

        craneLib = crane.mkLib pkgs;

        miro-pdf = pkgs.callPackage ./nix/package.nix {inherit craneLib;};
      in {
        formatter = pkgs.alejandra;

        packages = {
          inherit miro-pdf;
          default = miro-pdf;
        };

        checks = {inherit miro-pdf;};

        devShells.default = craneLib.devShell {
          packages = with pkgs; [rust-analyzer];
        };
      }
    )
    // {
      homeModules.default = import ./nix/module.nix self.packages;
    };
}
