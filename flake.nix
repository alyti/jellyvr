{
  description = "Jellyfin proxy for VR Media Players";

  inputs = {
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    utils,
    fenix,
    ...
  }:
    utils.lib.eachDefaultSystem
    (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [fenix.overlays.default];
        };
        toolchain = pkgs.fenix.complete;
        rustPlatform = pkgs.makeRustPlatform {
          inherit (toolchain) cargo rustc;
        };
      in rec
      {
        # Executed by `nix build`
        packages.default =
          rustPlatform.buildRustPackage {
            pname = "jellyvr";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [
              pkg-config
              rustPlatform.bindgenHook
            ];
            buildInputs = with pkgs; [
              openssl
            ];

            PROTOC = "${pkgs.protobuf}/bin/protoc";
            PROTOC_INCLUDE = "${pkgs.protobuf}/include";

            ROCKSDB_INCLUDE_DIR = "${pkgs.rocksdb}/include";
            ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";


            # For other makeRustPlatform features see:
            # https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/rust.section.md#cargo-features-cargo-features
          };

        # Executed by `nix run`
        apps = {
          default = utils.lib.mkApp {drv = packages.default;};
          watch = let 
            script = pkgs.writeShellScriptBin "watch" ''
              ${pkgs.systemfd}/bin/systemfd --no-pid -s http::0.0.0.0:3000 -- ${pkgs.cargo-watch}/bin/cargo-watch -x run
            ''; in {
            type = "app";
            program = "${script}/bin/watch";
          };
        };

        # Used by `nix develop`
        devShells.default = pkgs.mkShell {

          nativeBuildInputs = with pkgs; [
            pkg-config
            rustPlatform.bindgenHook
          ];

          PROTOC = "${pkgs.protobuf}/bin/protoc";
          PROTOC_INCLUDE = "${pkgs.protobuf}/include";

          ROCKSDB_INCLUDE_DIR = "${pkgs.rocksdb}/include";
          ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";
          
          buildInputs = with pkgs; [
            (with toolchain; [
              cargo
              rustc
              rust-src
              clippy
              rustfmt
              rust-analyzer-nightly
            ])
            cargo-watch
            systemfd
            openssl # required by openssl-sys
            jq
          ];
          NIXPKGS_ALLOW_UNFREE=1;
          # Specify the rust-src path (many editors rely on this)
          RUST_SRC_PATH = "${toolchain.rust-src}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.libclang}/lib";
        };
      }
    );
}
