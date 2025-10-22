{
  lib,
  craneLib,
  makeWrapper,
  rustPlatform,
  wayland,
  libGL,
  xorg,
  libxkbcommon,
  fontconfig,
  pkg-config,
  vulkan-loader,
  ...
}: let
  unfilteredRoot = ../.;

  libs = [
    wayland
    libGL
    xorg.libX11
    libxkbcommon
  ];

  commonArgs = {
    src = lib.fileset.toSource {
      root = unfilteredRoot;

      fileset = lib.fileset.unions [
        # Default files from crane (Rust and cargo files)
        (craneLib.fileset.commonCargoSources unfilteredRoot)

        # Example of a folder for images, icons, etc
        (lib.fileset.maybeMissing ../assets)
      ];
    };

    strictDeps = true;

    nativeBuildInputs = [
      pkg-config
      makeWrapper
      rustPlatform.bindgenHook
    ];

    buildInputs =
      [
        fontconfig
        vulkan-loader
      ]
      ++ libs;

    # prevent bindgen from rebuilding unnecessarily
    # see https://crane.dev/faq/rebuilds-bindgen.html
    NIX_OUTPATH_USED_AS_RANDOM_SEED = "_miro-pdf_";
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
  craneLib.buildPackage (commonArgs
    // {
      inherit cargoArtifacts;

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
    })
