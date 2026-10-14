{
  lib,
  stdenv,
  craneLib,
  rustPlatform,
  wayland,
  libGL,
  libx11,
  libxkbcommon,
  fontconfig,
  pkg-config,
  vulkan-loader,
  glib,
  gtk3,
  pango,
  xdotool,
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
      libx11
    ];
  libsPath = lib.makeLibraryPath libs;

  src = lib.fileset.toSource {
    root = unfilteredRoot;

    fileset = lib.fileset.unions [
      # Default files from crane (Rust and cargo files)
      (craneLib.fileset.commonCargoSources unfilteredRoot)

      # Example of a folder for images, icons, etc
      (lib.fileset.maybeMissing ../assets)
    ];
  };

  cargoVendorDir = craneLib.vendorCargoDeps {
    inherit src;

    overrideVendorCargoPackage = package: drv:
      if package.name == "mupdf-sys" && package.version == "0.8.0"
      then
        drv.overrideAttrs (old: {
          patches = (old.patches or []) ++ [./patches/mupdf-sys-writable-copied-sources.patch];
        })
      else drv;
  };

  commonArgs = {
    inherit src cargoVendorDir;

    strictDeps = true;

    nativeBuildInputs = [
      pkg-config
      rustPlatform.bindgenHook
      pango
    ];

    buildInputs = [
      fontconfig
      vulkan-loader
      gtk3
      glib
      xdotool
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
        patchelf $out/bin/miro-pdf --add-rpath ${libsPath}
      '';

      passthru = {
        runtimeLibsPath = libsPath;
      };

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
