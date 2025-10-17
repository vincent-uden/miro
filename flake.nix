{
  description = "miro-pdf";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    nci.url = "github:yusdacra/nix-cargo-integration";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        inputs.nci.flakeModule
      ];
      systems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin"];
      perSystem = {
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }: let
        crateOutputs = config.nci.outputs.miro-pdf;
      in {
        nci = {
          projects.miro-pdf.path = ./.;
          crates.miro-pdf = let
            commonInputs = with pkgs; [
              fontconfig
            ];
            commonNativeBuildInputs =
              commonInputs
              ++ (with pkgs; [
                pkg-config
                libclang.lib
                clang
              ]);
          in {
            runtimeLibs =
              commonInputs
              ++ (with pkgs;
                with xorg; [
                  vulkan-loader
                  libGL

                  wayland
                  libX11
                  libxkbcommon
                ]);
            depsDrvConfig = {
              mkDerivation = {
                nativeBuildInputs =
                  commonNativeBuildInputs
                  ++ (with pkgs; [
                    unzip
                    python3
                    gperf
                  ]);
              };
              env = {
                LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
              };
            };
            drvConfig = {
              mkDerivation = {
                nativeBuildInputs = commonNativeBuildInputs;
              };
              env = {
                LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
              };
            };
          };
        };

        devShells.default = crateOutputs.devShell.overrideAttrs (old: {
          packages =
            (old.packages or [])
            ++ (with pkgs; [
              rust-analyzer
            ]);
        });
        packages.default = crateOutputs.packages.release;
      };
    };
}
