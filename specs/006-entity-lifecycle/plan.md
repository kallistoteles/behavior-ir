# Implementation Plan: Entity Lifecycle (Universe Transitions)

**Branch**: `006-entity-lifecycle` | **Date**: 2026-09-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/006-entity-lifecycle/spec.md`, with the clarifications
of 2026-09-27:
- an identity names one lifetime;
- `exists(Id<T>)` is the single existence predicate, with `Ref<T>` as sugar;
- facts are observed state reads;
- the reverse-reference index is derived, and integrity is checked on S'.

## Summary

Make the entity universe part of state. Actions gain two effects, `create(T, id, {complete
value})` and `remove(p)`, and two predicates, `exists(id)` and `referenced(id)`. `Ref<T>` compiles
to `Id<T>` plus an existence constraint, so there is one existence semantics.

**Evaluation facts.** Evaluation reads existence and incoming-reference facts (current state), and
identity-registry facts (history), through one `EvaluationFacts` interface:
- plain evaluation takes them from the request's `facts` section;
- the store answers them from the backend as of the evaluated position;
- replay uses the recorded facts.

Together with the state, these facts form the evaluation snapshot, since two positions with the
same state identity can differ in history facts. Supplied facts must form a valid snapshot.
`referenced(id)` is a projection of the incoming-reference fact. Every fact actually observed goes
into the decision record and the read set. Facts about the
resulting state S' are derived from S's facts plus ΔS, so referential integrity is checked on S'.

**Persistence.**
- Creations insert and removals remove content hashes in the state accumulator.
- The identity registry is derived from the entity versions: a removal never deletes versions.
- A derived reverse-reference index, kept as as-of edge events, answers incoming references at
  any position.
- Removals and index changes join the atomic commit.

**Verification.** Existence, the registry and unbound referrers are modelled with uninterpreted
functions per entity type, with the new blocking check `referential_integrity`.

**Compatibility.** Any use of a form introduced by this feature requires wire IR 0.5; records
that use one are record 0.5. A transition has at most one lifecycle operation per typed identity. Every
existing module, record, golden and persistence document keeps its bytes and identity.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python 3.13.

**Primary Dependencies**: unchanged (Z3 4.16.0 for verification; no new crates).

**Storage**: host backends via the feature-005 contract, with three added primitives:
`removed_at`, `incoming_at`, and `used_at` (the last with a default implementation).

**Testing**:
- `cargo test`:
  - admission of lifecycle forms;
  - evaluation with facts: the S/S' semantics, short-circuited facts, `UNKNOWN_FACT`;
  - store commit, collisions, removal, integrity on S', index consistency;
  - 7 new conformance cases, with mutants: an index that diverges, removal that deletes versions,
    and a registry that forgets removed identities.
- Verifier fixtures for `lifecycle.json`.
- `proptest` for:
  - SC-002 (mixed-history tamper detection);
  - SC-003 (colliding concurrent creations);
  - SC-007 (re-creation attempts).
- An ignored perf test for SC-004.
- `pytest`, the determinism script, and all existing goldens byte-identical (SC-006).

**Target Platform**: Linux (NixOS dev shell).

**Project Type**: library + CLI + Python binding.

**Performance Goals**: creating or removing one entity in a store with 100,000 entities in under
50 ms, proportional to the changes (SC-004). The index lookup costs time proportional to the
incoming references of the target.

**Constraints**:
- No engine-generated identities.
- Only observed facts are recorded.
- S' facts are derived, never read from storage.
- The index is never part of the state identity.
- Existing identities and bytes are unchanged.

**Scale/Scope**: 4 user stories; 4 language forms, an evaluation-facts interface, record and wire 0.5,
persistence extensions (3 backend primitives, 3 optional record fields), a verifier extension, and
7 conformance cases.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | Identities are inputs, never generated. Facts come from one oracle at one position, and S' facts are derived deterministically. There is no AI. |
| II. AI output validated | Pass (n/a) | No AI. |
| III. Test-first | Pass (process) | Admission tables, fact-semantics tests, conformance cases, mutants and verifier fixtures are written and seen failing first. |
| IV. Reproducibility | Pass | Every observed fact (field, existence, identity, reference) is recorded, so replay needs no live store. Records and bytes of existing modules are unchanged. |
| V. Explicit state and auditability | Pass | Universe changes are explicit effects in records and history. Removal keeps history, and the registry makes identity permanent. |
| VI. Simplicity | Pass with justification | One existence mechanism (`Ref<T>` is sugar); one fact oracle for three contexts; the index is derived, not canonical. The added backend primitives are justified below. |
| Tech constraints | Pass | Rust gates, no new dependencies, no `unsafe`. |

**Post-design re-check (after Phase 1)**: all rows pass.

## Project Structure

### Documentation (this feature)

```text
specs/006-entity-lifecycle/
├── plan.md
├── research.md            # R1–R14
├── data-model.md
├── quickstart.md
├── contracts/
│   └── lifecycle-api.md   # DSL, engine EvaluationFacts, store/backend, errors
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── behavior-core/src/
│   ├── wire.rs, serialize.rs           # 0.5 forms: create/remove effects, exists/referenced, ref type
│   ├── admit/{resolve,typecheck,hash}.rs  # Ref<T> sugar, complete-value check, lifecycle conflicts, hash codes
│   ├── semantic/{expr,module,types}.rs # ExprKind::Exists/Referenced, Effect::Create/Remove, reference flag
│   ├── facts.rs                        # new: EvaluationFacts (state + history facts), observed facts, request/record facts sections
│   └── eval.rs, record.rs              # S/S' fact semantics, lifecycle effects, integrity on S', record 0.5
├── behavior-verify/src/{encode,checks}.rs  # ex_T/used_T/refd_T, creation/removal, referential_integrity
├── behavior-store/src/{documents,store,replay,memory,conformance}.rs  # registry, removals, derived index
├── behavior-cli/                       # verify/eval accept lifecycle modules (facts in requests)
└── behavior-py/                        # create/remove/exists/referenced/Ref; backend primitives
python/behavior/                        # DSL forms, evaluate(facts=...)
examples/accounts/                      # customers + accounts lifecycle example
tests/fixtures/                         # lifecycle wire, requests, verify fixtures, goldens
```

**Structure Decision**: no new crate. The fact oracle is a core module used by the evaluator, the
store and replay.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Three new backend primitives (`removed_at`, `incoming_at`, `used_at` with a default) | Existence and references must be answerable as of the evaluated position (consistent snapshot) without scanning the universe | Current-only answers break snapshot consistency; scanning breaks SC-004 |
| A `referenced(id)` predicate beyond the spec's `exists` | Without it, no removal of a referenced type can be proven safe | Relying on runtime refusal only leaves every such removal as a verification finding |
