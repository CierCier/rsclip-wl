{
  description = "rsclip, a Wayland clipboard manager with a GTK4 UI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    crane = {
      url = "github:ipetkov/crane";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    let
      inherit (nixpkgs) lib;

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem = system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "clippy"
              "rust-src"
              "rustfmt"
            ];
          };

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
          project = builtins.fromTOML (builtins.readFile ./crates/rsclip-ui/Cargo.toml);

          cargoFiles = craneLib.fileset.commonCargoSources ./.;
          cargoSrc = lib.fileset.toSource {
            root = ./.;
            fileset = cargoFiles;
          };
          buildSrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              cargoFiles
              ./crates/rsclip-ui/resources
            ];
          };
          packageSrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              cargoFiles
              ./crates/rsclip-ui/resources
              ./packaging
              ./config.example.toml
              ./LICENSE
              ./README.md
            ];
          };

          buildInputs = with pkgs; [
            gtk4
            gtk4-layer-shell
          ];
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          runtimeInputs = with pkgs; [
            bash
            tesseract
            wl-clipboard
            wtype
          ];

          commonArgs = {
            inherit buildInputs nativeBuildInputs;
            pname = "rsclip";
            version = project.package.version;
            strictDeps = true;
            cargoExtraArgs = "--locked --workspace";
            CARGO_PROFILE = "release";
          };

          cargoArtifacts = craneLib.buildDepsOnly (
            commonArgs
            // {
              pname = "rsclip-deps";
              src = cargoSrc;
            }
          );

          rsclip = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              pname = "rsclip";
              version = project.package.version;
              src = packageSrc;

              cargoBuildExtraArgs = "--bins";
              doCheck = false;
              nativeBuildInputs = nativeBuildInputs ++ [ pkgs.makeWrapper ];

              installPhaseCommand = ''
                install -Dm755 target/release/rsclip "$out/bin/rsclip"
                install -Dm755 target/release/rsclipd "$out/bin/rsclipd"
                install -Dm644 packaging/desktop/rsclip.desktop \
                  "$out/share/applications/rsclip.desktop"
                install -Dm644 packaging/systemd/rsclipd.service \
                  "$out/lib/systemd/user/rsclipd.service"
                substituteInPlace "$out/lib/systemd/user/rsclipd.service" \
                  --replace-fail /usr/bin/rsclipd "$out/bin/rsclipd"
                install -Dm644 config.example.toml \
                  "$out/share/doc/rsclip/config.example.toml"
                install -Dm644 README.md "$out/share/doc/rsclip/README.md"
                install -Dm644 LICENSE "$out/share/licenses/rsclip/LICENSE"
              '';

              postInstall = ''
                runtimePath=${lib.makeBinPath runtimeInputs}
                wrapProgram "$out/bin/rsclip" --prefix PATH : "$runtimePath"
                wrapProgram "$out/bin/rsclipd" --prefix PATH : "$runtimePath"
              '';

              meta = {
                description = "Wayland clipboard manager with a GTK4 UI and background daemon";
                homepage = "https://github.com/CierCier/rsclip-wl";
                license = lib.licenses.mit;
                mainProgram = "rsclip";
                platforms = lib.platforms.linux;
              };
            }
          );

          checks = {
            inherit rsclip;

            fmt = craneLib.cargoFmt {
              inherit (commonArgs) pname version;
              src = cargoSrc;
            };

            clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                src = buildSrc;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );

            tests = craneLib.cargoTest (
              commonArgs
              // {
                inherit cargoArtifacts;
                src = buildSrc;
                cargoTestExtraArgs = "--all-targets";
              }
            );
          };
        in
        {
          packages = {
            default = rsclip;
            inherit rsclip;
          };

          inherit checks;

          apps = {
            default = {
              type = "app";
              program = "${rsclip}/bin/rsclip";
              meta.description = "Open the rsclip clipboard history UI";
            };

            rsclipd = {
              type = "app";
              program = "${rsclip}/bin/rsclipd";
              meta.description = "Run the rsclip clipboard daemon";
            };
          };

          devShells.default = pkgs.mkShell {
            inherit buildInputs nativeBuildInputs;
            packages = [
              rustToolchain
              pkgs.alejandra
              pkgs.cargo-nextest
              pkgs.rust-analyzer
            ]
            ++ runtimeInputs;

            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          };

          formatter = pkgs.alejandra;
        };

      allSystems = lib.genAttrs systems perSystem;
    in
    {
      packages = lib.mapAttrs (_system: outputs: outputs.packages) allSystems;
      checks = lib.mapAttrs (_system: outputs: outputs.checks) allSystems;
      apps = lib.mapAttrs (_system: outputs: outputs.apps) allSystems;
      devShells = lib.mapAttrs (_system: outputs: outputs.devShells) allSystems;
      formatter = lib.mapAttrs (_system: outputs: outputs.formatter) allSystems;

      overlays.default = final: _prev: {
        rsclip = self.packages.${final.stdenv.hostPlatform.system}.default;
      };

      nixosModules.default =
        { config, lib, pkgs, ... }:
        let
          cfg = config.programs.rsclip;
        in
        {
          options.programs.rsclip = {
            enable = lib.mkEnableOption "rsclip, the Wayland clipboard manager";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.default;
              description = "The rsclip package to install and run.";
            };
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];

            systemd.user.services.rsclipd = {
              description = "rsclip clipboard daemon";
              after = [ "graphical-session.target" ];
              wantedBy = [ "graphical-session.target" ];
              partOf = [ "graphical-session.target" ];
              unitConfig = {
                ConditionEnvironment = "WAYLAND_DISPLAY";
              };
              serviceConfig = {
                Type = "simple";
                ExecStart = "${cfg.package}/bin/rsclipd watch";
                Restart = "on-failure";
                RestartSec = 2;
              };
            };
          };
        };
    };
}
