{
  lib,
  craneLib,
  makeWrapper,
  wayland,
  libGL,
  xorg,
  libxkbcommon,
  fontconfig,
  pkg-config,
  clang,
  libclang,
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
in
  craneLib.buildPackage {
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
      clang
      libclang
      makeWrapper
    ];

    LIBCLANG_PATH = lib.makeLibraryPath [libclang.lib];

    buildInputs =
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
  }
