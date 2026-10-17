# Research: WorkQueue Model (feature 507)

All facts are from the pinned Core 0.13.0 (`behavior-ir-core` tag `v0.13.0`, commit `1509497`)
and this repository at `main` (`1171f10`). "Executed" means confirmed by running the probe in
R1, not only by reading code.

## R1. Planning gates confirmed by execution

**Decision**: every planning gate of the spec except the effect binding of transitions (R3)
is expressible with existing public Core and model interfaces.

**Evidence**: a hand-written module shaped like the WorkQueue lowering. It has:
- a generated-style status enum and a `Work` entity with `Option[Id[Worker]]` and `Option[Time]`
  fields;
- the `ValidWork` per-entity invariant;
- an `enqueue` creation;
- an embedded StateMachine with `claim`, `renew` (LEASED → LEASED), `complete` and `cancel`
  (READY | LEASED → CANCELLED);
- generation postconditions;
- an identity-only `ready(now)` read.

The probe was run through the public Python binding and Store:

| Gate | Result |
|---|---|
| 1, 2 generated declarations, creation, self- and multi-source transitions | admitted (`sha256:3c32bf8a…`) |
| 3 optional owner and deadline; `is_some` conjunction with `value_or` | admitted; runtime correct |
| 4 attempt increment and bound | claim `evaluation_error` check **proven** (no overflow); preservation **proven** |
| 5 generation guards and postconditions | stale completion DENY, late completion DENY, current ALLOW; `attempts == generation` and `attempts == expected_generation + 1` **proven** |
| 6 local invariant | preservation **proven** for every action; a genesis LEASED item without owner → `GENESIS_INVALID: Work#x: invariant valid_work is violated` |
| 7 identity-only read | `ready(now=3)` → `[]`, `ready(now=5)` → `[{"id": "d1"}]`; the read record lists `now` as a captured context fact |
| 9 reused id | `ENTITY_ID_ALREADY_USED` |
| verifier overall | `verified` (dead-action and redundant-precondition checks return counterexamples, which means the actions are live and every precondition matters) |

**Alternatives considered**: relying on code reading only (the review agents' findings). This
was rejected because the spec requires planning to re-confirm by execution.

## R2. Lowering structure

**Decision**:

```text
L_WQ(Q) = Compose( Decls(Q), Inv(Q), Defs(Q), Enqueue(Q), L_SM(SM(Q)), Reads(Q) )
  Decls   = { <E>Status enum, E entity }                 E = EncodeEntityName(id)
  Inv     = { <id>__valid_work : ValidWork over E }       (new contribution kind: invariant)
  Defs    = { <id>__terminal(w) }                         (existing: derived definitions)
  Enqueue = <id>__enqueue                                 (creation, like RequestFlow's)
  SM(Q)   = (E, status, {READY}, 𝒯_Q)                     claim … cancel, by the unchanged lowerer
  Reads   = { <id>__ready(now), <id>__expired(now) }       (new contribution kind: read)
```

WorkQueue reuses the 503 declaration DAG for the enum and the entity, and the 503 definition
mechanism for `terminal` (added in P2, so hand-written behavior can call it). **As implemented**,
the invariant and the two reads are added by WorkQueue itself in P4, with its actions, through
the public builder (`add_invariant`, `read_projection`, `add_read`); a name clash is
`GENERATED_NAME_COLLISION`. They are not new common contribution kinds: no other model produces
them and nothing hand-written refers to them (Constitution VI). The invariant still covers every
action of the final module, because Core checks a per-entity invariant on every resulting state,
whoever wrote the action. The 502/503 neutrality rule applies: WorkQueue vocabulary never appears in
`composition.rs`, `identity.rs`, `source_map.rs` or `ids.rs`.

**Rationale**:
- the invariant must cover every action (Q17), which only a Core invariant does;
- the reads are ordinary Core reads (Q19);
- the transitions use the StateMachine lowerer (FR-002).

**Alternatives considered**:
- postconditions on each action, as Approval does. Rejected by Q17: they cover only the
  queue's own actions;
- lowering the transitions directly without StateMachine. Rejected by FR-002.

## R3. Effects on other entities from transitions — **decided: D1(a)** (2026-10-17)

**Finding**: StateMachine transitions have no state bindings. The lowered parameters are the
subject, then the inputs, then the context (`state_machine/lower.rs:46-63`, `TransitionInput`
in `state_machine/input.rs:43-54`), and Python `StateMachine.transition` has no `state=`. Only
RequestFlow creations bind extra entities (`request_flow/input.rs:43`).

The spec (FR-030, Q26, W24, reference step 9) requires that all nine capabilities may carry
effects on *other explicitly bound entities*. For enqueue, which is a creation, this works as
in RequestFlow. For the eight transitions, it is a **missing public facade capability** in the
model layer, not a Core gap: Core actions can take any number of state parameters.

**Options**:
- **(a) Recommended.** Add optional state bindings to StateMachine transitions in the model
  authority:
  - `TransitionInput.state: Vec<(param, entity)>`, as in `CreationInput.state`;
  - the bindings are lowered after the subject, sorted by name;
  - the canonical document includes `state` only when non-empty, so every pinned 501, 502 and
    503 case stays byte-identical. The compat gate proves this, and no version bump is needed;
  - the StateMachine ownership exemption in `final_pass` is narrowed from "any generated action
    of the owner" to "the owner's generated action, writing through its *subject* parameter".
    This closes the caveat found in the review: a binding of the subject type can no longer
    write the owned field. It also changes no existing case, because no transition has
    bindings today;
  - RequestFlow and WorkQueue transitions gain the same option;
  - WorkQueue then rejects any binding of its own WorkItem type (FR-020).
- **(b)** Reduce v1: transitions carry guards and command intents only, and effects on other
  entities happen through a separate action or command. Spec FR-030, W24 and reference step 9
  would then be revised.
- **(c)** WorkQueue lowers its transitions without the StateMachine lowerer. Rejected: FR-002.

**Decision (user, 2026-10-17)**: (a). It is a general model-layer capability: a transition
may change several entities atomically. A binding never grants authority: what a transition can
express is separate from what it may change. The decision comes with requirements 1–8, which
are applied as follows:

1. **Existing semantics preserved.** A transition without bindings lowers to the same Core IR,
   has the same canonical representation and keeps the established compat mappings (R13).
2. **Versioned.** See R13. Omitting an empty `state` is not relied on for versioning.
3. **Identity.** Bindings are part of the canonical document. Each binding is recorded as
   (parameter name, the entity's exact declaration identity under feature 505) and sorted by
   name, so authoring order is irrelevant. The lowered parameter order is the subject, then the
   bindings sorted by name, then inputs and context.
4. **Ownership.** The StateMachine exemption in `final_pass` covers exactly one thing: the
   owner's generated action writing the owned field through its **subject** parameter, which is
   the generated status effect. An authored fragment writing the owned field through any
   parameter of the subject's type, bindings included, is `TRANSITION_WRITES_OWNED_FIELD` at
   lowering. A binding's other fields are governed by every other model's ownership pass,
   unchanged.
5. **Core binding semantics.** Bindings are ordinary Core state parameters. Typed identity,
   aliasing (two bindings to one id), resolution against the snapshot, and read- and write-set
   concurrency are Core's. No model-specific rule is added.
6. **Composition.** Extra effects are ordinary Core effects. The final passes of WorkQueue,
   RequestFlow, Approval and every StateMachine inspect every parameter by type, bindings
   included (SA-OWN).
7. **Compatibility first.** Phase 0 captures the baseline: every pinned case and the 506
   report. If an established identity changes, stop and investigate. Golden files are never
   updated to make tests pass.
8. **Shared.** Bindings live in `state_machine/` and the shared composition. RequestFlow
   transitions and WorkQueue consume them. WorkQueue does not duplicate the binding lowering or
   validation; it only adds its own restriction (`WORK_ITEM_BINDING`).

## R13. Versioning of the transition-binding extension

**Constraint**: 501 FR-001 requires the model semantic version to change when the admitted
structure of the model language changes, and adding transition bindings is such a change.
Requirement 1 requires that existing models keep their canonical representation and hashes.

**Decision: language levels.**
- **StateMachine semantic version 3** is the extended language: transitions may have `state`
  bindings, and its canonical format is `behavior.model.state_machine.v3`, in which every
  transition has a `state` list (possibly empty).
- **Version 2 stays exactly as established**: the same format, the same canonical document and
  the same mappings.
- A machine's semantic version is determined by its content. It is 2 when no transition has a
  binding and 3 otherwise. The receipt records it.
- The function `(state_machine, 2, 1, Core 0.13.0, ModelHash) → BehaviorHash` is unchanged and
  its tuple stays established. No supersession happens, because nothing is replaced. A new
  `state_machine` 3/1 tuple is added, with binding reference cases (`sm_bind_*`).

**Effect on embedding and co-supplying kinds**:
- **RequestFlow** follows the same rule. Its transitions are the embedded machine's, so a flow
  is semantic version 3 with an embedded machine 3 when any of its transitions has a binding,
  and is otherwise unchanged at 2 with embedded machine 2. Creation bindings already exist in
  v2 and are not affected. A new `request_flow` 3/1 tuple (embedded `state_machine` 3) is added
  with `rf_bind_*` cases.
- **Approval** is unchanged at 1. Its `co_supplied` key records the actual versions of the
  co-supplied models, so a case co-supplied with a v3 machine or flow falls into a new approval
  tuple. Existing approval tuples are untouched.
- **WorkQueue** 1/1 records its embedded machine's version by the same rule. A queue whose
  transitions bind other entities embeds machine 3; one without bindings embeds machine 2.
  Both kinds of case are pinned (`wq_*` with and without bindings), giving one `work_queue`
  tuple per embedded version.

**Rejected alternative**: a global bump of StateMachine to 3 with supersession of 2, as feature
505 did. It would change the canonical document, and therefore the ModelHash, of every
existing StateMachine, RequestFlow and co-supplied Approval. That contradicts requirement 1,
and the change would not be in the meaning of those models.

**Gate check (verified by reading `scripts/check-models-compat.py`)**:
- the gate groups reference cases by the tuple key from their receipts (`tuple_key`, KEY_FIELDS
  include `model_semantic_version`) and checks each current group against its manifest entry.
  Several current tuples of one kind at different semantic versions therefore coexist with no
  supersession;
- one constraint: a reference case must map to a single tuple. A case mixing a standalone
  machine at version 2 with one at version 3 is rejected ("no single version tuple"), unless an
  embedding tuple covers both. The new reference cases keep one language level per case. Phase
  1 adds a gate self-test for coexisting 2 and 3 tuples.

## R4. Ownership

**Decision**: a WorkQueue ownership pass over the final module, `work_queue/ownership.rs`, in
the style of Approval's. For every action and every parameter of entity type `E_Q`, it checks
each effect, creation and removal against this table (FR-019):

| Item | Allowed writers | Error |
|---|---|---|
| creation of `E_Q` | `<id>__enqueue` | `UNAUTHORIZED_WORK_CREATION` |
| removal of `E_Q` | nobody | `UNAUTHORIZED_WORK_REMOVAL` |
| `payload` fields | `<id>__enqueue` | `UNAUTHORIZED_WORK_WRITE` |
| `attempts` | enqueue, claim | `UNAUTHORIZED_WORK_WRITE` |
| `owner` | enqueue, claim, complete, retry, fail, recover, exhaust, cancel | `UNAUTHORIZED_WORK_WRITE` |
| `lease_until` | enqueue, claim, renew, complete, retry, fail, recover, exhaust, cancel | `UNAUTHORIZED_WORK_WRITE` |
| `available_at` | enqueue, retry, recover | `UNAUTHORIZED_WORK_WRITE` |
| `status` | enqueue (literal READY), the embedded machine | existing StateMachine pass (narrowed per R3) |

An allowed writer passes only when it writes through its subject parameter, which is the
created entity for enqueue. In addition:
- **at lowering**: an authored effect on the subject is refused (`AUTHORED_WORK_EFFECT`), and so
  is any `E_Q`-typed binding in a WorkQueue capability (`WORK_ITEM_BINDING`, FR-020);
- **at build**: each queue's table covers only its own `E_Q`. With several queues (W25), one
  queue's actions writing another queue's items fail through the other queue's table.

**Rationale**: the check is exact per (action, parameter, field), so it does not depend on the
"subject is the only binding" coincidence (Q35).

## R5. Build-time validation

New typed errors in `error.rs`, all raised at resolution or lowering:

| Code | When |
|---|---|
| `INVALID_MAX_ATTEMPTS` | M < 1 or M not representable as i64 |
| `TIME_NOT_ORDERED` | the time type is not a nominal declared with `order` (FR-006) |
| `MISSING_AUTHORIZATION` | enqueue, claim, cancel, recover or exhaust lacks an authored authorization guard (FR-021) |
| `RESERVED_PARAMETER` | an authored context or input parameter uses a generated name (`worker`, `now`, `lease_until`, `available_at`, `generation`, `expected_generation`, `id`) (FR-022) |
| `AUTHORED_WORK_EFFECT`, `WORK_ITEM_BINDING` | R4 |
| `UNAUTHORIZED_WORK_*` | R4 |
| `PAYLOAD_FIELD_RESERVED` | a payload field named `status`, `attempts`, `owner`, `lease_until` or `available_at` |

Name collisions with other models use the existing composition errors (W22).

## R6. Identity and versions

**Decision**:
- model kind `work_queue`, `WORK_QUEUE_SEMANTIC_VERSION = 1`, `WORK_QUEUE_LOWERER_VERSION = 1`;
- the embedded StateMachine version is recorded as in RequestFlow.

The canonical document holds:
- `id` and M;
- the payload schema;
- the worker and time types by declaration identity (feature 505);
- per capability: authored inputs and context with their types, guard, effect and command
  bodies as Core hashes, and the authorization guard.

Display names, descriptions and locations are excluded. A new `work_queue` tuple goes into
`models/fixtures/compat.json`, with reference cases `wq_*`. All existing tuples stay
byte-identical, which is checked by the gate.

**Rationale**: the pattern of 502 and 503. ModelHash covers M and the guards (W19).

## R7. Python authoring API

**Decision**, mirroring RequestFlow and Approval (Python only records what the author wrote):

```python
q = WorkQueue("document_work", worker=Worker, time=Time, payload={"document": Id[Document]},
              max_attempts=3)
q.enqueue(authorized=lambda id, document, available_at, actor: actor == ADMIN_ID,
          context={"actor": Id[User]}, emits=[...], state={...}, effects=[...])
q.claim(eligible=lambda w, worker: any_(select(Worker), lambda x: (x.id == worker) & x.active))
q.renew(); q.complete(effects=[...], state={"doc": Document}, emits=[...]); q.retry(); q.fail()
q.recover(authorized=lambda w, actor: ..., context={"actor": Id[User]})
q.exhaust(authorized=...); q.cancel(authorized=...)
q.work_entity            # typed handle (FR-003)
q.terminal(w)            # derived-value handle
```

Every capability must be declared once. A missing one is `MISSING_CAPABILITY`, because the
lifecycle is fixed (Q1).

**Rationale**: explicit authorization per capability (Q8, Q25), and the same tracer protocol as
the existing models.

## R8. Trusted time and parameters

**Decision**:
- context `now`, `worker`, `lease_until` (claim, renew) and `available_at` (retry, recover)
  have fixed names and types;
- inputs `generation` (renew, complete, retry, fail) and `expected_generation` (claim) are Int;
- the Store records context in the decision and re-derives it at commit (R1, research by the
  review agent: `store.rs:2238`).

Nothing binds `now` to the commit time. The guarantee is relative to `now`, as the spec's Time
section states.

## R9. Conformance infrastructure reuse

**Decision**:
- move the kind-neutral parts of `models/examples/purchase_approval/report.py` into a shared
  package, `models/conformance/`:
  - the statuses and outcome exceptions;
  - the readiness entries;
  - the evidence recording;
  - the canonical writer;
  - `gate_failures`;
  - the pytest hooks, parameterized by feature id, scenario list and report path.
- 506 keeps its scenario list and report path. A regression test checks that the 506 report
  is byte-identical before and after the move.
- 507 adds `models/examples/work_queue/` with its own `report.py` (declared scenarios
  W01–W22, W23a, W23b, W24, W25, plus the VO obligations), its readiness registry and
  `target/conformance/507-report.json`.
- `scripts/conformance.sh <feature> [quick|full|complete]`.
- `gates.sh` runs `conformance.sh 506 quick` and `conformance.sh 507 quick`.

**Rationale**: Q15, Q23 and FR-037. 506's completion is untouched.

## R10. Evidence records per VO

**Decision**: the 507 report has an `obligations` section. Each VO has a list of dependencies
`{class, check, status}`, where class is `smt`, `structural`, `test` or `contract`. The VO's
status is the weakest dependency's, ordered `failed < blocked < inconclusive < unverifiable <
unsupported < passed`.

| Class | Source of the status |
|---|---|
| `smt` | the attestation's per-(action, kind) outcomes: `preservation`, `postcondition`, `evaluation_error`, with `proven` mapping to passed |
| `structural` | the named build-error tests and the lowered-IR inspection tests in `contracts/safety-arguments.md` |
| `test` | the scenario checks |
| `contract` | the Core/Store contract checks (genesis rejection, `STATE_CONFLICT`, backend conformance, replay) |

Assumptions (the Time section and the migration contract) are listed per VO under
`assumptions`, never as a dependency with a passed status.

## R11. Migration negative controls

**Decision**: use Core's public migration API (`Migration`, `apply_migration`) on a test store.
There are two controls:
- **(i)** reset `attempts` of a READY item that once had generation k, then lease it to its
  old owner by claim. A completion with the old generation k′ = k may then succeed. The control
  passes when the old-token completion is **allowed**, which shows the contract matters.
- **(ii)** a migration READY → LEASED keeping `attempts = k` and setting owner A. A
  completion by A with k succeeds after the migration.

Both run with the contract check **disabled** to show the break, and the report records the
migration contract as a host obligation (FR-036).

**Alternatives considered**: enforcing the contract in the model. This is impossible: the model
authority does not see migrations (Q37).

## R12. Cross-binding equality (W20)

**Decision**: a Rust integration test, `crates/behavior-models/tests/work_queue_reference.rs`,
authors the reference queue through `WorkQueueInput` directly, independently of the Python
glue, like `e2e_reference.rs`. The suite compares the BehaviorHash and ModelHash with the
Python build.
