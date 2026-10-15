# Implementation log: Approval (feature 503)

**Date**: 2026-10-15 | **Branch**: `503-approval-model`, rebased onto `main` `42777c1` (feature 504)
| **Core Release**: 0.13.0 (`15094972191aa0ae79705df0397c551a18745399`), unchanged by this feature.

Logs are in the session scratchpad (`…/scratchpad/logs/`), each with its SHA-256.

## Gates (T055)

`nix develop -c scripts/gates.sh` → `gates: OK` (log `503-gates-review2.log`, after the convergence tasks and review fixes, SHA-256
`642c0ed4bd4cb51a1dd1c2c23ae8bab8562ea9023a7e7a337ed8560cab259cfd`):

- core pin `OK (v0.13.0 1509497…)`, public surface, fetch core, `cargo fmt`, clippy `-D warnings`;
- `cargo test -p behavior-models`: 197 passed (feature 504 baseline 116);
- pytest `python/tests models`: **537 passed, 4 skipped** (504 baseline 480);
- mypy, models compat, determinism check, ownership, terms, workflows, script tests.

behavior-ir-core was not modified.

## Quickstart scenarios (T056)

| # | Command | Result |
|---|---|---|
| 1 | `pytest python/tests/test_approval*.py -k thresholds` | 7 passed |
| 2 | `-k "one_decision or approve_then_reject"` | 2 passed |
| 3 | `-k terminal` | 3 passed |
| 4 | `-k no_reject_threshold` | 1 passed |
| 5 | `-k stale_commit` | 2 passed (`STATE_CONFLICT`, retry DENIED by the guards) |
| 6 | `cargo test -p behavior-models --lib child_state` | 3 passed: postcondition `[0]` and `[1]` DENY on `S′` with the head unchanged; the control without postconditions commits both invalid states |
| 7 | `cargo test -p behavior-models --test approval_ownership` | 8 passed |
| 8 | `-k subject_untouched` | 1 passed |
| 9 | `-k "finalize or status_stays"` | 3 passed |
| 10 | `-k reference_document_review` | 1 passed (`ap_document_review` through the Store) |
| 11 | `cargo test -p behavior-models --test declaration_dag` | 9 passed |
| 12 | `pytest python/tests/test_model_staging.py -k pinned_cases` | 7 passed (every 501/502 case keeps its hashes) |
| 13 | `BEHAVIOR_FULL_PROPERTIES=1 pytest python/tests/test_approval_properties.py` | 1 passed (1,000 sequences, seed 503, ~50 s; 32 by default) |
| 14 | `BEHAVIOR_FULL_PROPERTIES=1 cargo test -p behavior-models --test approval_identity` | 7 passed (1,000 permutation cases, seed 503; 32 by default) |

The selectors of scenarios 1, 2, 6, 9 and 10 were corrected in quickstart.md to name the tests
that exist.

## Pinned reference hashes (T049)

| Case | BehaviorHash | ApprovalHash(es) |
|---|---|---|
| `ap_document_review` | `sha256:400dbdc3…3db9ef` | `document_review` `sha256:29c0a010…9dbb5e` |
| `ap_purchase_request` | `sha256:2a301cd8…636b12` | `purchase_request_approval` `sha256:5f75c136…9d9ecd` (flow `sha256:38574bde…3c9768`) |
| `ap_two_approvals` | `sha256:8f515e24…443eb3` | `finance_approval` `sha256:59257e04…9a6575`, `legal_review` `sha256:408a1884…f2dda8` (machine `ticket` `sha256:28cf011c…9219d9`) |

Full values, with the generated enum, entity and derived hashes, are in
`models/fixtures/compat.json`. The 501 and 502 tuples are byte-identical (the diff only adds
lines).

## Findings during implementation

- **Core 0.12.0 refused query-observing candidates under the command profile** (research R13).
  First misattributed to Core `main` as well (behavior-ir-core#24, corrected and closed); fixed
  by Core 023 and released in 0.13.0. Feature 504 moved the pin; 503 was rebased onto it, and
  the pinning test now requires a commit under both profiles
  (`decisions_observing_queries_commit_under_both_profiles`).
- **Core 0.13.0 contracts** adopted through 504 (complete parent snapshots for direct
  evaluation, record 0.8, verifier 0.9.0, idempotent store creation) need no Approval change:
  the Approval tests use the Store or supply complete snapshots. Seeded parents that violate
  the Approval invariant are still valid Core states, so the child-state test (T017) works
  unchanged.
- **No public per-entity hash in Core.** `ApprovalHash` references the subject and actor by
  `behavior_engine::schema::declarations(module)`, the declaration hash a store binds its
  schema to. A change to either entity (for example a RequestFlow payload) changes it (T044).
- RequestFlow's `needs` already followed from its contributions generically, so T041 needed no
  code change; T038 proves the edge.
- The receipt type in Python (`LoweringReceipt`) gained the optional `generated` field.

## Deviations, with reasons

- **Three approval compat tuples, not one.** An approval tuple's key includes `co_supplied`, the
  versions of the other models in the case (and those they embed), because an approval over a
  flow's entity depends on that lowering; a co-supplied lowerer bump must be a new tuple. The
  three reference cases have three co-supplied sets. quickstart.md and T055 were updated.
- **T045 builds each distinct permutation once** (48 permutations, 1,000 cases); a build is a
  pure function of its inputs, and repeated-build determinism is the determinism gate's job.
  Without the cache the property took 150 s.
- **Property depth by tier (review 2026-10-15).** The expensive property suites (T020, T021,
  T045) run 32 cases by default and the full 1,000 when `BEHAVIOR_FULL_PROPERTIES=1`, which the
  CI workflow and `scripts/release-check.sh` set. Locally the Rust properties went from about
  134 s to 5 s and the Python suite from 49 s to 2.4 s; golden and compat tests run on every
  change. The safety argument is the induction (valid genesis, local preservation, complete
  ownership); 1,000 random cases are its empirical check, not a proof. The exhaustive SC-002
  walk (T060) uses three actors by default and all four in the full tier. Optimizing Core's crates
  in the dev profile instead only saved 37 %, so the workspace profile is unchanged.
- **T030** was first written against `doc_approval` (the reference modules did not exist yet),
  then extended over the three `ap_*` modules in Python
  (`test_only_the_decision_actions_touch_decisions_in_every_reference_module`).
- **T051** flags an attempt word followed by `is/was/gets/got rejected` and arrows to `rejected`
  on denial lines, so outcome names such as `test_veto_first_reject_is_rejected` stay legal.

## Convergence (T058–T061)

`/speckit-converge` found four partial gaps; each is closed by a test:

- **T058 (SC-006)**: the permutation property in `approval_identity.rs` also permutes the
  supply order of a RequestFlow and a StateMachine with the two approvals (all 24 orders) and
  the order of three hand-written items over the decision entity. Its fingerprint includes the
  action names, so the property fails if a model or item is missing. Full tier: 1 passed,
  1,000 cases, 77 s; default tier about 4 s.
- **T059 (FR-010)**: `test_state_machine_transition_guarded_by_approval` and
  `test_request_flow_creation_guarded_by_approval` in `test_approval_composition.py`: DENY
  before the threshold, ALLOW after it.
- **T060 (SC-002)**: `test_every_sequence_matches_the_outcome_definition` in `test_approval.py`,
  per threshold configuration. A denied attempt leaves the state unchanged, so every sequence of
  up to 6 attempts by up to 4 actors is a walk through the reachable decision sets (at most
  3^4). Each is reached through the Store by committing its path, and its outcome and every
  attempt from it are checked against the definition. Four actors in the full tier (about
  50 s), three by default (about 18 s).
- **T061 (US4/AC7, FR-025)**: a hand-written action named `doc_approval__approve` or
  `doc_approval__reject` → `GENERATED_NAME_COLLISION`, nothing renamed; a StateMachine with the
  approval's id → `DUPLICATE_MODEL_ID` in both supply orders (model-generated capabilities
  cannot otherwise collide: one id namespace, injective `encode`).

## Review fix: self-dependent definitions

`definition_order` (`composition.rs`) accepted a definition whose `deps` named itself: it
returned an order instead of `DEFINITION_CYCLE`, because `topo_order` ignores self-edges (a
model's declarations may name each other). Direct recursion would then pass the cycle preflight
before any body is added. Self-dependencies are now refused explicitly, together with any longer
cycle (names sorted), and nothing is added
(`composition::tests::a_self_dependent_definition_is_a_cycle`). The contract
(`contracts/declaration-dag.md` §2) names the self-edge case.

## Review fixes: reference payloads and named parameter types

- **Reference payload fields.** An approval with `payload={"reviewer": Ref[User]}` failed with
  `DECLARATION_MISMATCH` against the declaration P2 itself generated: `existing`
  (`composition.rs`) compared Core's admitted types (`Ref(T)` is admitted as `Id(T)`, marked as
  a reference) with the contributed `Ref(T)`, and refused every reference-bearing entity. It now
  compares the admitted form (`Ref` → `Id`, inside `Option` too) and the set of reference
  fields. Tests: `reference_payload_fields_build_and_stay_references` (Rust, `Ref`,
  `Option<Ref>` and a plain `Id`), `references_are_compared_in_admitted_form_with_their_markers`
  (an `Id` against a `Ref` is still another entity), and
  `test_reference_payload_builds_and_records_the_reference` (Python).
- **Named parameter types in ApprovalHash.** Inputs, context and payload were hashed by type
  name only, so changing an unused context enum `Mode` from `{Standard, Fast}` to
  `{Standard, Slow}` kept ApprovalHash while the accepted invocations and BehaviorHash changed.
  Every named type (enum, nominal, entity) of a payload field, input or context parameter now
  carries its Core declaration hash (`types`, data model §8), read from the admitted module's
  public name table. Test: `approval_hash_follows_the_declarations_of_named_parameter_and_payload_types`.
  The four approval model hashes in `compat.json` were re-pinned; every BehaviorHash, and every
  501 and 502 tuple, is unchanged. The approval tuples exist only on this unreleased branch, so
  this is a correction, not a version change. StateMachine and RequestFlow share the
  name-only `params()` and keep it, because their hashes are pinned; the same gap there is
  behavior-ir#20.

## Anti-pattern audit (plan.md)

| Anti-pattern | Evidence |
|---|---|
| Approval moves the subject's status | `test_subject_untouched_by_every_decision`, `test_status_stays_in_review_while_outcome_approved`; the layout golden shows one `creates()` and no `effects()` |
| Stored, mutable outcome | `the_derived_bodies_have_the_data_model_structure`; ownership refuses every write of `X` (`a_hand_written_write_of_any_decision_field_is_refused`) |
| D(s) as "currently eligible decisions" | `test_eligibility_change_after_decision_still_counts`; derived bodies filter only `subject == s.id` |
| Implicit authorization | `test_unrestricted_approval_accepts_any_actor`, `test_eligible_false_is_denied` |
| An Approval query engine or runtime | `test_lowered_module_has_no_approval_constructs` (only ordinary Core items) |
| A correctness dependency on indexes | the properties read through Core's ordinary reads; no index is assumed |
| A model choosing its own declaration order or names | `declaration_dag.rs` (order, cycles, collisions, no rename) |
| Python deciding semantics | names from `_engine.decision_names`; `bind_schema` drift check; `test_predicate_handle_rejects_non_subject_argument` |
| Changing 501 or 502 outputs | `test_model_staging.py` pinned cases; compat gate (501/502 tuples byte-identical) |
| A workaround for Core | none; R13 was resolved by the Core release, not by forcing the legacy profile or stripping facts |
