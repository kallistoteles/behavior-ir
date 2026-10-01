# Contract: Release, Binding and Versions

## Release layout

```text
dist/v0.8.0/
├── behavior-0.8.0-cp313-abi3-manylinux_2_28_x86_64.whl
├── SHA256SUMS
└── release-manifest.json
```

`release-manifest.json` (canonical JSON):

```json
{
  "format": "behavior.release_manifest.v1",
  "release": "0.8.0",
  "tag": "v0.8.0",
  "commit": "<git commit hash>",
  "versions": {
    "engine": "0.8.0",
    "wire_ir": ["0.1", "0.2", "0.3", "0.4", "0.5", "0.6"],
    "records": ["0.4", "0.5", "0.6"],
    "store_documents": ["behavior.commit_bundle.v1", "…"],
    "verifier": "0.4.0"
  },
  "bindings": {"python": {"version": "0.8.0", "requires_python": ">=3.13"}},
  "platforms": ["manylinux_2_28_x86_64"],
  "artifacts": [{"file": "behavior-0.8.0-….whl", "sha256": "…"}],
  "solver": {"name": "z3", "version": "4.16.0"}
}
```

## Consumer install (Python)

```text
# requirements.txt of the consumer project
behavior @ file:///path/to/behavior-0.8.0-cp313-abi3-manylinux_2_28_x86_64.whl \
    --hash=sha256:<from SHA256SUMS>
```

`pip install --require-hashes -r requirements.txt`. An https URL to the uploaded asset works the
same way. No Rust toolchain is needed. The console script `behavior` is installed into the
environment.

## Version reporting

- `behavior.__version__` gives `"0.8.0"`, the binding version.
- `behavior.versions()` gives the manifest's `versions` object plus
  `"binding": {"python": "0.8.0"}`.
- `behavior engine-info` (CLI) prints the same object as canonical JSON and exits 0. It is new; `behavior version <wire>`, which prints a module's behavior version, is unchanged.
- At import, if the binding version differs from the engine version, `ImportError` names both.

## Solver prerequisite

- Found through `BEHAVIOR_Z3`, else `z3` on `PATH`.
- If missing, the CLI prints `behavior: verification needs the Z3 SMT solver (supported: 4.16.0);
  install z3 on PATH or set BEHAVIOR_Z3` and exits 3. Python raises `BehaviorError` with the same
  text.
- A different solver version produces a warning (stderr or `warnings.warn`). The attestation
  records the version that ran.
- `admit`, `eval`, `replay`, stores and conformance never need the solver.

## Release scripts

- `scripts/release.sh <version>`:
  1. It refuses unless the tree is clean and the workspace version equals `<version>`.
  2. It builds `dist/v<version>/`.
  3. It runs `scripts/release-check.sh`.
  4. On success it creates the annotated tag `v<version>`. Pushing and uploading are manual.
- `scripts/release-check.sh [dist-dir]`: the checks of research R9. It exits non-zero and names
  the first failing check.

## Versioning policy

See `docs/versioning.md` (research R13). Engine and binding versions are equal. The wire IR,
record, store document and verifier versions move independently and are reported, never
implied.
