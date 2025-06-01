{
  description = "ytrss - Quickly get RSS feed URLs from YouTube channel urls";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    fenix,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      toolchain = fenix.packages.${system}.complete.toolchain;

      rustPlatform = pkgs.makeRustPlatform {
        cargo = toolchain;
        rustc = toolchain;
      };

      manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
    in {
      packages = {
        default = self.packages.${system}.ytrss;

        ytrss = rustPlatform.buildRustPackage {
          pname = manifest.name;
          version = manifest.version;

          src = pkgs.lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          meta = with pkgs.lib; {
            description = manifest.description;
            license = licenses.mit;
            mainProgram = "ytrss";
          };
        };
      };

      devShells.default = pkgs.mkShell {
        inputsFrom = [self.packages.${system}.ytrss];

        packages = with pkgs; [
          toolchain
          rust-analyzer
          cargo-watch
          cargo-edit
          cargo-outdated
          cargo-audit
          pkg-config
        ];

        env = {
          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };
      };

      apps = {
        default = self.apps.${system}.ytrss;
        ytrss = flake-utils.lib.mkApp {
          drv = self.packages.${system}.ytrss;
          name = "ytrss";
        };
      };

      formatter = pkgs.alejandra;
    });
}
