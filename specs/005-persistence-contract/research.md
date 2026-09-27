# Research: Persistence Contract

Decisions for feature 005. The input is `docs/proposals/persistence-contract.md`, and the
clarifications are in `spec.md` (whole-state concurrency; an explicit evidence policy per store).

## R1. Where the contract lives

- **Decision**: a new crate, `behavior-store`, that depends on `behavior-core` (evaluation, replay,
  canonical JSON) and `behavior-verify` (transition hash, authorization decoding). The Python
  binding exposes it. It contains:
  - the canonical documents;
  - the state identity;
  - the store logic over a host backend;
  - the in-memory reference backend;
  - the conformance suite.
- **Rationale**: persistence is a new layer ("Behavior IR → evaluator → ΔS → commit bundle →
  persistence contract → host"). It needs both evaluation and governance hashing. In
  `behavior-core` it would pull governance in. In `behavior-verify` it would mix storage with SMT.
- **Dependency check**: `behavior-verify` does not link a solver. Z3 is an external process,
  started only by `verify()`, and the crate's dependencies are `sha2`, `ed25519-dalek` and serde.
  The store therefore does not pull the verifier in to check an authorization. If the verifier
  ever gains heavy dependencies, the evidence documents move to a small `behavior-evidence`
  crate:
  - Attestation, Waiver, ExecutionPolicy and CommitAuthorization;
  - their canonical hashing.

  Both `behavior-verify` and `behavior-store` would then depend on it. This is a follow-up, not
  part of 005.
- **Alternatives**:
  - A module in `behavior-core`: this inverts the dependency on governance documents.
  - A module in `behavior-verify`: the SMT crate is the wrong home for storage semantics.

## R2. Engine-owned store logic over a small host backend

- **Decision**: split the proposal's `StateStore` into two layers.
  1. **`Store<B>`**, engine code, owns all semantics:
     - building commit bundles;
     - validating a bundle against the parent state;
     - computing new entity versions and the new state identity;
     - the evidence policy;
     - conflicts and idempotency;
     - history queries;
     - replay.
  2. **`Backend`**, host code, owns only persistence primitives:
     - read the genesis document and the head;
     - read an entity **as of a position**: its newest version created at or before that
       position (versions are immutable and tagged with the position that created them);
     - read a version by revision;
     - read transition records by position;
     - one atomic compare-and-set commit. It writes the new entity versions, the transition record
       (which carries the idempotency information) and the new head (accumulator + state
       reference) as one unit, and only if the head is still the expected one.

  The proposal's four operations (current, load, commit, transitions) are the public operations of
  `Store`.
- **Rationale**:
  - FR-003 requires every store to reproduce the same identities, and SC-003 requires conforming
    stores to behave alike.
  - If each host re-implemented validation and hashing, conformance would test re-implementations
    of the engine.
  - With the split, a host writes about six storage functions, and every semantic rule has exactly
    one implementation.
  - The atomic compare-and-set is the only concurrency primitive a host must provide, and every
    database offers one (transaction + head check, conditional write, or file lock + rename).
- **Snapshot consistency (FR-018)**: evaluation reads the head once (state S at position n), then
  reads every bound entity *as of n*. Versions are immutable and position-tagged, and a commit
  becomes visible only through the atomic head move. So every read comes from exactly S, even if
  commits land meanwhile, and the expected parent is S. Reading "latest" versions one by one
  could mix S42 and S43 into a state that never existed; the as-of read rules that out without a
  separate snapshot object. The same primitive serves `load(key, at)` for past states.
- **Crash atomicity (FR-006)**: the compare-and-set covers versions, record, accumulator, head
  and the idempotency information (in the record) together. Conformance injects crashes at the
  wrapper boundary, which works for **any** backend without hooks:
  - an error before calling the inner `commit`;
  - an error after the inner commit returned (a durable write with a lost acknowledgement).

  Atomicity *inside* a backend's commit (no partial writes) remains the host's responsibility. It
  is documented, and tested for the reference backend with the `PartialWrites` mutant.

  It then checks that the store shows all or nothing, and that a retry is recognized as already
  committed.
- **Alternatives**:
  - A host implements the full `StateStore`: semantics would be duplicated per host and tested
    only by the conformance suite.
  - Snapshot handles from the backend: equivalent, but every host must implement transactional
    read views. As-of reads over immutable versions need only ordinary reads.
  - The engine does I/O itself: this violates "the engine never opens files or connections"
    (spec Assumptions).

## R3. State identity: an incremental, order-independent multiset hash

- **Decision**: `StateId = H_state(MuHash3072({c(e) | e ∈ current entities}))`.
  - Each element `c(e)` is the entity's **content hash**: the domain-tagged hash of
    `{entity, declaration, id, value}`. It binds the type name, the declaration hash, the id and
    the canonical value, and **not** the revision or any position.
    - Values swapped between ids (or between types with equal field shapes) give different
      elements.
    - The same raw values under another declaration give a different element.
    - Two states with equal semantic content have equal identities, however they were reached.
      Returning to an earlier content (position 42 and position 57) gives the same identity; the
      position tells the histories apart.
  - The state identity is pure content identity: two stores with identical content have equal
    state identities. Store identity is kept separate and bound in the bundle and record (R6).
  - It is expanded to 3072 bits with SHA-256 in counter mode (12 blocks), reduced modulo
    `p = 2^3072 − 1103717`, and multiplied into the accumulator.
  - The empty state is the accumulator 1.
  - A commit multiplies by the new content hashes and by the inverses of the replaced ones.
    The head keeps the accumulator as a numerator/denominator pair, and one modular inverse (with
    `num-bigint`) normalizes it for the identity.
- **Rationale**:
  - FR-002 requires the identity to depend only on content, so equal states have equal identities
    whatever the history.
  - SC-004 requires commit cost proportional to the touched entities, not to the store size.
  - A multiset hash gives both. The accumulator is one value in the head, so a host stores it
    without understanding it.
  - MuHash3072 with this prime is established practice for content hashes of large sets, for
    example the UTXO set hash in Bitcoin Core.
  - No new dependency: `sha2` and `num-bigint` are already in the workspace.
- **Alternatives**:
  - **Sparse Merkle tree over entity keys**: content-canonical and incremental, and it supports
    membership proofs. But every host must store tree nodes and the engine must drive node I/O.
    It is kept as a possible later upgrade if membership proofs are needed; the state document
    would then change its tag.
  - **Hash chain (parent id + delta)**: history-dependent, which violates FR-002.
  - **Hash of the full sorted state per commit**: O(state size), which violates SC-004.
  - **Additive hash mod 2^256**: insecure (generalized-birthday attacks).

## R4. Entity versions and revisions

- **Decision**:
  - Content and history are kept apart.
    - The **entity content** is `{entity, declaration, id, value}`, where `declaration` is the
      entity's semantic declaration hash. Its hash is the content hash that enters the state
      identity (R3).
    - The **entity version** is `{content_hash, entity, id, revision, created_at, value}`, one
      historical incarnation of the entity: the content hash, plus the revision and the position
      that created it. It is addressed by `(key, revision)` in history.
    - `entity` is the entity type name.
    - `value` is the canonical field map, including `id`, encoded as in decision records (for
      example fixed-scale text).
    - `revision` starts at 1.
  - A committed transition creates a new version, with revision + 1, exactly for the entities whose
    canonical value changes.
  - An effect that writes an equal value is in the write set, but it does not create a version.
  - The "previous revision" of the proposal is always `revision − 1`, so it is derived, not stored.
- **Rationale**:
  - Revisions count real changes.
  - Revisions are history, not content. With revisions in the state hash, an account at revision 3
    with balance 100 and the same account at revision 9 with balance 100 would be different
    states despite identical information.
  - An allowed transition with no effective change leaves the state identity unchanged, as the
    spec's "transition that changes nothing" edge case requires.
- **Alternatives**: bumping the revision on every write. Equal-content states would then differ,
  and identities would count no-op writes.

## R5. Entity types across behavior versions

- **Decision**:
  - The store's genesis document fixes the entity declarations: name → the entity's semantic
    declaration hash from the module's name table.
  - Every bundle states the declaration hashes of the entities it touches.
  - A commit whose behavior version declares any touched entity differently is refused with
    `ENTITY_DECLARATION_MISMATCH`.
  - Behavior versions with identical entity declarations can alternate in one history.
- **Rationale**: this is the edge case "behavior version changes over existing data". Data
  migration is out of scope, and it is refused rather than guessed.
- **Alternatives**: keying state by declaration hash. A harmless rename of an unrelated item would
  then split the state.

## R6. Evaluating against a store: bindings and the decision record

- **Decision**: `Store::evaluate(module, action, bindings, input, context, commit_time)`.
  1. `bindings` maps each **state** parameter to an entity id.
  2. The engine loads those entities at the store's current state and builds the standard request.
     The request's `state` section comes from the store. `input` and `context` are host-supplied,
     as today.
  3. It sets `data_version` to `store:<genesis hash>;state:<state id>;position:<n>` and evaluates
     with the existing evaluator. This canonical string binds both the store and the state, as
     FR-019 requires.
  4. It returns the decision record plus, only if the result is `ALLOW`, a commit bundle.
- **Rationale**:
  - The existing record format, replay and authorization stay unchanged (record_version 0.4).
  - `data_version` binds the record, and therefore the authorization's transition hash, to the
    store and the evaluated state. Evidence cannot be reused for another parent, or for another
    store with identical content and behavior.
  - **Context snapshot (FR-022)**: the decision record already carries the full canonical
    context. For example `context.actor` holds every field of the Employee as it was supplied,
    not only an id. Behavior replay therefore never depends on today's context data, and nothing
    changes here except making this a tested requirement.
  - Context entities (read-only actors) remain host-supplied: they are not state and are
    recorded in the bundle.
- **Alternatives**:
  - Store-held context entities: this needs a second binding kind and a policy on who may change
    actors. It is deferred.

## R7. The commit bundle, read set and write set

- **Decision**: a `behavior.commit_bundle.v1` document containing:
  - `evaluated_state`: a state reference, i.e. identity and position;
  - `behavior_version`;
  - `record`: the full decision record;
  - `transition_hash` of the record (the same hash as in the commit authorization);
  - `entity_declarations` for the touched entities;
  - `read_set`: the entity fields **actually observed** by this evaluation, grouped by entity with
    the entity's revision at the evaluated state. It includes reads inside derived values, rules,
    invariants and constraints. Operands skipped by short-circuiting are excluded: in
    `account.active and account.balance > amount` with `active = false`, `balance` is not read. A
    static superset of possible reads would be a separate `dependency_set`, which is not recorded
    in 005;
  - `write_set`: the `{entity, id, field}` cells written, with old and new values;
  - `commit_time`;
  - `evidence`: optional, see R9.

  In this feature, conflicts are whole-state (FR-007), and the read set is recorded for
  diagnostics and for the future entity-level rule. An observed read set is more precise than a
  syntactic one: a later rule could accept a transition evaluated at S42 onto S45 when every
  entity changed since then is outside its observed read and write sets.

  **Engine change (behavior-core)**: today the trace records reads per step, but a derived value's
  body is evaluated without a read collector, and a memo hit reports only the derived value. The
  evaluator gains an observed-read collector for the whole evaluation. It records every
  `param.field` actually evaluated, including inside derived bodies, and a memoized derived value
  keeps the reads of its first evaluation and attributes them again on every hit. Decision records
  and their bytes are unchanged; the collector is exposed through a new API used by the store.

  **Engine-derived dependencies (FR-020)**: the read set and write set are computed by the engine
  from the evaluation, never taken from the host. At commit, the store re-derives them from the
  bundle's decision record: re-evaluating it with the observed-read collector gives the read
  set, and `changes` gives the write set. It refuses a bundle whose sets differ (`BUNDLE_INVALID`). A write to an
  entity absent from the parent is refused (FR-021).
- **Rationale**:
  - The bundle is self-contained: a store validates it against the parent state without the
    module.
  - Read and write dependencies are recorded now, so an entity-level rule can be added later
    without changing the meaning of old records (FR-007a).
- **Alternatives**:
  - Not recording read sets: a later entity-level rule could not be applied to history.
  - Entity-level reads only: field-level observed reads are more precise for a future rule, and
    the entity revision is recorded alongside.
  - A syntactic (static) read set: less precise under short-circuiting. If needed later, it would
    be a separate `dependency_set`.

## R8. Commit, conflicts and idempotency

- **Decision**: `Store::commit(expected_parent, bundle)`:
  1. **Idempotency**: a bundle evaluated at position p can only have been committed as record p+1.
     If `record(p+1)` exists and has the same *semantic transition*, return that record's result
     (FR-009), whatever the head is now. The semantic transition is the transition hash of the
     decision record (behavior, store and state binding, input, context, decision, changes),
     which already binds p through `data_version`.
     - A retry after a timeout with a new commit time or re-attached evidence is recognized.
     - A retry after other commits followed is recognized too. It must never become a conflict,
       because the host would re-evaluate and apply the transition twice.
     - This costs one record read (O(1)) and runs before the conflict check.
  2. **Consistency and conflict**: `expected_parent` must equal the bundle's `evaluated_state`
     (`BUNDLE_INVALID` otherwise). If it is not the head, return `STATE_CONFLICT`. It carries the head and the entities written since the parent, computed from
     the records after the parent.
  3. **Validation**:
     - the bundle's hashes;
     - the declarations (R5);
     - every read-set revision must equal the parent's revision;
     - every write-set `old` must equal the parent value;
     - the evidence policy (R9).
  4. **Build**: the new entity versions, the new accumulator and state identity, and the transition
     record. The record has `evaluated_against` = `committed_on` = the parent, `result_state`,
     `evidence_policy`, `authorization`, and `previous_record`.
  5. **Commit**: call the backend's compare-and-set. If the head moved meanwhile, report
     `STATE_CONFLICT`.

  Stores never apply a partial commit: the backend primitive is atomic, and conformance tests it
  with injected failures.
- **Identities kept apart**:
  - the *transition semantic hash* is the existing transition hash of the decision record. It
    excludes commit time and evidence;
  - the *bundle hash* adds commit time and evidence;
  - the *audit record hash*, the chain link, adds the result state and the chain.

  Audit metadata never changes the semantic identity of a transition.
- **Rationale**: this is the proposal's optimistic concurrency with the whole-state rule. The
  invariant `evaluated_against == committed_on` is checked when building the record and when
  replaying.
- **Alternatives**:
  - Entity-level rebasing: this was clarified away for this feature.
  - Locks held during evaluation: they put blocking concurrency into the contract, where optimistic
    retry is simpler.

## R9. The evidence policy

- **Decision**: a content-addressed `behavior.evidence_policy.v1` document, fixed in the genesis
  document. Its fields:
  - `require`: `"none"` or `"commit_authorization"`;
  - `trusted_execution_policies`: an optional allowlist of execution-policy hashes that an
    authorization may cite.

  Unknown fields are refused, so the format can be extended by later versions.

  When evidence is present (an authorization plus the documents it cites), the store checks:
  - that the authorization decodes and its `decision` is `allow`;
  - that its `transition_hash` equals the bundle's;
  - that its cited policy, attestation and waiver hashes match the supplied documents;
  - that the execution policy is allowlisted, if an allowlist is given.

  If the policy requires an authorization and none is supplied, the commit is refused with
  `EVIDENCE_REQUIRED`; mismatches are refused with `EVIDENCE_MISMATCH`. Every record cites the
  policy hash, and the authorization hash when one is used.

  **Trust boundary (FR-023)**: the store does not re-run verification or re-evaluate the execution
  policy, and a content hash shows only *what* an authorization says, not *who* may issue it.
  `require: commit_authorization` is therefore a **structural guarantee within the current trust
  boundary**: a well-formed authorization that matches this exact transition, store and state is
  bound to the commit and cited in history. It is **not** cryptographic end-to-end proof of
  authority.
  - This is stated in the policy documentation, and in the commit result as
    `evidence_trust: "structural"`.
  - It ties into the open feature-002 risks (unsigned attestations, trusted cache).
  - Closing it is a follow-up. One option is signed authorizations: an authorization hash, a
    signer key and a signature, with the evidence policy listing the trusted keys; a later policy
    version would add a `trusted_authorizers` field. The other option is deterministic
    re-evaluation of the execution policy inside the store.
- **Rationale**: every commit is valid relative to an explicit, cited rule, and no commit has an
  implicit policy.
- **Alternatives**:
  - A boolean flag: less explicit and not extensible (clarification).
  - Re-authorizing at commit: this duplicates `authorize` and needs clock and keys in the store.

## R10. History and replay

- **Decision**: transition records are chained by `previous_record` (a hash chain, so reordering or
  deletion is detected) and carry a `position`. `transitions(from, to)` returns records by
  position.

  **Data replay**:
  1. Start from the genesis state.
  2. For each record, check the parent (`committed_on` = the running state reference) and apply the
     write set.
  3. Recompute the versions and the accumulator, and compare `result_state`.

  **Behavior replay**:
  1. For each record, look up the module by `behavior_version` in a host-supplied module set.
  2. Check that the record's state section equals the entity versions at the parent.
  3. Run the existing `replay(module, record)`, and check that its changes equal the write set.

  Both report the first diverging position and a kind: `parent`, `state`, `record`,
  `changes`, `decision`, `chain`, `invariant`. A snapshot is verified by comparing its identity
  with the replayed identity at its position; it never overrides the history (FR-012).
- **Trust anchor**: a hash chain protects history only relative to a trusted genesis and head.
  The genesis hash is the store identity. The head moves only through the atomic compare-and-set,
  so replay from the genesis to the head, with the head's `last_record` checked, verifies the
  whole history the backend presents.
- **Rationale**:
  - The two replays of the proposal.
  - The hash chain covers the tampering cases of SC-002 that state identities alone cannot, such
    as a reordered no-op transition.
- **Alternatives**: identity chains only. A no-op transition's record could then be altered or
  moved undetected.

## R11. Genesis and seeding

- **Decision**: a store is created from a `behavior.store_genesis.v1` document containing:
  - the evidence policy;
  - the entity declarations;
  - the seed entity values (all at revision 1).

  The genesis state S0 is the state of the seed. The genesis hash is the store's identity and the
  root of the record chain.

  Adding entities after genesis, whether by actions or by host import, is out of scope (spec). The
  genesis document's format leaves room for a later recorded import transition.
- **Rationale**: history replay needs a defined, content-addressed starting point.
- **Alternatives**: an empty genesis plus imports. It needs the import record kind now; deferred.

## R12. Python and conformance

- **Decision**:
  - **Python API**: the binding exposes
    - `Store(backend)`, where `backend` is `InMemoryBackend()` or any Python object with the
      backend methods;
    - `store.evaluate(...)`, which returns an `Evaluation` with `decision` and `bundle`;
    - `store.commit(expected_parent, bundle)`, which returns a `CommitResult` or raises
      `StateConflict` / `CommitRefused`;
    - `store.current()`, `store.load(...)`, `store.transitions(...)`;
    - `replay_data(store)`, `replay_behavior(store, modules)`;
    - `run_conformance(backend_factory)`.
  - **Conformance**: `behavior_store::conformance` runs named cases against a backend factory.
    Broken reference backends that must fail are in the test suite:
    - one ignoring the head check;
    - one applying commits partially;
    - one reordering records;
    - one losing entity versions;
    - one serving newest versions regardless of position;
    - one moving the head before writing the record.
- **Rationale**: FR-015, FR-016, SC-003 and SC-006.
- **Alternatives**: conformance as documentation only. It is not checkable.
