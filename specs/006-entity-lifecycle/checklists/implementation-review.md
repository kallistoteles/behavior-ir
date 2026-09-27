# Implementation Review: Entity Lifecycle (Universe Transitions)

**Feature**: 006 · **Reviewed**: 2026-09-27 · **Tasks**: T001–T036

## Principles

- [x] **Identity is permanent; existence is state.**
  - The registry (`used_at`) is derived from entity versions and keyed by type name and id; removal
    never deletes versions. `ENTITY_ID_ALREADY_USED` for any used identity, in the decision (plain
    and store evaluation) and again at commit.
  - Existence enters the state identity: a creation inserts its content hash, and a removal removes
    the last one (`lifecycle_create.rs`, `lifecycle_remove.rs` compare against a fresh MuHash and
    a genesis without the entity).
- [x] **Referential integrity is explicit, not inferred from identity typing.**
  - `Ref<T>` is `Id<T>` plus a synthesized constraint `exists(field)`; `AuditNote.about: Id<Customer>`
    never blocks anything (`references.rs` in core and store; `references_to` lists only `Ref`
    fields).
- [x] **State is observed through facts; every fact that influences a decision is part of that
  decision's reproducible snapshot.**
  - `EvaluationFacts` (exists, used, incoming) is read lazily; only observed facts are recorded, in
    the record's `facts` section and the bundle's `read_facts`. A short-circuited `exists` records
    nothing (`facts.rs`).
  - Plain evaluation: `UNKNOWN_FACT` for a missing fact, `INCONSISTENT_FACTS` for supplied facts
    that cannot describe one valid state. Store: facts as of the evaluated position (conformance
    `existence_snapshot`, which the `CurrentOnlyIndex` mutant fails). Replay: the recorded facts.
- [x] **Indexes accelerate semantics; they do not define semantics.**
  - The reverse-reference index is edge events outside the state identity, written in the atomic
    commit, reconstructible from content (`replay_index`, conformance
    `reference_index_consistency`, which the `StaleIndex` mutant fails). The verifier models the
    invariant "no surviving `Ref` to an absent entity", not the index.
- [x] **State identity describes current content; history position may contribute additional facts
  that affect valid future transitions.**
  - `used` is a history fact; `data_version` names store, state and position.

## Constitution

| Principle | Status | Evidence |
|---|---|---|
| I. Deterministic core | ✓ | Identities are inputs; facts come from one position; S' facts are derived (`exists'`, `incoming'`); `BTreeMap` everywhere; determinism check covers the lifecycle history, the 006 requests and the accounts example |
| III. Test-first | ✓ | Tests for each story were written before their implementation (T003/T004, T008/T009, T013/T014, T017–T019, T023–T025) |
| IV. Reproducibility | ✓ | Records carry observed facts and lifecycle entries; plain replay, store behavior replay (facts re-derived at the parent), data replay, reference replay |
| V. Explicit state and auditability | ✓ | Creations, removals and reference changes are in records (`created`, `removed`, `ref_changes`) |
| VI. Simplicity | ✓ | One existence predicate; one facts interface for three contexts; no new crate or dependency |
| Tech constraints | ✓ | fmt, clippy `-D warnings`, no `unsafe`, no unwrap/expect outside tests |

## Requirements

| Requirement | Status | Evidence |
|---|---|---|
| FR-001 lifecycle effects in ordinary actions | ✓ | `create`/`remove` effects in wire 0.5, semantic IR, DSL |
| FR-002 complete initial value | ✓ | `CREATE_INCOMPLETE`, `UNKNOWN_FIELD`, `TYPE_MISMATCH` at admission (`create_incomplete`, `create_wrong_id_type`) |
| FR-003 host-supplied typed identity | ✓ | The identity expression must be `Id<T>`; the engine generates nothing |
| FR-004 removal of a bound state entity | ✓ | `remove_non_state` (`TYPE_MISMATCH`), context entities `EFFECT_ON_READONLY`; the removal entry carries the last value |
| FR-005 one lifecycle operation per identity | ✓ | Static `LIFECYCLE_CONFLICT` (same input twice, update + remove, remove twice) and dynamic (equal runtime ids, create of a removed id) |
| FR-006 atomic effects on S' | ✓ | `switch_and_remove` is valid; constraints/invariants of created entities on S'; removed entities leave outgoing rules |
| FR-007 collisions | ✓ | `ENTITY_ID_ALREADY_USED` (decision) and re-check at commit |
| FR-008 removal of an absent entity | ✓ | Bound entities must exist (`ENTITY_NOT_FOUND` at evaluation and commit) |
| FR-009 one lifetime | ✓ | `identity_never_reused` (mutant `ForgetsRemovedIdentities`), `lifecycle_props.rs` (SC-007) |
| FR-010 created entities satisfy constraints | ✓ | `open_account_unchecked` negative balance → `DENY` on `constraint_post` |
| FR-010a `exists` | ✓ | `Id<T>` and `Option<Id<T>>`; S before effects, S' after |
| FR-010a2 `referenced` | ✓ | Projection of `incoming`; `incoming'` on S' (`referenced_reads_s_before_and_s_prime_after`) |
| FR-010b `Ref<T>` sugar | ✓ | Synthesized constraint, carried by the entity hash (0x25), not serialized, not a name-table item |
| FR-010c observed existence reads | ✓ | `facts.rs`, store `read_facts` re-checked at commit |
| FR-010d trust boundary | ✓ | `docs/persistence.md` → Universe transitions → Trust boundary |
| FR-010e integrity on S' via a derived index | ✓ | Evaluation (`DANGLING_REFERENCE` with the references), commit (`check_integrity` against `incoming_at` at the parent) |
| FR-010f current-state vs history facts | ✓ | `exists`/`incoming` vs `used`; `data_version` |
| FR-010g supplied facts form a valid snapshot | ✓ | `inconsistent_facts_are_refused`, `a_bound_referrer_must_match_the_incoming_fact` |
| FR-011 existence in state identity | ✓ | Accumulator insert/remove |
| FR-012 revision 1 at the creating position | ✓ | `a_creation_commits_version_one_and_extends_the_state` |
| FR-013 removal keeps history | ✓ | `removed_entity_history` (mutant `DeletesVersionsOnRemove`); no placeholder version |
| FR-014 universe changes only through commits | ✓ | See below |
| FR-015 replay reconstructs creations and removals | ✓ | `replay_data` (registry, removals, created versions), `replay_behavior` (facts), `replay_index` |
| FR-016 verification of creating and removing actions | ✓ | `ex_T`/`used_T`/`refd_T`; `referential_integrity`; `lifecycle.rs` |
| FR-017 reproducible counterexamples | ✓ | Counterexamples carry `facts` and are confirmed by plain evaluation (`counterexamples_are_confirmed_with_their_facts`) |
| FR-018 read/write sets with existence | ✓ | `read_facts`, `write_lifecycle`; conflicts include lifecycle keys |
| FR-019 colliding creations under whole-state concurrency | ✓ | `concurrent_creation_same_identity`, `lifecycle_props.rs` |
| SC-001 authoring surface | ✓ | `examples/accounts/` (Python DSL: `create`, `remove`, `exists`, `referenced`, `Ref`) |
| SC-002 tamper detection | ✓ | `lifecycle_replay.rs` proptest: created value, removal, ref change, fact, registry, state, chain — each at its position with its kind; 10,000-transition variant (ignored, release) |
| SC-003 colliding concurrent creations | ✓ | `lifecycle_props.rs` (64 cases; 10,000 in the ignored release variant) |
| SC-004 cost proportional to the change | ✓ | `lifecycle_perf.rs`: ~4.2 ms per creation or removal at 1,000 and at 100,000 entities (release) |
| SC-005 seeded defects found, nothing inconclusive | ✓ | `lifecycle.expected.json`: both seeded defects found and confirmed; guarded versions proven |
| SC-006 existing identities and bytes unchanged | ✓ | `ir_0_5.rs` against `frozen_versions_006.json` (26 modules: behavior versions and item hashes); store hash vectors unchanged; every golden record replays; records without facts stay `0.4` |
| SC-007 no identity created twice | ✓ | `lifecycle_props.rs`, `identity_never_reused`, the registry tamper case |

### FR-014: no universe change outside a recorded transition

- [x] The `Backend` trait writes only in `create` (the genesis, with the seed's index) and `commit`
  (versions, removals, index changes, record and head in one compare-and-set). `removed_at`,
  `incoming_at`, `used_at`, `version_at`, `version`, `record`, `head` and `genesis` are reads.
- [x] `Store` offers no entity insert or delete: creations and removals exist only as decision
  record `lifecycle` entries, re-derived and checked at commit.
- [x] Conformance `create_and_remove_entity` checks, for every key the history names (plus a key it
  never names) at every position, that existence and `removed_at` are exactly what the records say.

## Deviations from the plan

1. **Effect representation.** The semantic IR keeps `Effect` (field updates) and adds
   `ActionItem::creates` / `removes` instead of an `Effect` enum (T006). Every existing consumer of
   field effects stays unchanged; action hashes append lifecycle effect hashes (own tags) after the
   field effects, so actions without them hash as before.
2. **Bundle field names.** `read_set` and `write_set` are arrays in feature 005 and part of hashed
   documents, so observed facts and lifecycle writes are new optional top-level bundle fields,
   `read_facts` and `write_lifecycle` (data-model: `read_set.existence` …, `write_set.lifecycle`).
3. **Reference replay is its own function.** `Ref` fields are entity declarations, which data replay
   does not have; `replay_index(store, module, from, to)` rebuilds and checks the index and
   `ref_changes`. Data replay checks the registry, removals, created versions and state.
4. **`switch_and_remove` is not provable.** T019 expected "proven"; the verifier finds a real,
   runtime-confirmed counterexample: an account not bound by the action may still reference the
   removed customer. The expectations record the counterexample.
5. **Verification fixture.** `lifecycle.expected.json` points at `wire/valid/accounts.json`
   instead of a copied `verify/lifecycle.json`.
6. **Reference constraints of bound state entities** are not re-checked on S (the store keeps
   integrity) and on S' only when an effect assigns the field; a removed target is caught by the
   integrity check. The verifier follows the same steps, so counterexample step indices match.
7. **Creation-only actions** (`register_customer`) may have no state parameter.
8. **`identity_collision_unreachable`** (R12) is not reported as a check: collision freedom is a
   path condition of the creation step, as at runtime.
9. **Snapshot rules are closed under subsets** (an edge implies its source exists; it is refused
   only if a supplied fact says otherwise), so a record's observed facts always pass on replay.
   A refused facts section is echoed in the `INVALID_INPUT` record (record 0.5) so replay refuses
   it the same way.
10. **CLI exit codes:** `ENTITY_ID_ALREADY_USED` and `LIFECYCLE_CONFLICT` exit 1 like `DENY`.
11. **Breaking host interface:** `Backend::create` and `Backend::commit` gained parameters, and
    `removed_at` / `incoming_at` are required (Python: `used_at` optional). Feature-005 host
    backends must be updated; stored documents are unchanged.

## Follow-ups

- **Verifiable facts:** membership, non-membership and incoming-reference proofs (an authenticated
  index) would let a reader check recorded facts without the store.
- **Entity-level concurrency:** `read_facts` and `write_lifecycle` make concurrent creations of one
  identity and removal/update races detectable without reinterpreting records.
- **Bulk import** as recorded transitions; identity reuse remains out of scope by design.
- Earlier follow-ups stand: resolve behavior by content hash, cryptographic evidence, a
  `behavior-evidence` crate, proven stores through `value_or` / `some` / declared types.

## Code review fixes

- **Seed constraints with `exists` / `referenced`:** a genesis now decodes the seed first, then
  checks entity constraints with facts answered from the seed itself (`SeedFacts`: the seed is
  the whole universe and history, with the seed's reference index). Before, such a constraint
  failed for every seed entity. Test: `references.rs::seed_constraints_may_use_existence_and_references`.
- **Bound references in supplied facts:** `check_snapshot` refuses facts that mark the target of a
  bound entity's `Ref` field absent or unused (`INCONSISTENT_FACTS`), matching the store's
  integrity and the verifier's assumption. Test:
  `facts.rs::a_bound_entitys_reference_target_cannot_be_absent`.
- Not changed: "`Read`-bound entities are not treated as existing" — action parameters are only
  `state`, `input` or `context`; `Read` parameters belong to derived values, invariants and
  constraints and are never bound to store entities.
