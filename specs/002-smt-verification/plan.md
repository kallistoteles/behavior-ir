# Implementation Plan: SMT Verification of Behavior Modules

**Branch**: `002-smt-verification` | **Date**: 2026-09-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-smt-verification/spec.md`; builds on
`specs/001-verifiable-behavior-ir` and `PRINCIPLES.md`.

## Summary

Add formal verification to the Behavior IR engine. A new crate, `behavior-verify`, translates an
admitted module into one SMT-LIB 2 query per check (invariant and constraint preservation,
postconditions, reachable evaluation errors, dead actions, redundant preconditions, vacuous
rules), asserts exactly what the runtime guarantees before evaluation, runs the pinned Z3 binary
with a deterministic resource budget, confirms every counterexample by evaluating it, caches
results by content hash, and issues a content-addressed verification attestation. Governance
objects (waivers with detached Ed25519 signatures, a declarative execution policy, commit
authorizations) decide whether a proposed transition may be committed without changing
verification results. The runtime of feature 001 is extended so that the verifier's
assumptions hold: entity constraints checked on every incoming entity (`INVALID_CONTEXT`), state
identities required to be distinct (`INVALID_BINDING`), and change sets that address state
cells.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python 3.13 (binding and DSL).

**Primary Dependencies**: existing (feature 001) plus `ed25519-dalek` (signatures) and the Z3
4.16.0 binary from the flake's nixpkgs, driven as a subprocess (research R1). No native solver
bindings.

**Storage**: files: verification cache directory, attestations, waivers, policies (research R8).

**Testing**: `cargo test` with fixture modules containing seeded defects, run against the real
pinned Z3 (deterministic, research R2), counterexample confirmation through the evaluator, governance fixtures signed with fixed test keys; `pytest`; determinism script extended
to verification.

**Target Platform**: Linux (NixOS dev shell).

**Project Type**: library + CLI + Python binding (same workspace as feature 001).

**Performance Goals**: example modules verify in < 10 s (SC-003); re-verification after one
changed action in a 50-action module in < 20% of a full run (SC-004).

**Constraints**: byte-identical attestations for identical inputs (resource budget, fixed seed,
single thread); no false proofs (sound over-approximation for decimals, research R5); every
reported counterexample reproduces.

**Scale/Scope**: modules of tens of actions; 4 user stories; 5 check kinds; 4 governance objects.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic Core | Pass | Verification never decides a transition; the policy evaluator is deterministic code over declarative data. No AI involved. |
| II. AI output validated | Pass (n/a) | No AI in this feature. Solver output is treated as untrusted too: every `sat` answer is confirmed by the evaluator (R7). |
| III. Test-First | Pass (process) | Seeded-defect fixtures, expected findings, and governance cases are written before the verifier; tasks order tests first. |
| IV. Reproducibility | Pass with justification | The solver is an external process: pinned by Nix, fixed seed, one thread, deterministic `rlimit` budget; wall-clock guard results are marked non-reproducible and never cached (R2). `now` for waiver expiry is caller-supplied and recorded. |
| V. Explicit state and auditability | Pass | Attestations and commit authorizations are content-addressed, cite every hash involved, and embed confirming decision records. |
| VI. Simplicity | Pass with justification | One new crate keeps the solver out of `behavior-core`; SMT-LIB text instead of bindings. See Complexity Tracking. |
| Tech constraints | Pass | Rust, fmt/clippy gates, no `unsafe`, typed errors, exact numerics (R5). |

Checked against `PRINCIPLES.md`: verification is derived from semantics without annotations
(§10); the verifier assumes exactly what the runtime guarantees (§10, R4, R11); identity is
semantic and hidden aliasing is forbidden (§2, R11); identity claims are data and authority
requires evidence (§10, R10); governance objects are content-addressed (§6, R9).

**Post-design re-check (after Phase 1)**: all rows pass; no further dependencies were added.

## Project Structure

### Documentation (this feature)

```text
specs/002-smt-verification/
├── plan.md
├── research.md                 # R1–R13
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── verification-report.md  # attestation v1
│   ├── governance.md           # waiver, signed attestation, policy, commit authorization
│   └── engine-api.md           # wire IR 0.2 constraints, Rust/CLI/Python additions
└── tasks.md                    # /speckit-tasks
```

### Source Code (repository root)

```text
flake.nix                             # + z3 in the dev shell
crates/
├── behavior-core/                    # feature 001, extended:
│   └── src/
│       ├── wire.rs                   #   ir_version 0.2, `constraints`
│       ├── admit/                    #   ConstraintItem, hash tag, module entry kind 7
│       ├── eval.rs                   #   binding check, constraint checks, INVALID_CONTEXT
│       ├── record.rs                 #   record_version 0.2, state-cell change entries
│       └── serialize.rs              #   constraints
├── behavior-verify/                  # new
│   ├── src/
│   │   ├── lib.rs                    #   verify(), Profile, Attestation
│   │   ├── smt.rs                    #   SMT-LIB writer and s-expression reader
│   │   ├── encode.rs                 #   semantic IR → terms (types, derived, S/S', rounding)
│   │   ├── checks.rs                 #   the check kinds (research R6)
│   │   ├── solver.rs                 #   Solver trait, Z3Process
│   │   ├── confirm.rs                #   model → request → evaluate → confirm
│   │   ├── cache.rs                  #   check keys and cache directory
│   │   ├── governance.rs             #   waiver, signed attestation, policy, authorize()
│   │   └── hashing.rs                #   finding, attestation, governance hashes
│   └── tests/
├── behavior-cli/                     # + verify, authorize, waiver-hash
└── behavior-py/                      # + verify, authorize bindings
python/behavior/                      # + @constraint, verify(), authorize(), Profile
tests/fixtures/
├── verify/                           # seeded-defect modules and expected findings
└── governance/                       # policies, waivers, signed attestations, test keys
examples/tryout/                      # demo (+ dump.py) used by quickstart §2
```

**Structure Decision**: verification and governance live in a new crate so `behavior-core`
remains solver-free; the core gains only what the runtime/verifier contract requires (entity
constraints, binding check, state-cell change sets).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| External solver process (Z3 binary) | Deciding nonlinear real and integer arithmetic needs a mature SMT solver | Native bindings add a C build to every crate and the Python wheel; no pure-Rust solver handles the required theories |
| New crate `behavior-verify` | Keeps the solver dependency and governance code out of the core | Putting it in `behavior-core` would make every user of the engine depend on Z3 |
| `ed25519-dalek` dependency | Clarification Q5 requires cryptographic evidence of reviewer identity | Name matching (no evidence) was rejected by the user |
