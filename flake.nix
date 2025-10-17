{
  description = "miro-pdf";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    nci.url = "github:yusdacra/nix-cargo-integration";
    home-manager.url = "github:nix-community/home-manager";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        inputs.nci.flakeModule
        inputs.home-manager.flakeModules.home-manager
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

      flake = {
        homeModules.default = {
          config,
          lib,
          pkgs,
          ...
        }: let
          inherit
            (lib)
            mkEnableOption
            mkOption
            mkIf
            types
            ;

          cfg = config.programs.miro-pdf;
        in {
          options.programs.miro-pdf = {
            enable = mkEnableOption "Enable miro-pdf";
            package = mkOption {
              description = "Package including miro-pdf binary (e.g. miro-pdf.packages.\${pkgs.system}.default)";
              type = types.package;
            };
            config = mkOption {
              description = "Config file text (uses assets/default.conf from the repo by default)";
              type = types.lines;
              default = builtins.readFile ./assets/default.conf;
            };
          };

          config = mkIf cfg.enable {
            home = {
              file.".config/miro-pdf/miro.conf".text = cfg.config;
              packages = [
                cfg.package
              ];
            };
          };
        };
      };
    };
}
