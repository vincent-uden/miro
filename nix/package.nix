{
  lib,
  stdenv,
  craneLib,
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

  libs =
    [
      libGL
      libxkbcommon
    ]
    ++ lib.optionals stdenv.hostPlatform.isLinux [
      wayland
      xorg.libX11
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
      rustPlatform.bindgenHook
    ];

    buildInputs = [
      fontconfig
      vulkan-loader
    ];

    # prevent bindgen from rebuilding unnecessarily
    # see https://crane.dev/faq/rebuilds-bindgen.html
    NIX_OUTPATH_USED_AS_RANDOM_SEED = "_miro-pdf_";
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
  craneLib.buildPackage (commonArgs
    // {
      inherit cargoArtifacts;

      postFixup = ''
        patchelf $out/bin/miro-pdf --add-rpath ${lib.makeLibraryPath libs}
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
