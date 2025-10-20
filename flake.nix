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
        inherit (nixpkgs) lib;

        craneLib = crane.mkLib pkgs;
      in {
        formatter = pkgs.alejandra;

        packages.default = let
          unfilteredRoot = ./.;

          libs = with pkgs; [
            wayland
            libGL
            xorg.libX11
            libxkbcommon
          ];
        in
          craneLib.buildPackage {
            src = lib.fileset.toSource {
              root = unfilteredRoot;

              fileset = lib.fileset.unions [
                # Default files from crane (Rust and cargo files)
                (craneLib.fileset.commonCargoSources unfilteredRoot)

                # Example of a folder for images, icons, etc
                (lib.fileset.maybeMissing ./assets)
              ];
            };

            strictDeps = true;

            nativeBuildInputs = with pkgs; [
              fontconfig
              pkg-config
              clang
              libclang
              unzip
              gperf
              makeWrapper
            ];

            LIBCLANG_PATH = lib.makeLibraryPath [pkgs.libclang.lib];

            buildInputs = with pkgs;
              [
                fontconfig
                vulkan-loader
              ]
              ++ libs;

            postInstall = ''
              wrapProgram "$out/bin/miro-pdf" \
              --set LD_LIBRARY_PATH "${lib.makeLibraryPath libs}"
            '';

            meta = {
              description = "A native pdf viewer for Windows and Linux (Wayland/X11) with configurable keybindings";
              homepage = "https://github.com/vincent-uden/miro";
              license = lib.licenses.agpl3Only;
              maintainers = with lib.maintainers; [
                tukanoidd
                Vortriz
              ];
              mainProgram = "miro-pdf";
            };
          };

        devShell = craneLib.devShell {
          packages = with pkgs; [rust-analyzer];
        };
      }
    )
    // {
      homeModules.default = {
        config,
        lib,
        pkgs,
        ...
      }: let
        cfg = config.programs.miro-pdf;
      in {
        options.programs.miro-pdf = {
          enable = lib.mkEnableOption "Enable miro-pdf";

          package = lib.mkOption {
            description = "Package including miro-pdf binary (e.g. miro-pdf.packages.\${pkgs.system}.default)";
            type = lib.types.package;
            default = self.packages.${pkgs.system}.default;
          };

          config = lib.mkOption {
            description = "Config file text (uses assets/default.conf from the repo by default)";
            type = lib.types.lines;
            default = builtins.readFile ./assets/default.conf;
          };
        };

        config = lib.mkIf cfg.enable {
          home.packages = [cfg.package];

          xdg.configFile."miro-pdf/miro.conf".text = cfg.config;
        };
      };
    };
}
