# Implementation Plan: Relational Queries and Set Semantics

**Branch**: `007-relational-queries` | **Date**: 2026-09-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/007-relational-queries/spec.md`, with the
clarifications of 2026-09-27:
- query filters are candidate-local;
- relational invariants live at module level only;
- verification uses a symbolic evaluated state and exact changes;
- queries are inline expressions.

## Summary

Add typed, read-only set comprehensions over one entity type: `select(T)`, `where`, set algebra,
`count`, `any`, `all`, `sum`, `min`, `max` and `unique`, plus module-level invariants.

**Evaluation.** A query is answered through the existing evaluation-facts interface, which gains
`query` (membership of an instance) and `field` (a member value). Instances are identified by a
definition hash plus canonical captured values. Only observed facts are recorded, in records 0.6:
membership in `facts.queries`, member values in `facts.fields`. Short-circuiting operations visit
members in canonical identity order. Resulting-state results follow one rule,
`Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS`, where the baseline is an observed
query fact against S. Records list complete memberships (large results make large records; member
field reads add to them), which is accepted for this feature.

**Persistence.** The store answers queries as of the evaluated position from entity versions,
using new backend primitives `keys_at` (type index) and `keys_by_field_at` (optional field index).
Indexes are derived and never part of the state identity. Commits re-derive facts at the parent.
Module invariants are checked at genesis and on every resulting state they could be affected by,
with uniqueness decided by a delta rule.

**Verification.** Per query instance, symbolic summaries (count, sums, satisfying counts,
extrema) over S, constrained by the classified bound entities; exact deltas for a fixed instance;
fresh summaries for a changed instance; runtime-confirmed counterexamples concretized with explicit
unknown-member slots.

**Compatibility.** Any 007 form requires wire IR 0.6; everything existing keeps its bytes and
identity.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python 3.13.

**Primary Dependencies**: unchanged (Z3 4.16.0 for verification; no new crates).

**Storage**: host backends via the feature-005/006 contract, with two added primitives: `keys_at`
and `keys_by_field_at` (optional).

**Testing**:
- `cargo test`:
  - admission of the new forms and scope rules (`QUERY_NOT_ALLOWED`, `NON_LOCAL_PREDICATE`);
  - evaluation with supplied query and field facts: empty sets, exact sums, optional extrema,
    short-circuit order, `UNKNOWN_FACT`, `INCONSISTENT_FACTS`;
  - resulting-state derivation against full re-evaluation (proptest, SC-004);
  - store queries with and without indexes, iteration-order permutations (SC-002, SC-008);
  - module invariants (genesis, delta uniqueness, dependency skip soundness);
  - 4 new conformance cases with mutants.
- Verifier fixtures covering proven, counterexample and inconclusive outcomes (SC-005).
- `proptest` for SC-002/003/004/008; ignored perf test for SC-007.
- `pytest`, determinism script; all existing goldens byte-identical (SC-006).

**Target Platform**: Linux (NixOS dev shell).

**Project Type**: library + CLI + Python binding.

**Performance Goals**: a selective query (≤ 10 matches) over 100,000 entities under 50 ms with an
index for its predicate field (SC-007); cost independent of non-matching entities.

**Constraints**:
- Candidate-local predicates; no nested queries.
- Module invariants are closed state expressions (no inputs, context, parameters or invocation
  captures).
- `sum` only over additive numeric types with a canonical zero; the empty sum is that exact zero.
- Unknown-member slots in the verifier are witnesses only, never a finite-universe bound on proofs.
- Only observed facts are recorded; canonical order for short-circuiting.
- Resulting-state results are derived, never materialized in storage.
- Indexes never define results or enter the state identity.
- Existing identities and bytes are unchanged.

**Scale/Scope**: 4 user stories; 11 expression forms and module invariants; 2 fact kinds; record
and wire 0.6; 2 backend primitives; verifier summaries; 4 conformance cases.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | Sets are canonical; short-circuiting uses canonical identity order; exact arithmetic in aggregates; no dependence on backend iteration order |
| II. AI output validated | Pass (n/a) | No AI |
| III. Test-first | Pass (process) | Admission tables, fact semantics, derivation proptests, conformance cases, mutants and verifier fixtures are written and seen failing first |
| IV. Reproducibility | Pass | Membership and member values are recorded; replay needs no store; records without queries keep their bytes |
| V. Explicit state and auditability | Pass | Every set a decision depended on is visible in its record and read set |
| VI. Simplicity | Pass with justification | Queries are expressions (no new named item kind); one facts interface; derived values provide reuse. Module invariants are the one new item variant, justified below |
| Tech constraints | Pass | Rust gates, no new dependencies, no `unsafe` |

**Post-design re-check (after Phase 1)**: all rows pass.

## Project Structure

### Documentation (this feature)

```text
specs/007-relational-queries/
├── plan.md
├── research.md            # R1–R14
├── data-model.md
├── quickstart.md
├── contracts/
│   └── query-api.md       # DSL, engine facts, store/backend, errors
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── behavior-core/src/
│   ├── wire.rs, serialize.rs              # 0.6 forms, module invariants
│   ├── admit/{resolve,typecheck,hash}.rs  # lambda scopes, candidate-local checks, Query<T>, hashes
│   ├── semantic/{expr,module,types}.rs    # relational ExprKinds, Query type, module invariants
│   ├── facts.rs                           # query and field facts, snapshot checks
│   ├── query.rs                           # new: instances, captures, S' derivation, delta uniqueness
│   └── eval.rs, record.rs                 # relational evaluation, invariant_global, record 0.6
├── behavior-verify/src/{encode,checks}.rs # query summaries, deltas, slot concretization
├── behavior-store/src/{store,memory,conformance,replay}.rs  # query answering, indexes, genesis invariants
├── behavior-cli/                          # 0.6 modules and requests with query facts
└── behavior-py/                           # builder nodes, backend primitives
python/behavior/                           # select/where/count/any_/all_/sum_/min_/max_/unique
examples/orders/                           # customers, orders, employees
tests/fixtures/                            # orders wire, requests/007, verify fixtures
```

**Structure Decision**: no new crate. Query semantics live in one new core module (`query.rs`)
shared by the evaluator and the store.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Module-level invariants (a new invariant variant) | Relational validity belongs to the whole state (clarification Q2) | Per-entity relational invariants would need reverse dependency sets for every entity |
| Two backend primitives (`keys_at`, optional `keys_by_field_at`) | A store must enumerate a type as of a position, and SC-007 needs an index | Engine-side enumeration of everything breaks SC-007; host query languages break FR-022 |
| Delta rule for `unique` on S' | Keeps uniqueness checks proportional to the change | Re-reading every member per commit is linear in the store size |
