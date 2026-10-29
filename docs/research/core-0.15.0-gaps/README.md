# Core 0.15.0 — Gap 008 and Gap 009 investigation

Status: local reproductions and upstream drafts for review, 2026-10-24. No issues filed,
Core changes, release changes or production fixes. Gap 006 remains separate.

Baseline: published Behavior **0.14.0**, Core **0.15.0** commit
`6b93898a9375113f7a8946f23ab5535319b6f5d6`, verifier **0.10.0**, Z3 **4.16.0**.
The Python cases use the installed published wheel, SHA256
`794d2dd5ddf8613c52f524a1b4b74c0246d86a5ea52414424ab89ef104e86513`.
The Rust probe depends only on the public `behavior-engine` facade and `serde_json`,
with its own lock derived from the ecosystem lock. It never patches or imports private Core crates.

## Findings

| Gap | Evidence | Classification |
| --- | --- | --- |
| [008](gap-008.md) | Delegating public Backend counts: after a commit, a literal global `True` causes N members to be enumerated again. Local control: zero enumerated members. | Reproduced conservative optimization-coverage limitation; not an established validity or semantic regression. |
| [009](gap-009.md) | Public verifier produces an inconclusive preservation check with the exact complete-world removal encoding error, even with a required `not_(referenced(...))` guard. | Reproduced conservative verification limitation; no false proof observed. |

The observations do not reproduce a private application, establish its dominant cost or approve
a one-minute/30-minute endurance run. Short data and behavior replay checks pass; crash recovery,
lost acknowledgements, sustained load, durable backends and concurrent commits were not tested here.

## Reproduction

Use the released flake for the Rust/Z3 tools, and a clean installed package rather than this
workspace's older editable environment:

```sh
nix develop github:kallistoteles/behavior-ir/v0.14.0
python3 -m venv /tmp/behavior-core-gap-env
/tmp/behavior-core-gap-env/bin/python -m pip install --no-deps \
  'https://github.com/kallistoteles/behavior-ir/releases/download/v0.14.0/behavior-0.14.0-cp313-abi3-manylinux_2_28_x86_64.whl#sha256=794d2dd5ddf8613c52f524a1b4b74c0246d86a5ea52414424ab89ef104e86513'
```

From the ecosystem repository root, create a new output directory:

```sh
/tmp/behavior-core-gap-env/bin/python docs/research/core-0.15.0-gaps/reproduce.py \
  --output /tmp/core-gap-fixtures
```

This runs the Gap 009 assertions and writes three admitted Gap 008 modules. Normal exit 0 means
the observation/control harness completed, **not** that the Core limitations are resolved.
The precise red-capable Gap 009 invocation is:

```sh
/tmp/behavior-core-gap-env/bin/python docs/research/core-0.15.0-gaps/reproduce.py \
  --output /tmp/core-gap-red --require-supported
```

Observed exit **1**: `Gap009: guarded Ref-target removal is not verified`.
Every invocation requires a fresh Python output directory. After generating fixtures, the tight
Gap 008 signal is:

```sh
cargo run --locked --manifest-path docs/research/core-0.15.0-gaps/store-probe/Cargo.toml -- \
  /tmp/core-gap-fixtures /tmp/core-gap-minimal.json --minimal --require-local-promotion
```

Observed exit **101** after successful compilation/execution: `Gap008: even a constant global
invariant prevents incremental promotion`. Local post-commit enumeration: **0**; global `True`: **16**.
This is a proposed optimization acceptance signal, not a claim that Core promises this optimization today.
Compilation is separate from probe execution. Full matrix:

```sh
cargo run --locked --manifest-path docs/research/core-0.15.0-gaps/store-probe/Cargo.toml -- \
  /tmp/core-gap-fixtures /tmp/core-gap-scaling.json
```

No production instrumentation or fix is applied. The Backend wrapper forwards all methods to
Core's actual `InMemoryBackend`; counters measure selected public calls, not every internal backend
operation. Debug wall timings in JSON include wrapper overhead and are not a production performance
profile. Replay happens after copying the measured counts.

## Evidence and integrity

[evidence/](evidence/) contains the final compact reports, admitted neutral module fixtures,
actual attestations, red-signal logs and provenance/hashes. Earlier incomplete harness attempts
remain in task-owned `target/core-gap-triage/` outputs and are listed in provenance; they are
not passing controls or reproduced Core defects. The two final Python runs reproduce the same
attestations. Backend-count repetitions are compared separately from noisy timings.

These are implementation observations and source correspondence, not formal proofs of a proposed
optimization. The drafts specify correctness requirements without selecting an algorithm or
approving a wider Core language.
