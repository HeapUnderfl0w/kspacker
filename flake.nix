{
  description = "Rust Development Overlay";

  inputs = {
    nixpkgs.url      = "github:nixos/nixpkgs/nixos-unstable";
    naersk = { url = "github:nix-community/naersk"; inputs.nixpkgs.follows = "nixpkgs"; };
    flake-utils.url  = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix, flake-utils, naersk, ... }:
    flake-utils.lib.eachSystem (flake-utils.lib.defaultSystems) (system:
      let
        overlays = [ fenix.overlay ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        toolchain = (pkgs.fenix.toolchainOf { channel = "nightly"; sha256 = "sha256-o0S6q8Wi8rrPQpm6nFvmlSkqCnRGi3YSLvrKUqTvKPM="; }).withComponents [
          "cargo" "rustc" "clippy" "rustfmt" "rust-src"
        ];

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
            buildInputs = _buildInputs ++ [ pkgs.rust-analyzer-nightly ];
            nativeBuildInputs = _nativeBuildInputs;

            PROTON_PATH_OVR = proton_ovr;

            shellHook = ''
            echo "Loaded devshell"
          '';
          };
        }
    );
}
