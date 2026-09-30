# Implementation Review: Relational Queries and Set Semantics

**Feature**: 007 · **Reviewed**: 2026-09-27 · **Tasks**: T001–T034

## Principles

- [x] **Sets are semantic; indexes are implementation.**
  - A query means the set of existing entities of its type that satisfy its filters. The store's
    type index (`keys_at`) and optional field index (`keys_by_field_at`) only name candidates:
    `StoreFacts::query` re-checks every candidate against the query, so an index can only give too
    many, never too few.
  - Conformance `query_index_consistency` compares both indexes with a scan at every position
    (the `StaleFieldIndex` mutant fails it).
  - `query_perf.rs::the_index_never_changes_a_result` shows identical records with and without
    the index.
- [x] **Queries are pure state reads.**
  - Queries are expressions only: a query as an effect is a `DECODE_ERROR` (`query_as_effect`).
    Nothing is written for resulting-state queries.
- [x] **Query meaning is independent of iteration order.**
  - Members are sorted and deduplicated before use, and `any`/`all` short-circuit in canonical
    identity order (FR-020a).
  - Conformance `query_order_independence` and the positive control `ReversedKeys` (Rust and
    Python) show this, as does `query_order_props.rs`: 1,000 permutations with and without the
    field index give identical records.
- [x] **Every query result that influences evaluation is part of the reproducible snapshot.**
  - Observed memberships go to `facts.queries` and member values to `facts.fields` (records 0.6).
    The bundle's `read_facts` adds a `result_hash` per query. Plain replay uses the recorded facts;
    behavior replay re-derives them at each parent.
- [x] **Exactness and type semantics continue through aggregation.**
  - `sum` is exact (the empty sum is the exact zero of the type) and range-checked like any
    fixed-scale value. `min`/`max` are optional, and `sum` of non-additive or `min` of unordered
    types is `TYPE_MISMATCH`.
- [x] **Unsupported proof is inconclusive, never guessed.**
  - Changed captures, extrema after removals and partial-sum range checks become fresh symbols or
    free flags.
  - Witness slots never reach a proof. `raise_limit` and `remove_cheapest` are inconclusive in
    `queries.expected.json`.
- [x] **Local invariants describe entities; global invariants describe relations.**
  - Queries in entity constraints and per-entity invariants are `QUERY_NOT_ALLOWED`. Module
    invariants are closed state expressions: an input is `UNKNOWN_PARAM`.
- [x] **Name behavior and domain meaning; hash expressions by semantics.**
  - Queries are inline. The lambda parameter name is kept for display, but the candidate is
    normalized to `$c` in hashes (`definition_identity_ignores_lambda_names_and_follows_predicates`).

## Constitution

| Principle | Status | Evidence |
|---|---|---|
| I. Deterministic core | ✓ | Canonical member order; exact aggregates; `BTreeMap`/`BTreeSet` throughout. The determinism check covers the `query_history` example, the 007 requests and `examples.orders.run` |
| III. Test-first | ✓ | Each story's tests came before its implementation and were first seen failing (T009/T010 → T011, T012/T013 → T014/T015, T016/T017 → T018, T019–T021 → T022–T024). The S' proptest was also mutation-checked: dropping created members makes it fail |
| IV. Reproducibility | ✓ | Plain replay from the record alone; behavior replay re-derives query facts and result hashes; `query_replay.rs` detects tampering (member, member value, result hash) at its position |
| V. Explicit state and auditability | ✓ | Every set a decision read, and every member value it used, is in the record |
| VI. Simplicity | ✓ | Queries are expressions, not a new item kind. One facts interface serves store, plain and seed evaluation. Module invariants are the one new item variant (justified in plan.md). No new crate or dependency |
| Tech constraints | ✓ | fmt, clippy `-D warnings`, no `unsafe`, no unwrap/expect outside tests |

## Requirements

| Requirement | Status | Evidence |
|---|---|---|
| FR-001 typed entity queries | ✓ | `select(T)` over existing entities only; removed entities are never members (store `keys_at`, S' derivation drops removed candidates) |
| FR-002 set semantics | ✓ | Members are a sorted, deduplicated identity set; there is no `first`/`nth` |
| FR-003 candidate-local predicates | ✓ | `NON_LOCAL_PREDICATE` (`exists_in_filter`), `QUERY_NOT_ALLOWED` (`nested_query`) |
| FR-004 pure and relational | ✓ | Allowed in conditions, derived values and module invariants; refused as effects and in entity rules |
| FR-005 quantifiers | ✓ | `any(∅) = false`, `all(∅) = true`, `count(∅) = 0` (`queries.rs`, `check_orders_empty`) |
| FR-006 aggregates | ✓ | Exact `sum` (additive numeric types only); optional `min`/`max`; `sum_of_string`, `min_of_unordered` |
| FR-007 set algebra | ✓ | Union, intersection and difference of the same type; `mixed_set_types` is `TYPE_MISMATCH` |
| FR-008 query identity | ✓ | `QueryDefinitionHash` (tag `behavior.query.v1`) and `QueryInstanceId = H(definition, canonical captures)` (`behavior.query_instance.v1`) |
| FR-009 / FR-009a query and member facts | ✓ | `QueryFact` (membership only) plus ordinary field facts; only observed facts are recorded |
| FR-010 replay without the store | ✓ | `replay()` on records alone; `query_replay.rs` |
| FR-011 plain evaluation | ✓ | `UNKNOWN_FACT` for a missing instance; `INCONSISTENT_FACTS` for contradictory universes or query facts |
| FR-012 one snapshot | ✓ | Conformance `query_snapshot` (commits land during evaluation; the `CurrentOnlyKeys` mutant fails it) |
| FR-013 / FR-013a resulting-state queries, captures | ✓ | `result_state_queries.rs`; `result_state_props.rs` (SC-004); captures in S for preconditions, S' after effects; a changed capture is a second instance observed against S |
| FR-014 observed query dependencies | ✓ | `read_facts.queries[*].result_hash` |
| FR-015 future conflict semantics | ✓ | Whole-state CAS unchanged. Query results with hashes are recorded dependencies |
| FR-016 indexes do not define semantics | ✓ | See principles; indexes are outside the state identity |
| FR-017 sound verification | ✓ | `encode/relational.rs`: summaries as functions of captures, exact deltas, operator-specific precision, module invariants assumed on S and checked on S' |
| FR-018 confirmed counterexamples | ✓ | The witness becomes `universe` facts and is confirmed by evaluation. The finding reports the observed `queries`/`fields` facts once they reproduce it on their own (`counterexamples_are_confirmed_with_their_query_facts`) |
| FR-019 lifecycle forms unchanged | ✓ | `ir_0_6.rs::modules_without_007_forms_keep_identity_and_version`; feature 006 tests unchanged |
| FR-020 / FR-020a order independence, canonical order | ✓ | See principles |
| FR-021 uniqueness | ✓ | `unique(q, by=…)`, delta rule on S' (`uniqueness_is_decided_by_the_delta_rule`); the verifier's delta rule shares summaries with `not any(…)` preconditions (`hire` proven) |
| FR-022 no hidden entity loading | ✓ | See below |
| FR-023 wire compatibility | ✓ | Every 007 form needs 0.6 (`each_007_form_needs_0_6`, `query_in_0_5`). Modules without them keep their version, bytes and identity |
| FR-024 module-level invariants | ✓ | Genesis (`GENESIS_INVALID`) and every S' the dependency analysis (`affects`) cannot rule out (`INVARIANT_VIOLATED`); unaffected actions skip them (`unrelated_transitions_skip_the_invariant_and_related_ones_check_it`) |
| FR-025 queries are expressions | ✓ | No query items; reuse goes through derived values (`open_order_count`) |
| SC-001 authoring surface | ✓ | `examples/orders/` (`select`, `where`, `count`, `any_`, `all_`, `sum_`, `min_`, `max_`, `unique`, module invariant) |
| SC-002 order and index independence | ✓ | `query_order_props.rs` (1,000 permutations, ignored, release); conformance `query_order_independence` |
| SC-003 replay of query histories | ✓ | `query_replay.rs` (200 transitions plus tampering by default; 1,000- and 10,000-transition ignored release variants) |
| SC-004 derived S' equals re-evaluation | ✓ | `result_state_props.rs` (64 cases; 10,000 in the ignored release variant) |
| SC-005 proof boundary | ✓ | `queries.expected.json`: 5 proven, runtime-confirmed counterexamples (the 2 seeded ones plus 6 real ones, below), 2 inconclusive. Nothing is proven outside the model |
| SC-006 existing identities and bytes unchanged | ✓ | `ir_0_6.rs` against `frozen_versions_006.json`; records without query facts keep their version; every golden replays |
| SC-007 selective query performance | ✓ | `query_perf.rs`: 3.2 ms at 1,000 and at 100,000 entities (release, field index) |
| SC-008 `any`/`all` order independence | ✓ | `query_order_props.rs`, `queries.rs` short-circuit order tests |

### FR-022: no hidden entity loading

- [x] Evaluation reaches entity sets only through `EvaluationFacts::query` (memberships of a query
  instance) and `EvaluationFacts::field` (member values). The only other methods are feature 006's
  `exists`, `used` and `incoming`. Both new methods have default implementations that refuse
  (`UNKNOWN_FACT`), so a facts provider that does not implement them cannot leak anything.
- [x] These are answered by the store from versions at the evaluated position (`StoreFacts`), by
  the seed at genesis (`SeedFacts`), or by supplied facts (`Facts`: `queries`, `fields`,
  `universe`). There is no other host callback in the engine API: expressions have no function
  values, no extension points, and no way to name a store or a query language.
- [x] The backend primitives `keys_at` / `keys_by_field_at` are only called by `StoreFacts::query`,
  and every candidate they return is re-checked.

## Deviations from the plan

1. **Universe facts in plain requests.** Besides `queries`/`fields`, a request may supply
   `universe` sections: the complete set of entities of a type. The engine answers queries,
   member values, existence and incoming references of that type from them (a module-aware
   `Supplied` wrapper) and records only what it observed. This makes request fixtures readable and
   is how verifier witnesses are confirmed.
2. **Separate consistency check for query facts.** `check_query_snapshot` validates supplied query
   and field facts (members satisfy the query, non-members do not, bound entities are classified
   on their values) next to feature 006's `check_snapshot`.
3. **Touched declarations include queried types.** A bundle's touched declarations list every
   entity type the action queries, so a declaration change invalidates bundles that depend on the
   set.
4. **`Order.customer` is a plain `Id<Customer>`** in the fixture and example (spec US1 closes
   customers whose orders are all closed; a `Ref` would make the removal `DANGLING_REFERENCE`).
5. **No `query.rs`.** Query semantics (captures, S' derivation, the `unique` delta rule, index
   plans) live in `eval.rs` next to the evaluator they extend; the store uses the exported
   `index_hint`, `query_matches` and `queried_types`.
6. **Index plans.** `index_hint` returns an `IndexPlan`, not a single equality:
   - an equality against a captured value is preferred over one against a literal, since an enum
     literal such as `status == open` usually matches most of the set;
   - union takes the union of both sides' plans; intersection takes either side's; difference
     takes the left side's.

   Found by the SC-007 test: the first version took the outermost literal equality and ran in
   1.46 s at 100,000 entities.
7. **`keys_at` order.** The contract says "in any order", not "sorted". Order is never semantic,
   and the conformance suite compares sets. The `ReversedKeys` control passes every case.
8. **Verifier design details.** Summaries are uninterpreted functions of the flattened capture
   terms, so fixed-instance deltas follow from congruence. There are three witness slots per
   queried type. The finding's facts are the observed facts of the confirmed evaluation.
9. **Module invariants in the builder.** They are traced in a `closed` scope (no parameters); a
   Python `@invariant` with no parameters is a module invariant.
10. **`affects` is public.** The verifier uses the engine's own dependency analysis, so it checks
    exactly the module invariants the runtime checks, in the same trace order.
11. **Behavior replay needed no code change (T022).** It already compares re-derived `read_facts`,
    which now include queries, fields and result hashes.
12. **Real findings in the verification fixture.** Besides the two seeded counterexamples,
    `renumber` (no uniqueness guard) and every creation with the unguarded `amount` input
    (`non_negative_amount`) are genuine, runtime-confirmed counterexamples. They are recorded as
    such.
13. **Test runtime.** `query_replay.rs` runs a 200-transition history by default. The planned
    1,000-transition run (T019) and the 10,000-transition run are ignored release tests (see
    Follow-ups: test-infrastructure debt).

## Follow-ups

- **Relational precision debt (verifier).** Some checks are inconclusive although they hold.
  None of them is ever reported as proven.
  - *Changed captures* (`raise_limit`): a monotonicity lemma over the predicate would prove them.
  - *Extrema after a removal or change* (`remove_cheapest`): the fresh `min`/`max` is expected
    precision loss.
  - *Partial-sum range checks of `sum`* (in the example, `place_order`'s postcondition `sum`).
    Keep this distinction explicit:
    - **A domain guarantee the model already states.** `non_negative_amount` is an entity
      constraint, so every *stored* order has `amount ≥ 0`. A created order is only checked by
      `constraint_post`, after the postconditions, but in the example the precondition
      `amount ≥ 0` guards it. The verifier should eventually exploit stated constraints for
      arbitrary query members: with non-negative members every partial sum is bounded by the
      total. This is verifier precision work.
    - **An expectation the model does not state.** If non-negativity were only something the
      application happens to expect, the fix belongs in the Behavior model (`requires`, a
      constraint, a nominal type), never in a verifier assumption.
- **Test-infrastructure debt.** The exhaustive property and replay suites (1,000- and
  10,000-transition histories, 10,000-case proptests, 1,000 order permutations, performance) are
  `#[ignore]`d and run with `cargo test --release --workspace -- --ignored`. `query_replay.rs`'s
  default history is 200 transitions (about 17 s in debug; it was about 95 s with 1,000). A
  separate CI job should run the ignored suite on every change to the core, store or verifier.
- **Grouped invariants** ("at most 3 open orders per customer") and **cross-entity filters** (an
  explicit semijoin) were deferred by the clarifications.
- **Large results** could be stored as content-addressed query-result objects. The recorded
  semantics stay unchanged.
- **Entity-level concurrency** can use the recorded query instances and result hashes as
  dependencies.
- **Verifiable facts** (non-)membership proofs, as for feature 006's existence facts.
- Earlier follow-ups still stand.

## Code review fixes

- **The `unique` delta rule only applies to a `unique` known to hold on S.** Before, while a
  module invariant was checked on S', every `unique` in it was decided by the delta rule. Under
  `not` or `or` that `unique` need not hold on S, so the runtime could allow a violating state
  (`count(...) < 3 or unique(...)` after a creation) or deny a valid one (`not unique(...)`). The
  verifier had the same flaw and falsely proved `hire` for the disjunctive invariant.
  - Now only the invariant's top-level conjuncts (`held_uniques`) get the delta rule, in both the
    evaluator and the verifier. Any other `unique` is evaluated in full, or encoded as a fresh
    summary.
  - Tests: `module_invariants.rs::a_unique_under_negation_is_evaluated_in_full`,
    `a_unique_under_disjunction_is_evaluated_in_full`, and
    `behavior-verify/tests/queries.rs::a_unique_under_disjunction_is_not_proven_by_the_delta_rule`
    (now a confirmed counterexample).
- Not changed: "queries reached through a derived value are missing from the invariant
  signature". `signature` does descend into derived bodies (`derived_signature`), and a closed
  module invariant cannot reference a derived value at all: admission requires at least one
  entity parameter for a derived value.
