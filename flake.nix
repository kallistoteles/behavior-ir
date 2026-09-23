{
  description = "Deterministic AI system: verifiable Behavior IR core";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system:
        f (import nixpkgs { inherit system; overlays = [ rust-overlay.overlays.default ]; }));
    in
    {
      devShells = forAll (pkgs:
        let
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          python = pkgs.python313.withPackages (ps: [ ps.pytest ps.mypy ps.jsonschema ps.pip ]);
        in
        {
          default = pkgs.mkShell {
            packages = [ rust python pkgs.maturin ];
            shellHook = ''
              if [ ! -d .venv ]; then
                ${python}/bin/python -m venv --system-site-packages .venv
              fi
              export VIRTUAL_ENV="$PWD/.venv"
              export PATH="$VIRTUAL_ENV/bin:$PWD/target/debug:$PATH"
            '';
          };
        });
    };
}
