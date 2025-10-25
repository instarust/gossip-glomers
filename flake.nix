{
  nixConfig.bash-prompt-prefix = ''(gossip) '';

  inputs = {
    systems.url = "github:nix-systems/default";
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: let
    forAllSystems = f:
      inputs.nixpkgs.lib.genAttrs
      (import inputs.systems)
      (system:
        f (import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        }));
  in {
    devShells = forAllSystems (pkgs: let
      rust-toolchain =
        pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    in {
      default = pkgs.mkShell {
        packages = [
          (rust-toolchain.override
            {extensions = ["rust-src" "rust-analyzer" "clippy"];})

          pkgs.maelstrom-clj
        ];
        shellHook = ''echo "with löve from wrd :)"'';
      };
    });
  };
}
