{
  description = "crossport dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";

    # 🔥 Rust overlay を追加：これが rust-bin の正体
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # overlay を適用した nixpkgs を作る
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            (import rust-overlay) # ←これで rust-bin が生える
          ];
        };

        lib = pkgs.lib;
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            # Rust 1.91.0 をここで使えるようになる
            pkgs.rust-bin.stable."1.91.0".default

            pkgs.pkg-config
            pkgs.openssl
            pkgs.libiconv
            pkgs.nixfmt-classic
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.IOKit
            pkgs.darwin.apple_sdk.frameworks.CoreFoundation
            pkgs.darwin.apple_sdk.frameworks.Security
          ];

          LIBRARY_PATH = lib.makeLibraryPath [ pkgs.libiconv ]
            + (if builtins.getEnv "LIBRARY_PATH" == "" then
              ""
            else
              ":" + builtins.getEnv "LIBRARY_PATH");

          shellHook = ''
            echo "🚀 crossport dev shell (Rust 1.91.0)"
            echo "💡 Tips: "
            echo "   - 'cx <args>': alias for 'cargo run -- <args>'"
            echo "   - 'build': alias for 'cargo build --release'"
            echo "   - 'test': alias for 'cargo test'"

            alias cx="cargo run --"
            alias build="cargo build --release"
            alias test="cargo test"
          '';
        };
      });
}
