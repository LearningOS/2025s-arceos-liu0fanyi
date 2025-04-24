# in flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      # 用于消除在nix flake metadata里看到的重复依赖
      inputs = { flake-utils.follows = "flake-utils"; };
    };
    # cargo build rust for nix derivation
    crane = { url = "github:ipetkov/crane"; };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    # 虽然是eachDefaultSystem，但正常nix develop实际上只针对默认环境
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile
          ./rust-toolchain.toml;
        # this is how we can tell crane to use our toolchain
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        # cf. https://crane.dev/API.html#libcleancargosource
        src = craneLib.cleanCargoSource ./.;
        # 这里是编译过程中的相关依赖
        # 比如编译过后不需要rust工具链
        nativeBuildInputs = with pkgs; [ rustToolchain pkg-config ];
        # 这里是运行的时候仍然需要依赖的部分
        # 比如程序运行需要sqlite的动态链接库存在(sqlite3.so啥的)
        buildInputs = with pkgs; [ openssl sqlite ];
        # because we'll use it for both `cargoArtifacts` and `bin`
        commonArgs = { inherit src buildInputs nativeBuildInputs; };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        # remember, `set1 // set2` does a shallow merge:
        bin = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
      in with pkgs; {
        packages = {
          # that way we can build `bin` specifically,
          # but it's also the default.
          # inherit bin dockerImage;
          inherit bin;
          default = bin;
        };
        devShells.default = mkShell {
          # inputsForm = [ bin ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [
            # docker
            dive

            # rust
            rustToolchain

            # 方便输出asm
            cargo-show-asm

            # rcore
            cargo-binutils
            qemu
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
        };
      });
}
