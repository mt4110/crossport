{
  description = "crossport dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;
      in {
        devShells.default = pkgs.mkShell {
          # ここに「Cツールチェーン＋ライブラリ」を全部まとめる
          buildInputs = [
            pkgs.rustup
            pkgs.pkg-config
            pkgs.openssl
            pkgs.libiconv
            pkgs.nixfmt-classic
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.IOKit
            pkgs.darwin.apple_sdk.frameworks.CoreFoundation
            pkgs.darwin.apple_sdk.frameworks.Security
          ];

          # Rust は stable 固定
          RUSTUP_TOOLCHAIN = "stable";

          # 🔥 一番大事：リンカに libiconv の場所をちゃんと教える
          LIBRARY_PATH = lib.makeLibraryPath [ pkgs.libiconv ]
            + (if builtins.getEnv "LIBRARY_PATH" == "" then
              ""
            else
              ":" + builtins.getEnv "LIBRARY_PATH");

          shellHook = ''
            echo "🚀 crossport dev shell"
            echo "   rustup default stable してね（まだなら）"
          '';
        };
      });
}
