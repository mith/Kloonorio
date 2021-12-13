{
  description = "kloonorio";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nmattia/naersk";
  };

  outputs = { self, nixpkgs, utils, rust-overlay, naersk, ... }:
    utils.lib.eachSystem [ "x86_64-linux" ] (system:
      let
        rust = pkgs.rust-bin.stable.latest.default.override {
                extensions = [ "rust-src" ];
                targets = [ "wasm32-unknown-unknown" ];
              };
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ 
            rust-overlay.overlay
            (self: super: {
              rustc = rust;
              cargo = rust;
            })
          ];
        };
        naersk-lib = naersk.lib."${system}".override {
          cargo = rust;
          rustc = rust;
        };
      in
      rec {
        packages.kloonorio = naersk-lib.buildPackage {
          pname = "kloonorio";
          root = ./.;
          nativeBuildInputs = with pkgs; [
            pkg-config
            alsa-lib
            libudev
            xorg.libX11
            xlibs.libX11
            xlibs.libXcursor
            xlibs.libXi
            xlibs.libXrandr
            python3
          ];
        };
        defaultPackage = packages.kloonorio;

        apps.kloonorio = utils.lib.mkApp {
          drv = packages.kloonorio;
        };
        defaultApp = apps.kloonorio;

        devShell = pkgs.mkShell {
          inputsFrom = [ packages.kloonorio ];
          RUST_SRC_PATH="${pkgs.rust-bin.stable.latest.rust-src}/lib/rustlib/src/rust/library/";
          buildInputs = with pkgs; [ 
            rust-bin.stable.latest.default
            vulkan-loader 
            lldb
          ];
        };
      });
}
