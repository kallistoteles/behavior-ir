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
      packages = forAll (pkgs: {
        # Feature 510 uses the version and source already fixed by flake.lock.
        safety-proof-toolchain = pkgs.lean4;
      });

      devShells = forAll (pkgs:
        let
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          python = pkgs.python313.withPackages (ps: [ ps.pytest ps.mypy ps.jsonschema ps.pip ps.auditwheel ]);
        in
        {
          default = pkgs.mkShell {
            packages = [ rust python pkgs.maturin pkgs.z3 pkgs.zig pkgs.cargo-zigbuild pkgs.gh pkgs.lean4 ];
            shellHook = ''
              if [ -n "''${BEHAVIOR_LEAN_BINARY:-}" ] || [ -n "''${BEHAVIOR_LEANCHECKER_BINARY:-}" ]; then
                echo "Safety toolchain overrides are unsupported; use the locked Nix proof toolchain." >&2
                exit 1
              fi
              export BEHAVIOR_SAFETY_TOOLCHAIN="${pkgs.lean4}"
              # Identity metadata must not pull the tool's entire build-time source closure.
              export BEHAVIOR_SAFETY_TOOLCHAIN_DRV="${builtins.unsafeDiscardStringContext pkgs.lean4.drvPath}"
              if [ ! -d .venv ]; then
                ${python}/bin/python -m venv --system-site-packages .venv
              fi
              export VIRTUAL_ENV="$PWD/.venv"
              export PATH="$VIRTUAL_ENV/bin:$PWD/target/debug:$PATH"
              export BEHAVIOR_Z3="${pkgs.z3}/bin/z3"
            '';
          };
        });
    };
}
