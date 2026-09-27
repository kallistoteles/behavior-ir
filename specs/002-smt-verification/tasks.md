---

description: "Task list for SMT Verification of Behavior Modules"
---

# Tasks: SMT Verification of Behavior Modules

**Input**: Design documents from `/specs/002-smt-verification/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R13), data-model.md, contracts/
(verification-report.md, governance.md, engine-api.md), quickstart.md; feature 001 as built on
branch `001-verifiable-behavior-ir`; `PRINCIPLES.md`; `.specify/memory/constitution.md`.

**Tests**: Included and written first (constitution principle III). Each test task must be run
and seen to fail before the implementation tasks after it in the same phase.

**Organization**: Phase 2 extends the feature 001 runtime so the verifier's assumptions hold
(entity constraints, binding check, state-cell change sets) and builds the solver plumbing; the
user stories then add the checks and governance.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US4 from spec.md
- Paths are relative to the repository root

## Conventions for every task

- Rust: edition 2024, `#![forbid(unsafe_code)]` in `behavior-verify`, typed errors
  (`thiserror`), no `unwrap`/`expect` outside tests, ordered collections only.
- All JSON output goes through `behavior_core::canonical`; decimals are normalized strings.
- The verifier asserts exactly the runtime's pre-evaluation guarantees (research R4) and nothing
  else; every `sat` answer is confirmed by evaluation before it becomes a finding (R7).
- Solver calls use the pinned Z3 with `(set-option :rlimit N)`, `(set-option :smt.random_seed 0)`,
  one thread, and a wall-clock guard (R2).
- Feature 001's behavior versions and `tests/fixtures/hash_vectors.json` must stay unchanged.

---

## Phase 1: Setup

- [X] T001 Add `z3` (nixpkgs, 4.16.0) to the dev shell packages in `flake.nix`, export `BEHAVIOR_Z3` pointing at it in the shell hook, and verify `nix develop -c z3 --version` prints `Z3 version 4.16.0`
- [X] T002 Create `crates/behavior-verify/Cargo.toml` (deps: `behavior-core`, `serde` with `derive`, `serde_json`, `sha2`, `thiserror`, `ed25519-dalek` with `std`; dev-deps: `pretty_assertions`), add it to the workspace `Cargo.toml`, and create `crates/behavior-verify/src/lib.rs` with `#![forbid(unsafe_code)]` and empty modules `smt`, `solver`, `encode`, `checks`, `confirm`, `cache`, `hashing`, `governance`
- [X] T003 [P] Create `tests/fixtures/verify/` and `tests/fixtures/governance/` and document both in `tests/fixtures/README.md` (verify: modules with seeded defects and `<name>.expected.json`; governance: policies, waivers, signed attestations, fixed test keys)
- [X] T004 [P] Add `examples/tryout/dump.py` (prints `examples.tryout.demo.model.to_wire_json()` without a trailing newline; move the model into `examples/tryout/model.py` and keep `demo.py` importing it) and commit `examples/tryout/`

**Checkpoint**: workspace builds with the empty crate; `z3` is on the dev-shell path

---

## Phase 2: Foundational (runtime contract + solver plumbing)

**Purpose**: make the runtime guarantee what the verifier will assume (research R4, R11) and
provide SMT-LIB generation, parsing, the solver process, the encoding, and the hashes.

**⚠️ CRITICAL**: no user story work before this phase is complete

### Tests (write first, must fail)

- [X] T005 [P] Create wire IR 0.2 fixtures in `tests/fixtures/wire/valid/`: `constraints.json` (entities `Employee{role: string, approval_limit: Money}` with constraint `non_negative_limit`: `e.approval_limit >= Money(0)`, and `Account{balance: Money}` with constraint `non_negative_balance`; action `transfer(from: state Account, to: state Account, amount: input Money)` with precondition `amount > Money(0)` and effects on both balances) and in `tests/fixtures/wire/invalid/`: `constraint_unknown_entity` (`UNKNOWN_ENTITY`), `constraint_not_bool` (`NOT_BOOLEAN`), `constraints_in_0_1` (a `"0.1"` document with a `constraints` key: `DECODE_ERROR`), `constraint_duplicate_name` (`DUPLICATE_NAME` with an action of the same name), each with `.expected.json`
- [X] T006 [P] Write `crates/behavior-core/tests/constraints.rs`: `constraints.json` admits with item `constraint:non_negative_limit`; renaming a constraint keeps its item hash; every file in `tests/fixtures/wire/valid/` except `constraints.json` and every entry of `tests/fixtures/hash_vectors.json` keeps its behavior version (compare against the frozen values); serializing `constraints.json` round-trips to the same version and writes `"ir_version": "0.2"`, while modules without constraints serialize with `"0.1"`
- [X] T007 [P] Create request fixtures in `tests/fixtures/requests/002/` against `constraints.json`, plus `tests/fixtures/requests/002/expectations.json` (same format as feature 001's): `context_invalid` (a `review` action with `actor: context Employee` whose limit is `"-1"` → `INVALID_CONTEXT`, trace has one `constraint` step with `role: "context"`), `input_invalid` (an `Employee` passed as input with limit `"-1"` → `INVALID_INPUT`), `state_invalid` (an account with balance `"-5"` → `INVALID_STATE`), `constraint_post_violation` (transfer of more than the balance → `DENY` with a `constraint_post` step), `alias` (`from` and `to` with the same `id` → `INVALID_BINDING`, reason `STATE_ALIAS_NOT_ALLOWED` naming both parameters), `transfer_ok` (→ `ALLOW`, changes carry `entity: "Account"` and the ids)
- [X] T008 [P] Write `crates/behavior-core/tests/evaluate_002.rs` asserting `tests/fixtures/requests/002/expectations.json` (result, trace phases, reasons, change entries with `param`, `entity`, `id`, `field`, `old`, `new`) and that every record has `record_version: "0.2"`
- [X] T009 [P] Write builder and Python tests for constraints: in `crates/behavior-core/tests/builder.rs` a builder-built `constraints.json` model equals the wire fixture's version; in `python/tests/test_constraints.py` an `@constraint` over `Employee` rejects a context employee with a negative limit as `INVALID_CONTEXT` and `@constraint` with two parameters raises `BehaviorDefinitionError`
- [X] T010 [P] Write `crates/behavior-verify/tests/smt.rs`: the s-expression reader parses `sat`, `unsat`, `unknown`, `(error "…")`, and `get-value` results with integers, negative integers `(- 5)`, reals `3.0`, `(/ 1.0 100.0)`, `(- (/ 3.0 2.0))`, strings with `""` escapes, and booleans; converting reals to decimals yields normalized strings and reports non-representable values (e.g. `1/3`) as such
- [X] T011 [P] Write `crates/behavior-verify/tests/solver.rs`: `Z3Process` reports its version as `z3 4.16.0`; the purchase-budget query from research (see `specs/002-smt-verification/research.md` probe) returns `sat` with the same model bytes on two runs; a query exceeding a tiny `rlimit` returns `unknown` with reason `resource_limit`
- [X] T012 [P] Write `crates/behavior-verify/tests/encode.rs`: for small hand-built modules, encoded queries are satisfiable or unsatisfiable as expected: Int bounds (`x + 1 > i64::MAX` reachable), enum domain (a 2-value enum cannot take a third value), option structure (`is_none` and a value), distinct state ids for two state parameters of the same entity, derived value instantiation for S and S', a division whose rounding bound matters (`a / b` compared with a boundary it misses by less than the bound is neither proven nor refuted)
- [X] T013 [P] Write `crates/behavior-verify/tests/hashing.rs`: finding hashes depend only on kind and cited hashes (same finding with different counterexample values → same hash); check keys change when any involved hash, the profile, the verifier version, or the solver version changes; governance hashes use their tags and ignore the `hash` field

### Implementation

- [X] T014 Implement wire IR 0.2 in `crates/behavior-core/src/wire.rs`: accept `"0.1"` (no `constraints` key allowed) and `"0.2"` (required `constraints` list of `{name, entity, param, body, loc}`); unknown keys stay `DECODE_ERROR`
- [X] T015 Implement entity constraints in `crates/behavior-core/src/admit/` (`resolve.rs`: names unique across derived values, invariants, constraints, and actions; `typecheck.rs`: body Bool over the one parameter; `hash.rs`: tag `behavior.constraint.v1`, body like an invariant, module entry kind 7), `crates/behavior-core/src/semantic/module.rs` (`ConstraintItem`, accessors), and `crates/behavior-core/src/serialize.rs` (write `"0.2"` and `constraints` only when the module has constraints, otherwise `"0.1"`); make T005 and T006 pass
- [X] T016 Add `add_constraint(name, entity, param, body, loc)` to `crates/behavior-core/src/builder.rs` and the PyO3 `Builder` in `crates/behavior-py/src/lib.rs`
- [X] T017 Extend evaluation in `crates/behavior-core/src/eval.rs` to research R11's order: binding check (two state parameters with the same entity type and `id` → `INVALID_BINDING`, reason `STATE_ALIAS_NOT_ALLOWED`), entity constraints on every incoming entity by role (`INVALID_STATE` / `INVALID_INPUT` / `INVALID_CONTEXT`, trace phase `constraint` with `param` and `role`), state invariants on S, preconditions, effects, then postconditions, state invariants, and entity constraints (phase `constraint_post`) on S' state entities; change entries gain `entity` and `id`; `crates/behavior-core/src/record.rs` writes `record_version: "0.2"`; make T007 and T008 pass
- [X] T018 Regenerate `tests/fixtures/records/*.json` (feature 001) with `BLESS_RECORDS=1` after deleting them, review that the only differences are `record_version` and `entity`/`id` in `changes`, and update the CLI `intent` records accordingly
- [X] T019 Add `@constraint` (exactly one entity parameter, returns Bool) and `BehaviorModule(constraints=[...])` to `python/behavior/decl.py`, `python/behavior/module.py`, `python/behavior/__init__.py`, and `python/behavior/_engine.pyi`; make T009 pass
- [X] T020 Implement `crates/behavior-verify/src/smt.rs`: an SMT-LIB writer (declarations, assertions, `check-sat`, `get-value`) and an s-expression reader with value conversion to engine values; make T010 pass
- [X] T021 Implement `crates/behavior-verify/src/solver.rs`: `trait Solver { fn check(&self, query: &str) -> SolverAnswer; fn version(&self) -> String; }`, `SolverAnswer { Sat(model), Unsat, Unknown(reason) }`, and `Z3Process` (binary from `BEHAVIOR_Z3` or `PATH`, `-in` mode, options from research R2, wall-clock guard with reason `wall_clock_guard`); make T011 pass
- [X] T022 Implement `crates/behavior-verify/src/encode.rs` per research R3–R5: variables per state/input/context field, type domains (i64 range, decimal range, enum `[0, n)`, strings, options), assumptions (entity constraints on all entities, state invariants on state, distinct state ids), derived values instantiated per binding and per S/S', effects as S' terms, one bounded variable per decimal multiplication and division (`|r − exact| ≤ max(10^-28, |exact|·10^-27)`, division as `exact·divisor = dividend` under `divisor ≠ 0`), and path guards for short-circuit `and`/`or` and sequential preconditions; add the accessors it needs to `behavior-core` semantic types; make T012 pass
- [X] T023 Implement `crates/behavior-verify/src/hashing.rs`: finding hash (`behavior.finding.v1`), check key (`behavior.check.v1`), and governance hashes (`behavior.verification.v1`, `behavior.waiver.v1`, `behavior.policy.v1`, `behavior.authorization.v1`, `behavior.transition.v1`) per research R8–R9; make T013 pass

**Checkpoint**: runtime enforces constraints and distinct bindings; feature 001 tests pass with
regenerated records; the verifier can encode modules and talk to Z3

---

## Phase 3: User Story 1 - Prove that every action preserves the invariants (Priority: P1) 🎯 MVP

**Goal**: for each action and each state invariant or entity constraint on its state, prove
preservation or report a confirmed counterexample (FR-001, FR-006, FR-008, FR-011, FR-012).

**Independent Test**: quickstart §2: the demo's `approve` gets a confirmed `within_budget`
counterexample; with the budget precondition it is proven.

### Tests (write first, must fail)

- [X] T024 [P] [US1] Create `tests/fixtures/verify/purchase.json` (the demo model without a budget precondition) and `tests/fixtures/verify/purchase_fixed.json` (with `purchase.amount <= remaining(project)`), `tests/fixtures/verify/unchanged_entity.json` (an action that does not modify the entity its invariant constrains) and `tests/fixtures/verify/constraint_break.json` (`transfer` without `amount <= from.balance`), each with `.expected.json` listing every expected check `{kind, action, subject, outcome}`
- [X] T025 [P] [US1] Write `crates/behavior-verify/tests/preservation.rs`: expected outcomes for each fixture; every counterexample's embedded record has result `DENY` with the named invariant or constraint failing in a `invariant_post` or `constraint_post` step; no counterexample violates an entity constraint on input or context or reuses a state id; the attestation for the same fixture is byte-identical across two runs
- [X] T026 [P] [US1] Write `crates/behavior-cli/tests/cli_verify.rs` (exit 0 for `purchase_fixed.json`, 1 for `purchase.json`, 2 for an invalid wire file, `--out` writes the same bytes as stdout) and `python/tests/test_verify.py` (`verify(model).result`, findings with counterexamples, `attestation.json` is canonical)

### Implementation

- [X] T027 [US1] Implement preservation checks in `crates/behavior-verify/src/checks.rs` (query `A ∧ p1…pn ∧ ¬r(S')` per rule `r`, research R6) and counterexample search in two passes (nice decimals first: at most 4 fractional digits, magnitude ≤ 10^15; then unrestricted), research R5
- [X] T028 [US1] Implement `crates/behavior-verify/src/confirm.rs`: build an EvaluationRequest from a model (ids `e0`, `e1`, … in parameter order, values in request encoding), evaluate it with `behavior_core::evaluate`, and accept the finding only if the record shows the predicted failure; otherwise mark the check inconclusive (`counterexample_not_reproduced`)
- [X] T029 [US1] Implement `verify(module, profile, cache, solver) -> Attestation` in `crates/behavior-verify/src/lib.rs` producing contracts/verification-report.md (checks sorted, findings sorted by hash, `result`, attestation hash excluding `cached` flags); make T025 pass
- [X] T030 [US1] Add `behavior verify <wire> [--profile] [--cache] [--out]` to `crates/behavior-cli/src/main.rs` (exit 0 verified, 1 not verified, 2 admission failure, 64 usage) and `verify(model, profile=None, cache=None)` with an `Attestation` result to `crates/behavior-py/src/lib.rs`, `python/behavior/__init__.py`, `python/behavior/results.py`, `python/behavior/_engine.pyi`; make T026 pass
- [X] T031 [US1] Run quickstart.md §2 and fix any failures

**Checkpoint**: invariant and constraint preservation is proven or refuted with reproducible
counterexamples (MVP)

---

## Phase 4: User Story 2 - Verification attestations, waivers, and the execution policy (Priority: P2)

**Goal**: cached, deterministic attestations; signed waivers; a declarative policy producing
commit authorizations (FR-010, FR-014–FR-015, FR-017–FR-024).

**Independent Test**: quickstart §7 and §8.

### Tests (write first, must fail)

- [X] T032 [P] [US2] Write `crates/behavior-verify/tests/cache.rs`: a second run with the same cache marks every check `cached` and yields the same attestation hash; after changing one action in `purchase.json` only that action's checks are recomputed; a result stopped by the wall-clock guard is not written to the cache and is marked `reproducible: false`; a profile with a tiny `rlimit` yields `inconclusive` (`resource_limit`) findings with severity `blocking` and result `not_verified`
- [X] T033 [P] [US2] Create governance fixtures in `tests/fixtures/governance/`: `keys.json` (two fixed Ed25519 test key pairs as hex seeds, marked test-only), policies `require_verified.json`, `verified_or_waived.json` (trusts key A with role `risk_reviewer`, `waivable: ["inconclusive"]`, `forbidden: ["evaluation_error"]`), and a generator note in `tests/fixtures/governance/README.md` explaining that waivers and signatures are produced by the test helpers from `keys.json`
- [X] T034 [P] [US2] Write `crates/behavior-verify/tests/governance.rs` covering spec US2 scenarios 5–12: unverified → refuse `not_verified`; inconclusive-only + valid waiver signed by key A → allow with `waivers_used` recording waiver hash, key id, signature, principal, while the attestation stays `not_verified`; waiver after a semantic change (new behavior hash) or with a different finding hash → ignored; waiver for a forbidden kind → refuse `finding_not_waivable`; expired at `now` → refuse `waiver_expired`; signed by untrusted key B or with a corrupted signature → refuse `untrusted_key` / `invalid_signature`; one waiver with signatures from A and B → the waiver hash is unchanged and the authorization records the smallest satisfying key id; an authorization made under policy P1 keeps citing P1's hash after P2 is introduced; the same inputs give byte-identical authorizations
- [X] T035 [P] [US2] Write `crates/behavior-cli/tests/cli_authorize.rs` (`behavior authorize` exit 0 allow / 1 refuse / 2 invalid input, `behavior waiver-hash` output) and `python/tests/test_authorize.py` (`authorize(...)` with native dicts, decision and reasons)

### Implementation

- [X] T036 [US2] Implement `crates/behavior-verify/src/cache.rs` (directory of canonical JSON files named by check key, default `.behavior/verify-cache/`, no writes for non-reproducible results) and wire it into `verify`; make T032 pass
- [X] T037 [US2] Implement `crates/behavior-verify/src/governance.rs`: waiver decoding and hash, signed attestation verification (Ed25519 over `"behavior.waiver.v1" ‖ 0x00 ‖ waiver hash bytes`), policy decoding and hash, and `authorize(policy, module, record, attestation?, waivers, signed, now) -> Authorization` per contracts/governance.md; make T034 pass
- [X] T038 [US2] Add `behavior authorize` and `behavior waiver-hash` to `crates/behavior-cli/src/main.rs`, and `authorize(...)` to the PyO3 binding and `python/behavior/` (`Authorization` result with `decision`, `reasons`, `waivers_used`, `json`); make T035 pass
- [X] T039 [US2] Run quickstart.md §7–§8 and fix any failures

**Checkpoint**: attestations are cached and deterministic; waivers and policy decide commits
without changing verification results

---

## Phase 5: User Story 3 - No allowed transition can fail its postconditions or hit an evaluation error (Priority: P3)

**Goal**: postcondition and evaluation-error checks (FR-002, FR-005, FR-013, FR-013a).

**Independent Test**: quickstart §3 and §5.

### Tests (write first, must fail)

- [X] T040 [P] [US3] Create `tests/fixtures/verify/postcondition.json` (an action whose `ensures` its effects do not guarantee), `tests/fixtures/verify/overflow.json` (an Int multiplication that can exceed the 64-bit range under its preconditions), `tests/fixtures/verify/rounding.json` (a rule `a / b < c` that misses `c` by less than the rounding bound, and a variant with a clear margin), with `.expected.json`; reuse `tests/fixtures/wire/valid/project_margin.json` with `tests/fixtures/verify/project_margin.expected.json` (`flag_project_unguarded`: `evaluation_error` at the division, counterexample `revenue = 0`; `flag_project`: no evaluation-error finding)
- [X] T041 [P] [US3] Write `crates/behavior-verify/tests/errors_and_posts.rs`: expected outcomes; postcondition counterexamples evaluate to `DENY` at the named postcondition; evaluation-error counterexamples evaluate to `ERROR` naming the expression; the rounding fixture is `inconclusive` for the tight variant and `proven` for the clear margin

### Implementation

- [X] T042 [US3] Implement postcondition checks (`A ∧ p1…pn ∧ ¬qj(S')`) and evaluation-error checks (division by zero and Int/Decimal overflow per expression under its path guard, in preconditions, effects, and postconditions) in `crates/behavior-verify/src/checks.rs`, with confirmation expecting `DENY` or `ERROR`; make T041 pass
- [X] T043 [US3] Run quickstart.md §3 and §5 and fix any failures

---

## Phase 6: User Story 4 - Find actions that can never run and conditions that never matter (Priority: P4)

**Goal**: dead actions, redundant preconditions, vacuous rules as warnings (FR-003, FR-004).

**Independent Test**: quickstart §4.

- [X] T044 [P] [US4] Create `tests/fixtures/verify/dead_and_vacuous.json` (an action with preconditions `amount > 100` and `amount < 50`; a precondition implied by an entity constraint; a rule `x.amount >= Money(0)` under the constraint `amount >= 0`) with `.expected.json`, and write `crates/behavior-verify/tests/warnings.rs` asserting `dead_action`, `redundant_precondition`, and `always_true` warnings with explanations and result `verified` when nothing blocks
- [X] T045 [US4] Implement dead-action, redundant-precondition, and always-true/always-false checks in `crates/behavior-verify/src/checks.rs` (queries in research R6; for rules, assume only type domains and entity constraints of the rule's parameters); make T044 pass
- [X] T046 [US4] Run quickstart.md §4 and fix any failures

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T047 [P] Extend `scripts/determinism-check.sh` to run `behavior verify` twice on every file in `tests/fixtures/verify/` and `tests/fixtures/wire/valid/` and compare bytes, and to run `behavior authorize` twice on a governance case
- [X] T048 [P] Write `crates/behavior-verify/tests/perf.rs` (`#[ignore]`): verifying the invoice, project margin, and purchase modules takes < 10 s in total (SC-003); in a generated 50-action module, re-verification after changing one action with a warm cache takes < 20% of the cold run (SC-004); list the command in quickstart.md §1 and `README.md`
- [X] T049 [P] Update `README.md` (verification, attestations, waivers, policy; `behavior verify` example on the demo) and `specs/002-smt-verification/quickstart.md` if commands changed
- [X] T050 Run fmt, clippy (`-D warnings`), `cargo test --workspace`, `pytest python/tests`, `mypy`, `scripts/determinism-check.sh`, and the ignored perf tests; fix all findings
- [X] T051 Review the implementation against `PRINCIPLES.md` (§2 identity and aliasing, §10 assumptions and evidence, §11 constraints vs invariants) and the constitution; record the result in `specs/002-smt-verification/checklists/implementation-review.md`, including every deviation from this task list
- [X] T052 Run quickstart.md §1–§8 from a fresh `nix develop` shell and fix any failures

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)** → **Foundational (Phase 2)** → user stories
- **US1 (Phase 3)**: after Foundational
- **US2 (Phase 4)**: after US1 (attestations and cache wrap `verify`; governance needs findings)
- **US3 (Phase 5)** and **US4 (Phase 6)**: after US1 (they add check kinds to `checks.rs`); independent of US2 and of each other, but both edit `checks.rs`, so run them one after the other
- **Polish (Phase 7)**: after the stories you ship

### Within Each Phase

- Tests first, seen failing
- Rust core changes (T014–T018) before the Python DSL (T019)
- `smt.rs` → `solver.rs` → `encode.rs` → `checks.rs` → `confirm.rs` → `lib.rs`
- Golden regeneration (T018) requires a human review of the diff before commit

### Parallel Opportunities

- Setup: T003, T004 in parallel after T002
- Foundational tests T005–T013 in parallel; T020, T021, T023 in parallel after their tests
- US1 tests T024–T026 in parallel; US2 tests T032–T035 in parallel
- Polish T047–T049 in parallel

---

## Parallel Example: Phase 2 tests

```bash
Task: "Create wire IR 0.2 fixtures (T005)"
Task: "Write crates/behavior-core/tests/constraints.rs (T006)"
Task: "Create request fixtures in tests/fixtures/requests/002/ (T007)"
Task: "Write crates/behavior-verify/tests/smt.rs (T010)"
Task: "Write crates/behavior-verify/tests/solver.rs (T011)"
Task: "Write crates/behavior-verify/tests/encode.rs (T012)"
Task: "Write crates/behavior-verify/tests/hashing.rs (T013)"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup + Foundational (runtime contract, SMT plumbing)
2. US1: preservation proofs with confirmed counterexamples, `behavior verify`
3. **Stop and validate**: quickstart §2 on the demo

### Incremental Delivery

1. + US2 → attestations, cache, waivers, policy, commit authorizations
2. + US3 → postconditions, division by zero, overflow, decimal intervals
3. + US4 → dead actions and vacuous conditions
4. Polish → determinism, performance, review

---

## Notes

- Goldens (regenerated 001 records, expected verification outcomes) are reviewed by a human
  before commit; frozen hash vectors must not change
- Test keys in `tests/fixtures/governance/keys.json` are for tests only and must never be trusted
  by a real policy
- Commit after each task or logical group; never commit a failing gate
