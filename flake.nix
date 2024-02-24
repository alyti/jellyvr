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
    self,
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
        cargoToml = with builtins; let toml = readFile ./Cargo.toml; in fromTOML toml;

        buildMetadata = with pkgs.lib.strings;
          let
            lastModifiedDate = self.lastModifiedDate or self.lastModified or "";
            date = builtins.substring 0 8 lastModifiedDate;
            shortRev = self.shortRev or "dirty";
            hasDateRev = lastModifiedDate != "" && shortRev != "";
            dot = optionalString hasDateRev ".";
          in "${date}${dot}${shortRev}";

        version =  with pkgs.lib.strings;
          let
            hasBuildMetadata = buildMetadata != "";
            plus = optionalString hasBuildMetadata "+";
          in "${cargoToml.package.version}${plus}${buildMetadata}";
      in rec
      {
        # Executed by `nix build`
        packages = let 
          bin = rustPlatform.buildRustPackage {
            inherit version;

            pname = "jellyvr";
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

          dockerImage = pkgs.dockerTools.buildLayeredImage {
            name = "jellyvr";
            tag = "v${builtins.replaceStrings [ "+" ] [ "-" ] version}";
            created = "now";
            config = {
              Entrypoint  = "${bin}/bin/jellyvr";
              Env = [ "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ];
              WorkingDir = "/data";
              Volumes = {
                "/data" = {};
              };
            };
          };
        in {
          inherit bin dockerImage;
          default = bin;
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
            dive
          ];
          NIXPKGS_ALLOW_UNFREE=1;
          # Specify the rust-src path (many editors rely on this)
          RUST_SRC_PATH = "${toolchain.rust-src}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.libclang}/lib";
        };
      }
    );
}
