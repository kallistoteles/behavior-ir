# Research: Entity Lifecycle (Universe Transitions)

Decisions for feature 006. The inputs are `spec.md` (FR-001–FR-019, FR-010a–e, clarifications of
2026-09-27) and the architecture of features 001–005.

## R1. Language forms

- **Decision**: add four forms to the Behavior IR, and nothing else.
  1. The effect `create(T, id, {field: expr, …})`: a creation effect with the complete initial value.
  2. The effect `remove(p)`: removal of the entity bound to the state parameter `p`.
  3. The predicate `exists(e)`: `e : Id<T>` or `Option<Id<T>>` (an absent value gives false).
  4. The predicate `referenced(e)`: does any entity of the evaluated state hold a `Ref` to `e`?

  In addition, the field type `Ref<T>` is sugar: an `Id<T>` field marked as a reference, plus a
  synthesized entity constraint `exists(field)` (FR-010b). In the DSL these are `create(...)`,
  `remove(...)`, `exists(...)`, `referenced(...)` and `Ref[T]`.
- **Rationale**:
  - The effects make lifecycle a kind of ΔS (FR-001).
  - `exists` is the single existence semantics (FR-010a).
  - `referenced` is needed so an author can *prove* a removal safe. Without it, the verifier always
    finds a counterexample: some other entity might hold a reference. It is defined as a
    **projection** of the incoming-reference fact, `referenced(id) := incoming(id) ≠ ∅`, and is
    state-relative like `exists`: on S for preconditions, on S' for postconditions and outgoing
    rules. It is never a separate fact source that could diverge (FR-010f).
  - `Ref<T>` compiles to `Id<T>` plus a constraint, so evaluator, verifier and replay see only
    `exists`.
- **Alternatives**:
  - A separate lifecycle action kind: rejected by FR-001.
  - `Ref<T>` as an independent type with its own checks: rejected by the clarification (two
    mechanisms).
  - No `referenced`: removal of any referenced type could never be verified.

## R2. Admission rules

- **Decision**:
  - **Complete initial value:** a creation must give every field of `T` except `id`, with the
    declared types. Missing, unknown or ill-typed fields give `CREATE_INCOMPLETE` /
    `UNKNOWN_FIELD` / `TYPE_MISMATCH`.
  - **Typed identity:** the identity expression must have type `Id<T>` (not `Id<U>`, not `String`).
  - **Removal target:** `remove(p)` needs `p` to be a state parameter of entity type.
  - **Statically visible conflicts** are refused as `LIFECYCLE_CONFLICT`:
    - two creations whose identities are the same input parameter;
    - a field update of a removed parameter;
    - removing the same parameter twice.
  - **Dynamic conflicts** are refused by evaluation with `LIFECYCLE_CONFLICT`: at most one
    lifecycle operation per typed identity per transition (FR-005). Two creations with equal
    identities at runtime, or a creation of an identity bound for removal, conflict. Both look at
    the same S, so `used = false` cannot allow two of them.
- **Rationale**: FR-002, FR-003, FR-005. Admission catches every conflict visible from the syntax;
  runtime values cover the rest.
- **Alternatives**: runtime-only checks. Rejected, because admission-time errors are cheaper and
  are part of what "admitted" means.

## R3. Evaluation facts: current-state facts and history facts

- **Decision**: evaluation reads facts through one interface, `EvaluationFacts`. It serves two
  semantic kinds (FR-010f):
  - **current-state facts:** `exists` and `incoming`, properties of the evaluated state;
  - **history facts:** `used`, a property of the store's history.

  Two positions with the same state identity can differ in `used`, which is why the deterministic
  input is an **evaluation snapshot**: the state plus the observed current-state and history
  facts, named by `data_version` (store, state, position). The facts:
  - `exists(T, id) → bool`, at the evaluated state;
  - `used(T, id) → bool`: has the store ever used this identity (FR-009);
  - `incoming(T, id) → [(entity, id, field)]`: the surviving `Ref` fields pointing at this
    identity at the evaluated state.

  Every answer that evaluation actually obtains is an **observed fact**: an `ExistenceRead`,
  `IdentityRead` or `ReferenceRead`. Observed facts go into the decision record's `facts` section
  and into the read set (FR-010c, FR-018). Facts are obtained lazily, so short-circuited checks are
  never asked and never recorded.

  Three `EvaluationFacts` providers:
  1. **Plain `evaluate`:** facts supplied in the request's new `facts` section. A needed fact that is
     missing is the evaluation error `UNKNOWN_FACT`, deterministic and never a guess. Supplied
     facts are first checked to form a valid snapshot, or the result is `INCONSISTENT_FACTS`
     (FR-010g):
     - `exists → used`;
     - every incoming reference's source is a bound or supplied existing entity, and its target
       exists;
     - bound entities exist and are used;
     - no fact is duplicated or contradictory.
  2. **`Store::evaluate`:** answers from the backend as of the evaluated position (`exists_at`,
     `used_at`, `incoming_at`), the same snapshot as `version_at`.
  3. **Replay:** answers from the recorded facts. A fact the replay needs but the record lacks is a
     `decision` divergence.
- **Rationale**:
  - The principle "state is observed through facts": field reads, existence reads, identity reads
    and reference reads are all recorded state facts, so replay needs no live store.
  - The same oracle design serves three contexts without special cases.
- **Alternatives**:
  - The host supplies the whole universe: impossible at scale, and it violates "only observed
    facts".
  - The evaluator queries the store directly: couples core to storage, and breaks plain evaluation
    and replay.

## R4. Where facts are evaluated (S versus S')

- **Decision**: the evaluator asks the oracle about the *evaluated* state S only. Facts about the
  resulting state S' are computed deterministically from S's facts plus ΔS:
  - `exists'(x) = (exists(x) ∨ created(x)) ∧ ¬removed(x)`;
  - `incoming'(x)` = `incoming(x)`, minus references whose source is removed or whose field is
    retargeted in ΔS, plus references from created entities and retargeted fields.

  `exists` in postconditions and outgoing rules uses `exists'`. The referential-integrity check
  (R6) uses `incoming'`.
- **Rationale**: FR-006, FR-010e. S' never exists in the store before commit, so the only honest
  source of its facts is S plus ΔS. The oracle is queried at one position.
- **Alternatives**: materializing S' in the backend. That writes before the decision, and breaks
  atomicity.

## R5. Creation semantics and collisions

- **Decision**:
  - **Collisions:** a creation reads `used(T, id)` (an observed identity read). If it is true, the
    decision is `ENTITY_ID_ALREADY_USED` with the offending identity (FR-007). The commit re-checks
    the registry at the parent (R9).
  - **Checks on the new entity:** its entity constraints, including synthesized `Ref` existence
    constraints, are checked on S' like any outgoing constraint. State invariants of `T` apply
    to it on S'.
  - **Writes:** the new entity's existence and all its fields are in the write set (FR-018).
- **Rationale**: FR-007, FR-010, FR-006.
- **Alternatives**: checking collisions only at commit. The decision itself must say
  `ENTITY_ID_ALREADY_USED`, so evaluation must observe the registry.

## R6. Removal and referential integrity

- **Decision**:
  - **Validity:** `remove(p)` is valid only if `p`'s entity exists at S, which holds by binding.
  - **The integrity check on S'**, after all effects:
    1. For each removed identity x, compute `incoming'(x)`. It must be empty, or the decision is
       `DANGLING_REFERENCE` with the surviving references.
    2. For each created entity and each retargeted `Ref` field, the target must satisfy `exists'`
       (the synthesized constraint).

    Only surviving `Ref` fields count, and plain `Id<T>` fields never do.
  - **Writes:** a removal writes the existence of x (FR-018). Reading `incoming(x)` is an observed
    reference read (FR-010e).
- **Rationale**: FR-008, FR-010b, FR-010e. Retargeting and removing in one transition work, because
  the check runs on the resulting reference graph.
- **Alternatives**: checking against the index at S. Rejected by the clarification, because it
  refuses valid transitions.

## R7. Decision records

- **Decision**: the record format gains optional sections, emitted only when non-empty:
  - `lifecycle`: `[{op: "create", entity, id, value}, {op: "remove", entity, id, value}]`, where a
    removal carries the removed entity's last value;
  - `facts`: `{existence: [{entity, id, exists}], identities: [{entity, id, used}], references:
    [{entity, id, incoming: [{entity, id, field}]}]}`, sorted canonically.

  A record with either section has `record_version "0.5"`; all others stay `"0.4"`, byte for byte.
  The request gains an optional `facts` section in the same shape. It is echoed into the record
  only as observed facts, never wholesale.
- **Rationale**: SC-006. Existing goldens are unchanged, and lifecycle records are
  self-contained for replay.
- **Alternatives**: always 0.5. It changes every golden for no semantic reason.

## R8. Wire IR and identity of existing modules

- **Decision**:
  - **Version:** **any** use of a semantic form introduced by feature 006 requires wire IR `0.5`:
    `create` and `remove` effects, `exists`, `referenced`, `{"t":"ref","entity":T}`, and facts
    sections. This holds even if no action ever creates or removes anything, so the version
    boundary is mechanical.
  - **Compatibility:** admission accepts `0.4` and `0.5`. A 0.4 document with a lifecycle form is
    `UNSUPPORTED_IR_VERSION`. Serialization writes `0.5` only when a lifecycle form is used, like
    feature 003's selective version.
  - **Hashing:** new hash codes for the new nodes and for the reference flag of an `Id` field.
    Modules without lifecycle forms hash exactly as before (frozen identities, SC-006).
- **Rationale**: this is a pure extension. Nothing existing changes meaning, so unlike 004 there is
  no reason to reject 0.4.
- **Alternatives**:
  - 0.5 only: would force a migration with no semantic change.
  - No version step: 0.4 readers would silently misread new forms.

## R9. Persistence: universe, registry, and derived reference index

- **Decision**:
  - **Backend primitives (3 new, 10 in total):**
    - `removed_at(key) → Option<position>`: a removal is recorded once and immutable;
    - `incoming_at(target, position) → [RefEdge]`: the derived reverse-reference index, as of a
      position;
    - `used_at(key, position)`: derived from `version_at(key, position).is_some()`, so it needs no
      new method in practice. It is kept in the contract for clarity, with a default
      implementation.

    `exists_at(key, p) = version_at(key, p).is_some() ∧ removed_at(key) ∉ (−∞, p]`.
  - **The registry key** is the entity type's stable nominal name plus the id, never its
    declaration hash, so a schema change cannot make an old identity reusable.
  - **The index** is stored as immutable edge events: `{target, source, field, added_at,
    dropped_at?}`. A later commit may set `dropped_at` once. `incoming_at` folds the events at a
    position. The index is derived: it does not enter the state identity, and conformance checks it
    against a reconstruction from entity content (case `reference_index_consistency`).
  - **The atomic commit** also writes the removals and the index edge changes, in the same
    compare-and-set as versions, record and head (FR-010e, FR-014).
  - **Genesis:** seed `Ref` fields must point at seed entities. The genesis builds the initial index,
    and the registry starts as the seed identities.
  - **State identity:** a creation inserts the new content hash; a removal removes the last content
    hash (FR-011). The cost is proportional to the changes.
  - **Transition records** gain optional `created: [EntityVersion]`, `removed: [{entity, id,
    last_revision, last_content_hash}]` and `ref_changes: [RefEdgeChange]`, omitted when empty.
    Feature-005 documents therefore hash unchanged, and the vectors stay frozen.
- **Rationale**:
  - FR-009 to FR-015.
  - As-of index reads keep the consistent-snapshot guarantee of 005 for reference facts.
  - Immutable events fit the "versions never change" storage model; a set-once `dropped_at` is the
    only mutation, done inside the atomic commit.
- **Alternatives**:
  - A current-only index: cannot answer at the evaluated position when commits land meanwhile, which
    breaks FR-018 / FR-010e.
  - Hashing the index into the state: two representations of one truth (clarification).

## R10. Commit validation additions

- **Decision**: after the 005 checks, in order:
  1. Re-derive lifecycle effects and facts by re-evaluating the record, with the store as oracle at
     the parent.
  2. The record's `facts` section must equal the facts observed now at the parent. Any difference is
     `BUNDLE_INVALID`.
  3. Created identities must be unused at the parent (`ENTITY_ID_ALREADY_USED`).
  4. Removed entities must exist at the parent.
  5. Referential integrity on S' (R6), with `incoming_at` at the parent (`DANGLING_REFERENCE`).
  6. Build the new versions (created entities at revision 1), removals, index edge changes and the
     accumulator.
  7. Commit everything in one compare-and-set.
- **Rationale**: FR-020 of 005 extended to facts: the engine derives, the store checks, and nothing
  host-supplied is trusted.
- **Alternatives**: trusting the bundle's facts. Rejected (engine-derived dependencies).

## R11. Replay

- **Decision**:
  - **Data replay** applies `created`, `removed` and `ref_changes` with the write sets. It checks:
    - no identity is created twice across the history (the registry);
    - removed entities existed;
    - a replay-built index equals the backend's `incoming_at` at each checked position (sampled at
      the end, and at every position where the history changes references);
    - every state identity.
  - **Behavior replay** re-evaluates with the recorded facts as oracle, then checks the recorded
    facts against the store's facts at the parent.
  - **Divergence kinds** gain `lifecycle` and `references`.
- **Rationale**: FR-013, FR-015, SC-002, SC-007.

## R12. Verification (SMT)

- **Decision**: per entity type T, three uninterpreted functions over identities at S:
  - `ex_T` (exists);
  - `used_T`, with the axiom `ex_T(x) → used_T(x)`;
  - `refd_T`: "some entity **not bound by the action** holds a `Ref` to x".

  Bound entities are symbolic records whose existence is true. The functions carry the
  relations the runtime guarantees, so the solver does not propose impossible worlds that the
  runtime would downgrade to inconclusive:
  - `ex_T(x) → used_T(x)`;
  - bound identities satisfy `ex_T` and `used_T`;
  - `refd_T(x) → ex_T(x)` in a valid S, since surviving references point at existing entities.
  - **Creation** of `T#i` with values v: assume `¬used_T(i)` and that `i` differs from the bound
    identities of T. Check T's constraints (including `Ref` existence via `ex'`) and invariants on
    v, and postconditions on S'.
  - **Removal** of bound `p`: `ex'_T(p.id) = false`. Integrity requires `¬refd_T(p.id)` and no
    surviving bound or created entity whose `Ref` field equals `p.id`.
  - **Predicates:** `exists(e)` is `ex_T(e)` on S and `ex'` on S'. `referenced(e)` on S is
    `refd_T(e)` ∨ (some bound entity's `Ref` field = e).
  - **New check kinds:** `referential_integrity` (blocking) and `identity_collision_unreachable`
    (proven by construction; reported for completeness).
  - **Counterexamples** set the uninterpreted functions to concrete facts, which become the plain
    request's `facts` section. The runtime confirms them without a store (FR-017).
- **Rationale**: FR-016, FR-017. The verifier models the semantic invariant "no surviving `Ref` to
  an absent entity", not the index. Uninterpreted functions are exact for quantifier-free facts
  about finitely many identities.
- **Alternatives**: quantifying over all entities of T. Undecidable in general, and slower;
  uninterpreted `refd_T` is sound because the runtime checks exactly "any unbound referrer".

## R13. Read and write sets

- **Decision**:
  - **Read set:** gains `existence: [{entity, id, exists}]`, `identities: [{entity, id, used}]` and
    `references: [{entity, id, incoming}]`, equal to the record's observed facts.
  - **Write set:** gains `lifecycle: [{op, entity, id}]`. A creation writes existence and every
    field; a removal writes existence.

  These fields are optional in the bundle and omitted when empty, so 005 bundles hash unchanged.
- **Rationale**: FR-018. A future entity-level rule can detect concurrent creations of the same
  identity (an identity read `used=false` versus a later creation), and removal/update races.

## R14. Python and examples

- **Decision**:
  - **DSL:** `create(Account, id=account_id, owner=…, …)`, `remove(account)`, `exists(order.customer)`,
    `referenced(customer)` and `Ref[Customer]`.
  - **Plain evaluation:** `evaluate(..., facts={...})`.
  - **Store:** its Python binding covers lifecycle unchanged, since facts come from the backend. A
    Python backend adds `removed_at` and `incoming_at` (and optionally `used_at`), and `commit`
    receives `removals` and `ref_changes`.
  - **Example:** `examples/accounts/` has customers, accounts (`owner: Ref[Customer]`), open,
    deposit, close and `remove_customer` (guarded by `not referenced`).
- **Rationale**: SC-001, SC-005.
