{
  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
          config.allowUnfree = true;
        };

        # Helper to collect -I flags from a list of dev packages
        includeFlags = devPkgs: builtins.concatMap
          (p: [ "-I${p}/include" ])
          devPkgs;
      in {
        devShells.default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              pkg-config          # <-- must be here for setup hook to wire PKG_CONFIG_PATH
#               rust-bin.stable.latest.default
              (rust-bin.nightly.latest.default.override {
                extensions = [ "rust-src" "rust-analyzer" "miri" ];
              })
              rust-analyzer
              cargo-watch
              rustfmt
              clippy
              libclang
              gdb
            ];

            buildInputs = with pkgs; [

            ];

            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            BINDGEN_EXTRA_CLANG_ARGS = [
            ];


            LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath [

            ];
        };
      }
    );
}
