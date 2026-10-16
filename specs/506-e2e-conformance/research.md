# Research: End-to-End Behaviour Conformance (feature 506)

Every decision below was checked against the code at `main` `d817840` and the bundled Core
0.13.0 CLI.

## R1. The reference workflow and its parameters

**Decision**: A purchase approval workflow built from three model kinds:
- the RequestFlow `purchase_request` (payload `title`, `amount`; creation `create` → `Draft`;
  `submit`, `start_review`, `finalize_approved`, `finalize_rejected`);
- its embedded StateMachine;
- a standalone StateMachine `budget` over an authored `Budget` (`Open` → `Closed`);
- the Approval `purchase_request_approval` over the flow's request entity, with `k_A = 2` and
  `k_R = 1`. Eligibility is "the actor's role is `Approver`", read through a query over `User`.
  Open is "the request is `InReview`".

`finalize_approved` requires `approved(request)`, `finalize_rejected` requires
`rejected(request)`, and `finalize_approved` emits the command intent `RequestApproved`.

The users are `alice` (Requester), and `bob`, `carol` and `dave` (Approver). A hand-written
action `set_role` changes a user's role, so an observed fact can go stale (C11).

**Rationale**: This exercises every model kind through its published API, a cross-model guard
(a flow transition guarded by an approval predicate), a query observation that can go stale,
and a command intent. It is small enough to state every expected outcome by hand. `k_R = 1`
makes the rejection path a single decision, and `k_A = 2` makes the approval path need two.

**Alternatives considered**:
- Reuse the 503 reference case `ap_purchase_request`. Rejected: it has no query-based
  eligibility and no standalone machine. The new app is not a compat reference case, so the
  pinned manifest is unaffected.
- `k_R = ⊥`. Rejected: the rejection path would be impossible.

## R2. Lifecycle step → public operation

| Step | Operation |
|---|---|
| Author, resolve, lower | `BehaviorModule(entities, enums, models, actions, reads)` (the P1–P4 build protocol of 501–503) |
| Admit | the module build; `model.admission` and `to_wire_json()`; CLI `admit` on the wire for C16b |
| Initial state | `Store.genesis_v2_for(model, seed, policy)` and `Store.create(InMemoryBackend(), model, genesis)` |
| Evaluate (Store-backed) | `Store.evaluate(model, capability, bindings, input, context, commit_time)` |
| Evaluate (direct) | `evaluate(model, action, state, input, context, data_version, facts)` and `invoke(model, document, snapshot)` |
| Commit | `Store.commit(model, bundle)`; a conflict raises `StateConflict` |
| Read state and history | `Store.read`, `Store.load`, `Store.current`, `Store.history()`, `Store.history_at`, `Store.commands_since` |
| Data replay | `replay_data(store)` |
| Behavior replay | `replay_behavior(store, [model])`, and `replay_invocation(model, record)` for portable records |
| Static verification | `verify(model, Profile())` → `Attestation` |
| Backend conformance | `run_conformance(InMemoryBackend)` |

**Rationale**: Every step has a public operation in the pinned release, so FR-001 needs no gap
report.

## R3. A checked valid initial state (VO-001)

**Decision**: Rely on Core: store creation validates the genesis state and refuses an invalid
one with `GENESIS_INVALID`. Checked on 0.13.0 with a violated invariant. The suite also adds:
- a negative control with an invalid seed;
- an independent check of the Approval laws on `S₀`. The seed has no decisions, so the laws
  hold trivially, and the check confirms that.

**Rationale**: VO-001 asks for a check, not an assumption. Core performs it at creation, and the
negative control proves that it is active. The Approval laws are child-state postconditions,
not invariants, so Core does not check them on a seed (503 research). They are checked here as
oracles over the read state (R6).

## R4. Authoring conformance (C16a)

**Decision**:
- **Rust half**: `crates/behavior-models/tests/e2e_reference.rs` constructs the same workflow
  with the Rust model authority, using Rust fragment sources of the kind the 503 tests use. It
  works from the same explicit definition, written out independently, never by reading
  Python's output.
- **Python half**: the reference app's module.
- **Comparison**: each side writes its **authoring identity** to
  `target/conformance/c16a-{rust,python}.json`, and the Python test compares the two. The
  authoring identity (contract `comparison.md` §1) is:
  - BehaviorHash;
  - the CLI-equivalent name → item-hash table;
  - each model's model hash and versions;
  - Approval's generated hashes.

  Source locations are excluded: they are outside semantics and checkout-dependent until
  core#31.
- **Missing Rust half**: if the Rust file is absent (pytest run alone), C16a is UNSUPPORTED
  ("Rust half not run"), never passed.

**Rationale**: Exact identity comparison is possible because every item hash and the
BehaviorHash are Core's canonical hashes. Comparing wire JSON would compare locations.

**Alternatives considered**: Deserializing Python's wire JSON into Rust. Rejected by the spec:
it is not an independent construction.

## R5. Execution conformance (C16b)

**Decision**: For each compared operation, the test builds explicit documents and gives the
same bytes to both paths:
- the module wire (`to_wire_json()`);
- the request (`{data_version, state, input, context, facts}`), or an invocation document with
  its snapshot.

The Python paths are `evaluate` (giving `Decision.record_json`) and `invoke` (giving
`InvocationRecord`). The CLI paths are `behavior eval WIRE REQUEST` and
`behavior invoke WIRE INVOCATION SNAPSHOT` in a subprocess, given files under the test's
temporary directory.

**Comparison**: exact equality of the canonical record JSON, which carries `record_id`. In the
current profile, Core detaches source and display diagnostics from records, so nothing
checkout-dependent remains. The compared set covers the allowed and refused operations of
C04, C05, C07, C08 and an invalid input.

**Rationale**: Records are Core's canonical, content-addressed artifact, so byte equality is the
strongest comparison and needs no normalization beyond canonical JSON (contract
`comparison.md` §2). Both paths use the same engine. The spec states that they are independent
consumers, not independent implementations.

## R6. Oracles and invariant checks

**Decision**: `models/examples/purchase_approval/expected.json` states, per scenario and by
hand:
- the expected result (ALLOW, DENY, or the refusal code);
- the expected derived outcome;
- the expected status and command intents;
- the expected history length.

The invariants VO-003 and VO-004 are checked by small predicates over decision rows read
through a declared read. They count decisions per `(subject, actor)` and per choice and compare
with `k_A` and `k_R`.

**Rationale**: FR-023 requires independently stated expectations. The predicates are the
definitions in the spec's verification obligations, applied to observed data, not a
re-implementation of Core's evaluation: they decide nothing that Core decides. VO-005,
ownership, is Core's and the model authority's. The suite checks it by building a module with a
hand-written decision write and expecting `UNAUTHORIZED_DECISION_WRITE`, a negative control.

## R7. Store contract scenarios

**Decisions**:
- **C10**: after `bob`'s approval, approvals by `carol` and `dave` are evaluated on the same
  head. `carol`'s commits and reaches `k_A`. `dave`'s raises `StateConflict` (`STATE_CONFLICT`),
  so the history grows by exactly one. Re-evaluated on the new head, `dave`'s approval is DENY
  by the terminal guard, because the outcome is already APPROVED. No invalid double commit can
  occur.
- **C11**: `carol`'s approval is evaluated while she is an Approver. `set_role(carol,
  Requester)` commits. Committing the stale approval raises `StateConflict`, and re-evaluating
  it is DENY (not eligible): the stale observation authorized nothing. C10 is plain contention
  on one head. C11 changes the observed fact, and the re-evaluation shows that it matters.
- **Atomic publication (FR-013)**: after every failed commit, `current()`, `history()` and
  `commands_since(...)` equal their values from before the attempt.
- **C17**: `Store.create` twice on one backend with the same genesis yields the same `store_id`
  and history. With a different genesis it is refused (the Core 019 creation contract).
- **FR-018**: `run_conformance(InMemoryBackend)` passes.

## R8. Replay, integrity and commit history

**Decisions**:
- **C13a, data replay**: `replay_data(store)` reports success, and its recomputed head identity
  equals `current()`.
- **C13b, behavior replay**: `replay_behavior(store, [model])` reports success for every
  transition. The reference history's direct invocations are also recorded as
  `InvocationRecord`s and replayed with `replay_invocation(model, record)`.
- **Historical commit validity**: a separate check over `history()`. Each entry's parent is
  the previous head (`Parent(h′) = h`), positions are contiguous, and each entry's changes
  equal the effects of its recorded decision. It is reported separately and never inferred
  from C13b.
- **C14, negative controls**:
  - a recorded invocation's facts, input or result altered → `replay_invocation` reports a
    mismatch;
  - a record replayed against a different module (one changed declaration) → mismatch or
    refusal;
  - a record with a required context field removed → refusal;
  - a stored transition altered in a wrapping test backend → `replay_data` fails.

**Rationale**: These are the pinned release's public replay contracts. Tampering inside a test
backend is a negative control, not a Store protocol. Missing historical inputs are reported
UNVERIFIABLE by the report rules (R11).

## R9. Static verification (C19)

**Decision**: `verify(model, Profile())` on the reference module, in full mode only.
- `Attestation.result == "verified"` → passed.
- A finding → failed.
- A check with an unknown or timed-out result → inconclusive.
- `solver_notice()` not None → unsupported, with owner "environment".

The report records the attestation hash, the verifier version (`engine-info`) and the profile
(`checks`, `rlimit`).

In quick mode, C19 is UNSUPPORTED with the reason "static verification runs in full mode". This
is never a pass.

**Rationale**: FR-025 and SC-012. The solver is present in the dev shell
(`solver_notice() is None`).

## R10. Readiness checks (core#29, core#31)

**Decision**: A readiness registry in the suite's `conftest.py` maps each open upstream issue
to the scenarios it blocks, and a scenario marked `blocked_by(...)` reports BLOCKED. The
analysis shows that neither issue blocks a 506 scenario:
- **core#29** concerns Core's binding conformance fixtures, which 506 does not use. Backend
  conformance runs `run_conformance`.
- **core#31** concerns source-location relativization. 506 compares identities and records,
  which carry no locations.

The registry therefore lists both with empty affected sets and these reasons. A self-test proves
that a `blocked_by` scenario is reported BLOCKED and not passed.

**Update (implementation, 2026-10-16)**: C19 found a verification gap, filed as
behavior-ir-core#35. The verifier cannot discharge Approval's inductive preservation argument,
so the uniqueness postcondition is inconclusive. The registry lists core#35 as affecting C19
with `"run": true`: C19 runs, and it reports `blocked` with the exact solver result only when
the result is that documented limitation. Approval's semantics and hashes are unchanged
(decision 2026-10-16: no semantic change to Approval to make the report green). The
integration gate (`full`) passes with the recorded blocker, and full conformance (`complete`)
does not.

**Rationale**: FR-022 wants dependencies explicit, not repaired. An empty affected set is the
analysed result and is visible in the report.

## R11. The conformance report

**Decision**: A canonical JSON document (sorted keys, the existing canonical-JSON convention)
written to `target/conformance/506-report.json` by a pytest hook in the suite's
`conftest.py`. It has:
- `format: "behavior.conformance_report.v1"`;
- `mode` (quick or full);
- the Core release (from `core-release.json`) and `engine-info`;
- the model kinds' versions (from receipts) and the profiles;
- one entry per check: `id`, `scenario`, `level`, `status`, `reason`, `owner`.

Statuses come from the test outcome:
- a pass → `passed`;
- an assertion failure → `failed`;
- the suite's `Outcome` exceptions → `inconclusive`, `unverifiable`, `unsupported` or `blocked`.

Every other exception → `failed`. A plain pytest skip is not allowed for scenario tests: a
`skip` → `failed`. That way nothing silently passes.

The document has no timestamps, so it is deterministic.

**Rationale**: FR-021 and SC-011, using only pytest and the existing canonical-JSON style. The
report format is a document schema in an existing format, not a new serialization format.

## R12. Orchestration and CI

**Decision**: `scripts/conformance.sh [quick|full]`:
1. runs `cargo test -p behavior-models --test e2e_reference`, which writes the Rust identity;
2. runs `pytest models/examples/purchase_approval`, which writes the report;
3. prints the summary.

The modes differ in what fails the script:
- **quick**: any `failed`. Static verification and full property depth are skipped.
- **full**: any status other than `passed` or `blocked`.

`scripts/gates.sh` replaces `pytest … models` with
`pytest … models --ignore=models/examples/purchase_approval` and adds the gate
`conformance` (`scripts/conformance.sh quick`, or `full` when `BEHAVIOR_FULL_PROPERTIES=1`,
which CI already sets). `ecosystem-ci.yml` uploads `target/conformance/` as an artifact with a
pinned `actions/upload-artifact`.

**Rationale**: One runner keeps the two C16a halves ordered. Reusing the existing tier variable
keeps one quick-or-full switch for the whole project.

## Risks carried into implementation

These are known, not open questions. Each has a defined fallback that the report makes visible.

- **C12 construction.** The 503 child-state technique (seeded parents, direct evaluation) must
  reach the postcondition with the reference module's guards. If a configuration cannot reach
  it without bypassing public interfaces, C12 reports `unsupported` with owner `ecosystem` and
  the reason. The guard-level refusals (C07, C08) remain covered.
- **Refusal codes and the CLI error shape.** The invalid-input and invalid-snapshot codes are
  taken from Core's documented error catalogue, not observed. If the CLI's structured error
  differs in shape from the binding's `BehaviorError`, `N_E` maps both to the code alone
  (contract `comparison.md` §2). If the codes themselves differ, that is a C16b failure, not
  something to normalize away.
- **Tampering a stored transition.** C14's backend control wraps the backend's public
  `Backend` protocol. If the in-memory backend cannot be wrapped through public surface, that
  control is `unsupported` (owner `ecosystem`). The record-level controls remain.
