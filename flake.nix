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

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      crane,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) miro;
      in
      {
        formatter = pkgs.alejandra;
        checks = { inherit miro; };
        packages.default = miro;
        devShells.default = (crane.mkLib pkgs).devShell {
          inputsFrom = [ miro ];
          packages = with pkgs; [ rust-analyzer ];
        };
      }
    )
    // {
      homeModules.default = import ./nix/module.nix self.packages;
    };
}
