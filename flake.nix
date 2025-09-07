{
  description = "cogex rust toolchain flake";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    naersk,
    fenix,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [fenix.overlays.default];
      };

      rustToolchain = fenix.packages.${system}.stable.withComponents [
        "rustc"
        "rust-src"
        "cargo"
        "clippy"
        "rustfmt"
      ];

      naersk-lib = pkgs.callPackage naersk {
        rustc = rustToolchain;
        cargo = rustToolchain;
      };
      dlopenLibraries = with pkgs; [
        libGL
        libxkbcommon
        vulkan-loader
        wayland
      ];
    in {
      packages.default = naersk-lib.buildPackage {
        src = ./.;
      };

      devShells.default = pkgs.mkShell {
        buildInputs = [rustToolchain pkgs.rust-analyzer-nightly pkgs.pkg-config pkgs.openssl];
        LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath dlopenLibraries;
      };
    });
}
