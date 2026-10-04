# Research: First-Class Reads

**Feature**: 010-first-class-reads | **Date**: 2026-10-03

Each decision has the form decision / rationale / alternatives. No open NEEDS CLARIFICATION
remain: the spec's clarifications of 2026-10-03 fixed projections, the entry-point rule,
cardinality and disclosure. These decisions settle the rest.

## R1. One read definition shape, two places

**Decision.** A read is defined by one wire shape, `WRead`. It can sit in two places:

- the module's new `reads` section, as a **declared read**, which gets a name-table entry and a
  hash and so becomes part of the behavior version;
- a standalone **read document** that a trusted host passes per call, as an **ad-hoc read**. It is
  admitted against the module on use and never enters the module.

Both go through the same admission (`admit_read`) and the same evaluator.

**Rationale.** "Ad-hoc" and "declared" differ in trust and identity, not in semantics. One shape
gives one typing path, one evaluator, one record format and one replay path. In Python, the same
`@read` function is either listed in `BehaviorModule(..., reads=[...])` or passed directly to
`store.read(...)`.

**Alternatives.**
- A bare expression for ad-hoc reads: rejected. It has no parameter declarations, so an ad-hoc
  read with inputs or bound entities would need a second, implicit typing context.
- A different encoding for each kind: rejected, because it gives two evaluators and two replay
  paths for the same semantics.

## R2. `WRead`: parameters and body

**Decision.**

```text
WRead  = { name, params: [WParam], body: WReadBody, loc }
WReadBody = { "value": WExpr }
          | { "project": { "over": WExpr, "param": ident, "items": [WItem] } }
WItem  = { "field": ident } | { "derived": ident }
```

- **Parameters** reuse `WParam`:
  - `state` entity parameters are entities bound by identity from the snapshot, which must exist
    at the read's state;
  - `input` and `context` parameters are as for actions.
  - Read-bound entities reuse the `state` role because binding is the same act (resolve the
    identity at one exact state, refuse it if absent), and the capability boundary already
    names these entities as `targets`.
- **Value body.** Any expression the module's derived values may contain: fields of bound
  entities, derived values, queries and folds, `exists` / `referenced`, and exact arithmetic. It
  may not be entity-typed or query-typed; that remains `TYPE_MISMATCH`, as today.
- **Projection body.** `over` is one of two things:
  - a **query expression** (`select`, `where`, set algebra), giving a **query projection**;
  - the name of a `state` parameter, giving an **entity projection**.

  `param` names the member variable that `items` are about. Every item is either a stored field
  of the entity type (including `id`, which is always present and need not be listed) or a
  derived value with exactly one parameter, whose type is `Entity<T>`.

**Rationale.**
- A query is "not a value" in the existing typing. Here the projection is the context that gives
  it a meaning, so no new value type (sets, records) enters the expression language.
- Items are names, not expressions, which enforces "no ad-hoc traversal" by construction: a
  projection can only expose stored fields and declared derived values.

**Alternatives.**
- Items as arbitrary member expressions: rejected. That would allow `o.customer` traversal and
  computed columns without names, the hidden join mechanism the clarification excluded.
- A record value type in the expression language: rejected. It is a much larger change (struct
  types are explicitly out of scope).

## R3. Admission rules for reads

**Decision.** `admit_read` and the `reads` section of module admission check:

- the parameters, as action parameters: known types and unique names, with the roles `state`,
  `input` and `context` only;
- the body types, with the derived-value expression context: queries are allowed, effects do not
  exist in the expression grammar, and an entity-typed result is refused;
- for a projection:
  - `over` is a query of `T`, or a `state` parameter of type `Entity<T>`;
  - every item is a field of `T` or a derived value `d(x: Entity<T>)`, otherwise
    `UNKNOWN_PROJECTION_ITEM`, naming the item;
  - no item appears twice, and no field name equals a derived item name in the same projection
    (`DUPLICATE_PROJECTION_ITEM`);
  - an item that names a reference path is refused as unknown (it is not a field name).
- **`READ_CALL_NOT_ALLOWED`.** A `Derived` call whose name is a declared read (in any module
  expression, including another read's body) is refused with this code. The message says to
  move the shared computation into a derived value.
- **Capability namespace.** A declared read may not share its name with an action or a derived
  value (`DUPLICATE_CAPABILITY`). Actions and reads are capabilities that a caller names, and a
  derived-value name must keep meaning exactly one item, so a call `d(x)` is never ambiguous
  between a derived call and `READ_CALL_NOT_ALLOWED`.
- **Effects.** There is no effect syntax inside expressions, so "a read cannot write" holds
  structurally in the IR. The Python DSL raises `BehaviorDefinitionError` when `set_`, `create`,
  `remove`, `requires` or `ensures` is used inside a `@read` body.

**Rationale.** These are the spec's FR-004, FR-006, FR-008 and FR-015a, made checkable at
admission. The capability namespace keeps "the agent's list of capabilities" unambiguous.

**Alternatives.**
- Letting a read and an action share a name, told apart by kind: rejected. It confuses agents and
  capability listings for no benefit.

## R4. Hashing and versions

**Decision.**
- **Hashes.** New tags `behavior.read.v1` (a declared or ad-hoc read) and `behavior.projection.v1`
  (its projection body). A new name-table kind, `Kind::Read = 8`. Reads hash like other items:
  parameter types, body expression hashes, and projection items by field name or by the derived
  value's hash.
- **Identity compatibility.** The module hash folds the name table, so a module without reads has
  exactly the entries, and therefore the hash, it had before (FR-021). This is verified with
  the existing hash vectors and frozen version fixtures.
- **Formats.**
  - Wire IR **0.7**: a `reads` section. A module that uses it needs 0.7, and earlier documents
    are unchanged. The gate is `NeedsReadVersion`, like `NeedsQueryVersion`.
  - A standalone read document carries `ir_version: "0.7"`.
- **Versions.** `VERIFIER_VERSION` becomes **0.6.0** for read checks (R10). The release is
  **0.10.0**.

**Alternatives.**
- Reads outside the behavior version: rejected by the spec (FR-015). Declared reads are
  capabilities, and capabilities are behavior.

## R5. Evaluation: a read is a transition-free run of the existing evaluator

**Decision.** A new `run_read` in `eval.rs` sits next to `run_transition` and uses the same
`Evaluator`:

1. **Decode** the parameters. `decode_sections` is generalized from `&ActionItem` to a parameter
   slice. Missing or wrong values give `INVALID_INPUT`, with every problem listed.
2. **Facts.** In plain mode, the supplied facts are checked with `check_snapshot` and
   `check_query_snapshot`, both generalized to a parameter slice. With a provider, a `facts`
   section is refused, as today.
3. **Value read.** `ev.eval(body, vals, Phase::S)` gives a value. On failure the result is
   `EVALUATION_ERROR`, with reason `EVALUATION_ERROR`, or `UNKNOWN_FACT` for a missing fact, and its
   location.
4. **Query projection.**
   - The query members come from the same query-fact path as folds, so the query instance is
     recorded as a query fact.
   - Members are sorted by identity.
   - For each member, each item in projection order:
     - a stored field is read through `facts.field` (recorded as a field fact), or through the
       bound value for a bound entity;
     - a derived item is evaluated through the existing `DerivedRef` path, with the member as
       the candidate, so memoization, derived entries and dependency facts behave exactly as in
       transitions.
   - The first failure ends the read with `EVALUATION_ERROR`. The reason names the member
     (`T#id`) and the item.
5. **Entity projection.** Same as a query projection, for the one bound entity. Its stored fields
   are recorded as observed field reads `(param, field)`, as for any bound entity.

Phase is always S. There is no S′ and no rules or invariants run: a read checks nothing about
validity, it observes a state that is already valid.

**Projected stored fields are always observed.** Each projected stored field is read
explicitly, never through short-circuiting, so every one is recorded (FR-007), regardless of
what the derived items read.

**Rationale.** It reuses exactly the semantics transitions already use, which is FR-006a's "no
separate read-only implementation". The evaluator already supports evaluation with no S′, as
`check_global_invariants` and `query_matches` show.

**Alternatives.**
- A separate read evaluator: rejected, because it would duplicate semantics.
- Evaluating a read as a synthesized effect-free action: rejected. That is the TCUP workaround,
  it needs a bound entity, and its result is ALLOW/DENY.

## R6. The read record

**Decision.** A new record kind with format tag `behavior.read_record.v1`. Its fields, in
canonical JSON:

| Field | Content |
|---|---|
| `format` | `"behavior.read_record.v1"` |
| `behavior_version` | the module hash |
| `read` | `{ "name", "hash", "declared": bool }`, plus `"definition"` (the read's canonical wire) for an ad-hoc read, so the record is self-contained |
| `data_version` | as for decision records: store identity and state reference against a store, host-supplied in plain mode |
| `state` | the bound entities' canonical values (only `state` parameters) |
| `input`, `context` | decoded and normalized |
| `result` | `VALUE`, `EVALUATION_ERROR`, `INVALID_INPUT` or `INVALID_BINDING` |
| `value` | for `VALUE`: the canonical value, or for a projection the records `[{"id": …, item: …}]` (one object for an entity projection) |
| `reasons` | for anything other than `VALUE` |
| `derived` | derived entries, as in decision records (phase always `S`) |
| `observed` | the field reads `(param, field)` of bound entities, sorted |
| `facts` | observed facts, as in decision records (existence, identities, references, queries, fields) |
| `record_id` | `read:` + the display form of `H(behavior.read_record.v1, canonical(record without record_id))` |

- **Absent values.** An absent optional value is the canonical absent encoding (`null`), present
  under its key and never omitted (FR-007). An evaluation failure never yields `null`, because
  it fails the whole read (FR-006b).
- **What `record_id` binds.** It is computed over everything else, so it binds the read, the
  behavior version, the exact state (through `data_version`), the parameters, the observations
  and the result. "Result + record identity" therefore names exactly one record (FR-009).
- **Trace.** The trace of a read is its `derived` entries plus its `observed` and `facts`.
  Transitions' per-precondition trace steps have no counterpart in a read.

**Rationale.** It follows the decision record (self-contained, no timestamps, no host data), so
replay and audit tooling carry over. A separate format tag keeps reads from ever being mistaken
for decisions or transition records.

**Alternatives.**
- Reusing `DecisionRecord` with `result: "VALUE"`: rejected. Its version line (0.4–0.6) and its
  `action` and `changes` fields belong to transitions, and mixing them would let a read record
  pass for a decision.
- No `record_id` inside the record: rejected. The capability response must carry an identity
  that a reader can check against the record.

## R7. Read execution: capability response and evidence as distinct types

**Decision.** Every read returns `ReadExecution { record: ReadRecord, response: ReadResponse }`.

- **`ReadResponse`** holds `{ result, value | reasons, record_id }`. It never contains `state`,
  `facts`, `observed` or `derived` (FR-016a).
- **`ReadRecord`** is the full evidence.

The two are distinct Rust types, Python classes and CLI outputs. Nothing converts one into the
other, except a `ReadResponse` built from a record by the engine.

For a direct trusted-host read, the host receives both. For a read intent, the engine still
returns both, and the type the host forwards to the agent is `ReadResponse`.

**Rationale.** The clarification fixes disclosure in the engine, not in host routing. Separate
types make "forward the record by mistake" a type error in Rust and Python.

**Alternatives.**
- A single object with an `include_trace` flag: rejected. That makes trace exposure
  configurable, which the clarification rules out.

## R8. Store reads

**Decision.**

```text
Store::read(&self, module, read: &ReadSource, bindings, input, context, at: Option<&StateRef>)
    -> R<ReadExecution>
```

- **Position.** `at = None` reads the head once, which is the snapshot. A past `StateRef` must be
  a state of this store (`state_at(position) == at`), or the read is refused like `load`.
- **Schema.** `bind_schema(module, position)` gives `SCHEMA_MISMATCH` before anything is
  evaluated (FR-013).
- **Bindings.** Bound entities are loaded with `exists_at` and `version_at` at the position. An
  identity that does not exist there gives a record with result `INVALID_BINDING`. Its existence
  fact (`false`) is recorded, so replay reproduces it, and nothing else is evaluated.
- **Errors without a record.** `SCHEMA_MISMATCH`, and a `StateRef` that is foreign or beyond the
  head, stay `Err`. There is no valid snapshot to record (FR-009).
- **Facts.** `StoreFacts { position }`, as for transitions, so every value comes from that one
  position (FR-002, FR-014).
- **No writes.** The method takes `&self`. The backend trait's write methods need `&mut`, so a
  read cannot write by construction (FR-012).
- **Performance.** A query projection fetches each member's version once and answers its stored
  fields from it, through a new defaulted `EvaluationFacts::entity(entity, id)`. Every field
  answered this way is still recorded as a field fact. This keeps SC-004 (10,000 × 5) at one
  `version_at` per member.

**Alternatives.**
- `&mut self`, plus a test that nothing changed: rejected. A type-level guarantee is stronger
  than a test, and the test (SC-002) is kept anyway.

## R9. Read intents (capability boundary)

**Decision.**

```text
evaluate_read_intent(module, intent, host) -> Result<ReadExecution, IntentRejection>
Store::read_intent(&self, module, intent, context, at) -> R<Result<ReadExecution, IntentRejection>>
```

- **Intent shape.** The same as action intents: `{capability, targets, input}`.
  - `capability` must name a declared read, otherwise `UNKNOWN_CAPABILITY`. An action name is
    also unknown here, and vice versa: action intents cannot name reads.
  - `targets` gives the identities of the `state` parameters.
  - Context comes from the host.
- **Validation.** Every problem is listed before evaluation (`MISSING_TARGET`, `EXTRA_TARGET`,
  `MISSING_ARGUMENT`, `EXTRA_ARGUMENT`, `WRONG_TYPE`, and so on), reusing the action-intent
  validator, generalized to a parameter list. Against a store, each target must also exist at the
  read position: a missing one is `UNKNOWN_TARGET`, listed together with every other problem.
- **No ad-hoc reads.** An ad-hoc definition in an intent is `EXTRA_ARGUMENT`: the intent may only
  carry `capability`, `targets` and `input`.

**Rationale.** It is the same validated boundary as for actions (FR-016). Separate entry points
make "an agent asked a question" and "an agent asked to change something" distinct calls, so
neither can be turned into the other.

## R10. Verifier: evaluation errors in declared reads

**Decision.** `verify` runs `evaluation_error` for every declared read (FR-017), reusing the
encoder of action expressions.

- **Subjects.** The check's subject is `read:<name>`.
- **Value read.** The obligations are those of its body: division by zero, unwrap of absent,
  range and rescale overflow, under the module's constraints and invariants on the bound
  entities and the state.
- **Query projection.** For each derived item, the obligations of the derived body are checked
  for a symbolic member `m: T` that satisfies the query's predicate. Predicates are
  candidate-local by admission (`NON_LOCAL_PREDICATE`), so they encode over `m` alone:
  - `where` gives a conjunction;
  - `union` gives a disjunction;
  - `intersection` gives a conjunction;
  - `difference` gives `a ∧ ¬b`.

  A counterexample is confirmed by evaluating the read on the concrete state, as for actions.
- **Entity projection.** The same obligations, for the bound entity.

`VERIFIER_VERSION` becomes 0.6.0, because new subjects can change verification outcomes for
modules that declare reads. Modules without reads get identical results, which is checked with
the existing verify fixtures.

**Alternatives.**
- Skipping the query predicate (any member of `T`): rejected. It reports counterexamples that the
  filter excludes, and confirmation would then fail as inconclusive.

## R11. Replay of read records

**Decision.**

- **Plain replay.** `replay_read(module, record) -> ReplayResult` re-evaluates from the record's
  own `facts`, `state`, `input` and `context`:
  - for an ad-hoc read, the definition is re-admitted from `read.definition`;
  - for a declared read, the read is looked up by name, and its hash must match.

  The result is compared with `first_difference`, including `record_id` (FR-011).
- **Store replay.** `Store::replay_read(module, record)` parses `data_version`, checks that the
  store identity and the state reference are this store's, re-reads at that position with a
  fresh provider, and compares the result byte for byte.
- **Tampering.** An altered result, fact, state reference or identity is reported as the first
  differing path. A forged `record_id` with otherwise valid content is reported at `record_id`.

## R12. Python surface

**Decision.**

```python
@read
def open_total(customer: Customer) -> Exact[Money]:
    return sum_(select(Order).where(lambda o: o.customer == customer.id), lambda o: o.amount)

@read
def order_view(order: Order):
    return project(order, lambda o: [o.status, o.amount, total(o)])

@read
def active_cultures():
    return project(select(Culture).where(lambda c: c.active), lambda c: [c.name, c.stage, age(c)])

model = BehaviorModule("lab", entities=[...], derived=[...], actions=[...], reads=[order_view, ...])

behavior.evaluate_read(model, open_total, state={"customer": {...}}, data_version=..., facts={...})
store.read(model, open_total, bindings={"customer": "c1"}, at=None)
store.read_intent(model, {"capability": "order_view", "targets": {"order": "o1"}}, context={})
behavior.replay_read(model, record);  store.replay_read(model, record)
```

- **`project(over, items)`.** `items` is a lambda over the member that returns a list.
  - Each element must be a field of the member (`o.status`) or a derived value over the member
    alone (`total(o)`).
  - Anything else raises `BehaviorDefinitionError` (`UNKNOWN_PROJECTION_ITEM`) at trace time,
    and the engine checks again.
  - The lambda mirrors the existing `where` and `sum_` lambdas.
- **Read results.**
  - `ReadResult`: `.result`, `.value`, `.reasons`, `.record_id`, `.record` (a `ReadRecord`).
  - `ReadResponse` is the capability type. `read_intent` returns `ReadExecution(response, record)`.
- **Declared reads.** A `@read` function used inside another behavior body raises
  `BehaviorDefinitionError` with `READ_CALL_NOT_ALLOWED`.

## R13. Command line

**Decision.** Three new commands:
- `behavior read <module.json> <request.json>`: plain mode. The request names a declared read or
  carries a read document, plus `state`, `input`, `context`, `data_version` and `facts`. It
  prints the read record.
  - Exit codes: 0 for `VALUE`, 1 for `EVALUATION_ERROR`, 2 for refused input.
- `behavior read-intent <module.json> <intent.json> <host.json>`: prints the read response. With
  `--record <path>`, it also writes the record.
- `behavior read-replay <module.json> <record.json>`: exit code 0 on a match.

Store reads stay a library and binding concern, as store transitions are today.

## R14. Validating the success criteria

- **SC-001**:
  - new example `examples/lab_reads/`, modeled on the external application's read patterns: a
    whole-record view, a list with filter, a count, an existence check, a past-position audit
    question, and a declared-read capability set;
  - no bound placeholder entity and no effect-free actions;
  - a test asserts that the module declares no action without effects.
- **SC-002 and SC-003**: a proptest over generated histories of up to 1,000 operations, mixing
  reads, transitions and one migration:
  - the store bytes (head, records, versions) equal a reference history without reads;
  - every read record replays, plain and against the store;
  - a mutated record (result, fact, `data_version`, `record_id`) never replays.
- **SC-004**: an ignored release test, a projection of 5 items × 10,000 entities on the in-memory
  backend in under 2 s (3 stored fields and 2 derived values), asserting 30,000 recorded field
  facts (3 stored fields × 10,000).
- **SC-005**: reads at sampled past positions are recorded, more transitions are committed, and
  the reads are repeated: the records are byte-identical.
- **SC-006**: intent fixtures (`tests/fixtures/read_intents/`) for unknown read, action name,
  missing or extra targets and inputs, wrong types, and ad-hoc smuggling. A type-level test
  checks that `ReadResponse` has no evidence fields.
- **SC-007**: the existing hash vectors, frozen versions, golden records and store fixtures pass
  unchanged, and the determinism check runs the new read fixtures twice.
