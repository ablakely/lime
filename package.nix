{ lib, rustPlatform }:

let fs = lib.fileset;
    src = fs.toSource {
      root = ./.;
      fileset = fs.gitTracked ./.;
    };
in

rustPlatform.buildRustPackage {
  pname = "lemon-website";
  version = "1.0.0";
  inherit src;
  cargoLock.lockFile = ./Cargo.lock;
}
