{
  description = "Rust Development Overlay";

  inputs = {
    nixpkgs.url      = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nix-community/naersk";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, naersk, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        toolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
          extensions = [ "clippy" "rustfmt" "rust-src" "rust-analyzer-preview" ];
          targets = [ "x86_64-pc-windows-gnu"];
        });

        naersk-lib = naersk.lib."${system}".override {
          rustc = toolchain;
          cargo = toolchain;
        };

        _buildInputs = with pkgs; [
          openssl
          pkgconfig
          exa
          fd
          toolchain
          valgrind
          massif-visualizer

          # deps for eframe
          glib
          pango
          gdk-pixbuf
          atk
          gtk3
        ];

        _nativeBuildInputs = with pkgs; [ wrapGAppsHook ];

        proton_ovr = builtins.readFile ./proton-steam-comptime.txt;
      in
        rec {
          packages.kspacker = naersk-lib.buildPackage {
            pname = "kspacker";
            root = ./.;
            buildInputs = _buildInputs;
            nativeBuildInputs = _nativeBuildInputs;
            cargoBuildOptions = options: options ++ ["--features" "proton-steam-comptime"];
            PROTON_PATH_OVR = proton_ovr;
          };
          defaultPackage = packages.kspacker;

          apps.kspacker = flake-utils.lib.mkApp {
            drv = packages.kspacker;
          };
          defaultApp = apps.kspacker;

          devShell = pkgs.mkShell {
            buildInputs = _buildInputs;
            nativeBuildInputs = _nativeBuildInputs;

            PROTON_PATH_OVR = proton_ovr;

            shellHook = ''
            echo "Loaded devshell"
          '';
          };
        }
    );
}
