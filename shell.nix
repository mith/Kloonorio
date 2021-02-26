let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> {
    overlays = [ moz_overlay ];
  };
  ruststable = (nixpkgs.latest.rustChannels.stable.rust.override {
    extensions = [ "rust-src" ];
  });

  inherit (nixpkgs.lib) optionals;
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "kloonorio";
    buildInputs = [
      gdb
      ruststable
      cargo-edit
      rust-analyzer
      alsaLib
      pkgconfig
      python3
      vulkan-validation-layers
      vulkan-headers
      vulkan-loader
      xlibs.libX11
      xlibs.libXcursor
      xlibs.libXi
      xlibs.libXrandr
      udev
      renderdoc
    ];

    APPEND_LIBRARY_PATH = lib.makeLibraryPath [
      vulkan-loader
      vulkan-headers
      xlibs.libXcursor
      xlibs.libXi
      xlibs.libXrandr
    ];

    shellHook = ''
      export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$APPEND_LIBRARY_PATH"
      export RUSTFLAGS="-C target-cpu=native"
    '';
  }
