{
  description = "Crossport development environment and package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        toolchain = pkgs.rust-bin.stable."1.98.1".minimal.override {
          extensions = [ "rustfmt" "clippy" ];
        };
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };
        crossport = rustPlatform.buildRustPackage {
          pname = "crossport";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.makeWrapper ];
          nativeCheckInputs = [ pkgs.lsof ];
          doCheck = true;
          buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
          # Unix scanning requires lsof even outside a development shell.
          postInstall = ''
            wrapProgram $out/bin/crossport --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.lsof ]}
          '';
          meta = {
            description = "Inspect and manage local listening ports";
            license = pkgs.lib.licenses.mit;
            mainProgram = "crossport";
          };
        };
      in {
        packages.default = crossport;
        apps.default = {
          type = "app";
          program = "${crossport}/bin/crossport";
        };
        checks.default = crossport;
        devShells.default = pkgs.mkShell {
          packages = [ toolchain pkgs.lsof ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
          shellHook = ''
            echo "Crossport: cargo run -- ui | cargo test | nix build"
          '';
        };
      });
}
