# Implementation Plan: Verifiable Behavior IR Core (v0.1)

**Branch**: `001-verifiable-behavior-ir` | **Date**: 2026-09-23 (revised after clarification
session) | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-verifiable-behavior-ir/spec.md`; design
principles from `PRINCIPLES.md`.

## Summary

Build the executable, analyzable core of the Behavior IR system. A Python DSL, which is a PyO3
binding to the Rust engine, constructs behavior (entities, nominal types, derived values, rules,
invariants, and transitions over state, input, and context): every operator calls the engine's
builder, which type-checks the node immediately, so ill-typed expressions fail at the author's
line (revised 2026-09-25, research R10, R12, R17). Untrusted **wire IR** (JSON, from files or
other frontends) is **admitted** by the same pipeline the builder finishes through (strict
decode, name resolution, cycle detection, type checking, explicit conversions, recursive
domain-separated content hashing) into a **semantic Behavior IR** that cannot be constructed any
other way. The engine serializes admitted modules to canonical JSON. The
engine evaluates transitions deterministically (invariants on S, preconditions, ΔS,
postconditions and invariants on S'), emits decision records that cite item and predicate
hashes, replays them, and checks structured intents whose context is supplied by the trusted
host. Python holds engine objects (builder, nodes, modules) through PyO3; the CLI exposes the
same engine over JSON files.
Formal verification is deferred (research R16).

## Technical Context

**Language/Version**: Rust stable (pinned in `rust-toolchain.toml`, edition 2024) for the
engine; Python 3.13 for the authoring layer.

**Primary Dependencies**: Rust: `serde`, `serde_json`, `rust_decimal`, `sha2`, `thiserror`,
`pyo3`, `clap`; build: `maturin` (research R15).

**Storage**: N/A. Wire IR and fixtures are files in Git; the engine stores no data.

**Testing**: `cargo test` with `proptest`, frozen hash vectors, and golden JSON fixtures;
`pytest` for the DSL and end-to-end; a typing-case table run against the engine's single type
checker; serialization round-trip tests;
a determinism script that runs golden and replay cases twice.

**Target Platform**: Linux (NixOS dev shell via Nix flake).

**Project Type**: library (Rust workspace + Python package) with a CLI.

**Performance Goals**: admit 200 derived values + 50 actions in < 2 s; evaluate one action in
< 10 ms (SC-005).

**Constraints**: byte-identical wire IR, admission results, and decision records across runs
and machines; hashes independent of serialization, source metadata, and human names; exact
numerics; no floats; no timestamps in outputs.

**Scale/Scope**: modules up to a few hundred declarations; 3 user stories; 1 frontend.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic Core, Probabilistic Edge | Pass | No AI in v0.1. All decisions are made by the Rust evaluator. The capability boundary accepts only capability, targets, and input from an intent; state and context come from the trusted host (R13). |
| II. Every AI Output Is Validated | Pass | Intents are strictly decoded into typed structures; all problems are listed as typed errors; targets must match supplied state ids. No repair by guessing. |
| III. Test-First | Pass (process) | Tasks put tests first per story: hash vectors and hash property tests, typing-case table, invalid wire fixtures, decision-record goldens, recorded intents. No live model. |
| IV. Reproducibility and Replay | Pass | Semantic hashing (R4, R5), canonical JSON for outputs, ordered collections only, no clock/randomness/env in the engine, relative source paths, replay (R14). |
| V. Explicit State and Auditability | Pass | Result state machine (data-model.md); every evaluation records each predicate with its hash, values read, outcome, and source location; illegal semantic IR unrepresentable by construction (R2). |
| VI. Simplicity | Pass with justification | One core crate + CLI + binding; hashing and admission are the minimum needed for the clarified identity and typing requirements. Second language justified below. |
| Tech: fmt, clippy, Cargo.lock, no unsafe, typed errors | Pass | `#![forbid(unsafe_code)]` in `behavior-core` and `behavior-cli`; `behavior-py` has no hand-written `unsafe`. |
| Tech: float determinism | Pass | Floats banned; exact decimals with documented rounding (R9). |
| Workflow gates | Pass | quickstart §1. |

The design also checks against `PRINCIPLES.md`: transitions return ΔS before anything is
applied (§2, §4); predicates see only their declared inputs (§3); behavior is data and the DSL
never evaluates (§5); identity is content-addressed with names and metadata outside the hash
(§6); references are explicit and the graph is derived (§8); invalid behavior cannot enter the
semantic model (§9); invariants hold before and after (§11).

**Post-design re-check (after Phase 1)**: all rows pass. The added contract (hashing.md) and
admission pipeline introduced no new dependencies or components.

**Re-check after the 2026-09-25 revision** (Python as a binding): all rows pass; Simplicity
improves (one type checker, one serializer); no new dependencies.

## Project Structure

### Documentation (this feature)

```text
specs/001-verifiable-behavior-ir/
├── plan.md              # This file
├── research.md          # Decisions R1–R17
├── data-model.md        # Semantic IR, typing rules, results, record
├── quickstart.md        # Validation guide
├── contracts/
│   ├── ir-encoding.md   # Wire IR JSON (untrusted exchange form)
│   ├── hashing.md       # Semantic content hashing v1
│   ├── python-api.md    # Public authoring API
│   └── engine-api.md    # Rust / binding / CLI operations and payloads
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
Cargo.toml                    # workspace
Cargo.lock
rust-toolchain.toml
flake.nix                     # dev shell: rust toolchain, python 3.13, maturin, pytest, mypy
pyproject.toml                # maturin build; package `behavior`, extension `behavior._engine`

crates/
├── behavior-core/
│   ├── src/
│   │   ├── wire.rs           # wire IR structs (serde, strict), untrusted
│   │   ├── semantic/         # semantic IR: types with private fields, built only by admission
│   │   │   ├── types.rs      #   Type, nominal ops, typing rules
│   │   │   ├── expr.rs
│   │   │   └── module.rs     #   Module, items, name table
│   │   ├── builder.rs        # construction API used by the Python binding (research R17)
│   │   ├── serialize.rs      # admitted module → canonical wire JSON
│   │   ├── admit/            # decode → resolve → graph/cycles → typecheck → convert → hash
│   │   │   ├── resolve.rs
│   │   │   ├── graph.rs
│   │   │   ├── typecheck.rs
│   │   │   └── hash.rs       #   contracts/hashing.md
│   │   ├── decimal.rs        # normalized decimal encoding
│   │   ├── eval.rs           # transition evaluation, ΔS, trace
│   │   ├── record.rs         # DecisionRecord, replay
│   │   ├── intent.rs         # capability boundary, target check
│   │   ├── pretty.rs         # expr_text
│   │   └── canonical.rs      # canonical JSON output
│   └── tests/                # hash vectors, hash properties, typing cases, goldens
├── behavior-cli/             # `behavior` binary
└── behavior-py/              # PyO3 module `behavior._engine`: Builder, Node, Module classes

python/
├── behavior/
│   ├── __init__.py           # public API (contracts/python-api.md)
│   ├── types.py              # type descriptors: Option, Id, nominal, Context, Input (no rules)
│   ├── expr.py               # operator overloading over engine node handles
│   ├── decl.py               # entity, field, derived, rule, invariant, action
│   ├── statements.py         # requires, ensures, set_, tracing context
│   ├── module.py             # BehaviorModule: drives the engine builder
│   ├── results.py            # AdmissionResult, Decision, ReplayResult
│   └── errors.py
└── tests/
    ├── fixtures/             # DSL fixtures (cycles.py, bad_types.py, …)
    ├── test_dsl.py
    ├── test_wire.py          # golden wire IR
    └── test_end_to_end.py

examples/
├── invoice/
└── project_margin/

tests/fixtures/               # shared by Rust, CLI, and Python tests
├── typing_cases.json
├── hash_vectors.json
├── versions.json
├── wire/{valid,invalid}/
├── requests/
├── intents/
├── host/
└── records/

scripts/determinism-check.sh
```

**Structure Decision**: a Cargo workspace with one core library crate holding all semantics
(wire decoding, admission, semantic IR, hashing, evaluation), a CLI crate, and a binding crate,
plus one Python package built by maturin. Shared fixtures in `tests/fixtures/` keep Rust, CLI,
and Python asserting against the same bytes. The semantic IR is a module whose constructors are
private to `admit/`, so invalid semantic IR cannot be built elsewhere in the crate. A future
`crates/behavior-verify/` consumes the semantic IR and can cache results by hash.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Second language (Python) next to the Rust stack | User-chosen authoring layer: typing/autocomplete, programmatic composition, no premature text syntax | Hand-written wire IR: syntax design before the model is known, poor authoring ergonomics |
| PyO3 binding crate | Python is a binding to the engine: typed nodes are built and checked in Rust as the author writes them | CLI via subprocess: slow, and could not give per-line type errors |

The earlier "two type checkers" entry was removed on 2026-09-25: typing now exists only in Rust
(research R17).
