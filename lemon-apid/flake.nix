{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
  };

  outputs = {self, nixpkgs}:
    let systems = [ "x86_64-linux" "aarch64-linux" ];
        eachSupportedSystem = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
    in {
      packages = eachSupportedSystem (pkgs: {
        default = pkgs.callPackage (import ./package.nix) {};
      });

      nixosModules.default = import ./module.nix;
    };
}
