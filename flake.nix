{
  description = "neuron-pokopoko development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, fenix, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        fenixPkgs = fenix.packages.${system};

        # stable + wasm32(Worker 用)。バージョンは flake.lock で固定される
        rustToolchain = fenixPkgs.combine [
          fenixPkgs.stable.cargo
          fenixPkgs.stable.clippy
          fenixPkgs.stable.rustc
          fenixPkgs.stable.rustfmt
          fenixPkgs.stable.rust-src
          fenixPkgs.targets.wasm32-unknown-unknown.stable.rust-std
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.rust-analyzer
            pkgs.just
            pkgs.jq
            pkgs.nodejs_latest
            pkgs.pnpm
            pkgs.wrangler
            pkgs.worker-build # wrangler のカスタムビルド。worker crate とバージョンを揃える
            pkgs.binaryen # wasm-opt
            pkgs.imagemagick # magick
            pkgs.actionlint # GitHub Actions
          ];

          shellHook = ''
            echo "neuron-pokopoko dev shell"
            echo "  rust: $(rustc --version)"
            echo "  node: $(node --version), pnpm $(pnpm --version)"
          '';
        };
      });
}
