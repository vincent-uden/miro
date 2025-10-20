packages: {
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
      default = packages.${pkgs.system}.default;
    };

    config = lib.mkOption {
      description = "Config file text (uses assets/default.conf from the repo by default)";
      type = lib.types.lines;
      default = builtins.readFile ../assets/default.conf;
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [cfg.package];

    xdg.configFile."miro-pdf/miro.conf".text = cfg.config;
  };
}
