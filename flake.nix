{
  description = "kloonorio";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-local.url = "github:mith/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  };

  outputs = inputs @ {
    self,
    nixpkgs,
    flake-utils,
    fenix,
    crane,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages."${system}";
        toolchain = fenix.packages.${system}.stable;
        crane-lib = crane.lib."${system}";
        kloonorio-src = builtins.path {
          path = ./.;
          name = "kloonorio-src";
          filter = path: type:
            nixpkgs.lib.all
            (n: builtins.baseNameOf path != n)
            [
              "web"
              "assets"
              "flake.nix"
              "flake.lock"
              "README.md"
              ".envrc"
              ".direnv"
              ".gitignore"
            ];
        };
        buildInputs = with pkgs; [
          libxkbcommon
          alsa-lib
          udev
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
          libxkbcommon
          python3
          vulkan-loader
          wayland
        ];
        nativeBuildInputs = with pkgs; [
          mold
          clang
          pkg-config
        ];
      in {
        packages.kloonorio-bin = crane-lib.buildPackage {
          name = "kloonorio-bin";
          src = kloonorio-src;
          inherit buildInputs;
          inherit nativeBuildInputs;
        };
        packages.kloonorio = pkgs.stdenv.mkDerivation {
          name = "kloonorio";
          src = ./assets;
          phases = ["unpackPhase" "installPhase"];
          installPhase = ''
            mkdir -p $out
            cp ${self.packages.${system}.kloonorio-bin}/bin/kloonorio $out/kloonorio
            cp -r $src $out/assets
          '';
        };

        packages.kloonorio-wasm = let
          target = "wasm32-unknown-unknown";
          toolchain = with fenix.packages.${system};
            combine [
              stable.rustc
              stable.cargo
              targets.${target}.stable.rust-std
            ];
          craneWasm = (crane.mkLib pkgs).overrideToolchain toolchain;
        in
          craneWasm.buildPackage {
            src = kloonorio-src;
            CARGO_BUILD_TARGET = target;
            CARGO_PROFILE = "release";
            inherit nativeBuildInputs;
            doCheck = false;
          };

        packages.kloonorio-web = let
          local = import inputs.nixpkgs-local {system = "${system}";};
        in
          pkgs.stdenv.mkDerivation {
            name = "kloonorio-web";
            src = ./.;
            nativeBuildInputs = [
              local.wasm-bindgen-cli
              pkgs.binaryen
            ];
            phases = ["unpackPhase" "installPhase"];
            installPhase = ''
              mkdir -p $out
              wasm-bindgen --out-dir $out --out-name kloonorio --target web ${self.packages.${system}.kloonorio-wasm}/bin/kloonorio.wasm
              mv $out/kloonorio_bg.wasm .
              wasm-opt -Oz -o $out/kloonorio_bg.wasm kloonorio_bg.wasm
              cp web/* $out/
              cp -r assets $out/assets
            '';
          };

        packages.kloonorio-server = pkgs.writeShellScriptBin "run-kloonorio-server" ''
          ${pkgs.simple-http-server}/bin/simple-http-server -i -c=html,wasm,ttf,js -- ${self.packages.${system}.kloonorio-web}/
        '';

        defaultPackage = self.packages.${system}.kloonorio;

        apps.kloonorio = flake-utils.lib.mkApp {
          drv = self.packages.${system}.kloonorio;
          exePath = "/kloonorio";
        };
        defaultApp = self.apps.${system}.kloonorio;

        checks = {
          pre-commit-check = inputs.pre-commit-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              alejandra.enable = true;
              statix.enable = true;
              rustfmt.enable = true;
              clippy = {
                enable = false;
                entry = let
                  rust = toolchain.withComponents ["clippy"];
                in
                  pkgs.lib.mkForce "${rust}/bin/cargo-clippy clippy";
              };
            };
          };
        };

        devShell = pkgs.mkShell {
          shellHook = ''
            export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.lib.makeLibraryPath buildInputs}"
            ${self.checks.${system}.pre-commit-check.shellHook}
          '';
          inputsFrom = [self.packages.${system}.kloonorio-bin];
          RUST_LOG = "error,kloonorio=info";
          nativeBuildInputs = with pkgs;
            [
              (toolchain.withComponents ["cargo" "rustc" "rust-src" "rustfmt" "clippy"])
              rust-analyzer
              lldb
              nil
              cargo-nextest
            ]
            ++ nativeBuildInputs;
        };
      }
    );
}
