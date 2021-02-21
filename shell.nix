let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> {
    overlays = [ moz_overlay ];
  };
  ruststable = (nixpkgs.latest.rustChannels.stable.rust.override {
    extensions = [ "rust-src" ];}
  );
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "rust";
    buildInputs = [
      ruststable
      cargo-edit
      rust-analyzer
      pkg-config
      alsaLib
      xorg.libX11
      libudev
      steam-run
    ];
    LD_LIBRARY_PATH = with pkgs.xlibs; "${pkgs.mesa}/lib:${libX11}/lib:${libXcursor}/lib:${libXxf86vm}/lib:${libXi}/lib";
  }
