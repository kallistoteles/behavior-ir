# The persistence contract (features 005–007)

> **The engine defines the canonical state-transition and persistence contract; hosts choose how
> that contract is stored.**

The engine defines what must be preserved for behavior to be reproducible. It never opens files or
connections, and it does not dictate how anything is stored. Details: `specs/005-persistence-contract/`
(spec, research R1–R12, data model, `contracts/store-api.md`, implementation review).

## Guiding invariant

> Every committed transition is reproducible from an immutable semantic state, an exact history
> position, an exact behavior version, exact input and exact context. No storage implementation
> may alter those semantics.

Three concepts are kept apart:

| Concept | Question | Represented by |
|---|---|---|
| State identity | What information exists? | A content hash (MuHash3072) over entity content = type + declaration + id + value. Revisions and positions are **not** content |
| History position | Where in this store's history? | A monotonic position plus the hash chain of transition records from the genesis |
| Entity version | Which historical incarnation supplied a value? | Revision plus the position that created it |

Equal content gives an equal state identity, even at different positions: position 42 and position
57 may both be state `ABC`.

## Pipeline

```text
Store (engine semantics) over a Backend (host storage)
        │  read the head once (state S at position n), every bound entity *as of n*
        ▼
S + input + context + facts as of n ──► evaluator ──► decision + ΔS + observed reads and facts
        │  only ALLOW produces a commit bundle, bound to store and state:
        │  data_version = store:<genesis>;state:<S>;position:<n>
        ▼
commit(expected_parent = S, bundle)
   1. idempotency: a transition evaluated at n can only be record n+1
   2. whole-state concurrency: S must still be the head, otherwise STATE_CONFLICT
   3. validation: the record reproduces; read and write sets are re-derived by the engine;
      revisions, old values, declarations and facts match; creations use unused identities,
      removals exist, and no reference to a removed entity survives (feature 006)
   4. evidence policy (cited by every record)
   5. the backend's atomic compare-and-set: versions, removals, index changes, record and head
      as one unit
        ▼
new state identity + transition record (evaluated_against == committed_on) + new head
```

## What a host implements

A `Backend` has twelve methods: `genesis`, `head`, `create(genesis, head, seed, seed_refs)`,
`version_at(key, position)`, `version(key, revision)`, `record(position)`, `removed_at(key)`,
`incoming_at(target, position)`, `used_at(key, position)` (with a default), `keys_at(entity_type,
position)` and `keys_by_field_at(entity_type, field, value, position)` (optional, feature 007),
and `commit(expected_last_record, versions, removals, ref_changes, record, new_head)`. Only
`create` and `commit` write.
- **The compare-and-set:** `commit` is the only concurrency primitive, and it must be atomic and
  crash-safe.
- **As-of reads:** stored versions never change and are tagged with the position that created them,
  so reads at one position form a consistent snapshot, even while commits land.
- **Everything else is the engine's:** hosts never re-implement validation, hashing, conflicts or
  replay.

`InMemoryBackend` is the reference backend. `behavior_store::conformance::run(factory)` (Python:
`run_conformance(factory)`) runs 28 named cases against any backend. Faults and interleavings are
injected around the backend, so no hooks are needed. Twelve deliberately broken backends each fail
their designated case, and a backend listing keys in reverse order passes every case.

## Replay

- **Data replay:** re-applies each write set, recomputes every entity version and state identity,
  and checks the record chain, the parents, and `evaluated_against == committed_on`.
- **Behavior replay:** re-evaluates every transition under its recorded behavior version, and checks
  the decision, the changes, the observed reads and the observed facts.
- **Reference replay** (`replay_index(store, module, …)`, feature 006): rebuilds the reverse-reference
  index from entity content and checks it against the backend's `incoming_at`, and each record's
  `ref_changes` against the content changes. It needs the module, because `Ref` fields are part of
  the entity declarations.

All three report the first divergence: its position and kind (`state`, `parent`, `chain`, `record`,
`changes`, `decision`, `invariant`, and since 006 `lifecycle` and `references`). Snapshots are a
cache: `verify_snapshot` confirms them, and they never override the history.

## Universe transitions (feature 006)

> Identity is permanent; existence is state.

The entity universe is part of state. Actions create entities (`create(T, id, {complete value})`)
and remove them (`remove(p)`) as ordinary effects, evaluated and committed atomically with field
updates. Details: `specs/006-entity-lifecycle/`.

- **Identities are host-supplied inputs;** the engine never generates them.
- **The identity registry:** an identity is *used* once any version of it exists, keyed by the
  entity type's name and the id (never the declaration hash). A removed identity is never created
  again: `ENTITY_ID_ALREADY_USED`.
- **Removal is logical:** `removed_at(key)` is set once, inside the commit; versions are kept, so
  `load` at earlier states and both replays still see the removed entity. A removal removes the
  last content hash from the state accumulator; a creation inserts the new one (revision 1).
- **Referential integrity is explicit:** `Ref<T>` is `Id<T>` plus the constraint `exists(field)`.
  Plain `Id<T>` fields never block anything. Integrity is checked on the resulting state S', so
  retarget-and-remove in one transition is valid, and a removal with a surviving reference is
  `DANGLING_REFERENCE`.
- **The derived reverse-reference index** answers `incoming_at(target, position)`. It is stored as
  edge events (`added_at`, a set-once `dropped_at`), written in the same atomic commit, never part
  of the state identity, and always reconstructible from entity content (conformance case
  `reference_index_consistency`, `replay_index`).

**The evaluation snapshot.** Evaluation observes *facts*: existence and incoming references
(current state) and whether an identity was used (history). Two positions with the same state
identity can differ in history facts, so the deterministic input is the state *plus* the observed
facts, named by `data_version` (store, state, position). The store answers facts as of the
evaluated position; only facts actually observed are recorded, in the decision record's `facts`
section and the bundle's `read_facts`; the commit re-derives them at the parent and refuses a
bundle whose facts differ. Plain evaluation takes facts from the request (`UNKNOWN_FACT` if one is
missing, `INCONSISTENT_FACTS` if they cannot describe one valid state); replay uses the recorded
facts.

**Trust boundary.** A recorded existence or reference fact is *what the store answered* at that
position, checked again at commit and by replay against the same store. It is **not** a standalone
cryptographic proof: MuHash commits to the state's content, but a fact about absence or about
incoming references cannot be verified without the store (no membership or non-membership proofs).
A reader who does not trust the store must replay against it.

## Relational queries (feature 007)

> Sets are semantic; indexes are implementation.

A query (`select(T)`, `where`, set algebra, and `count`/`any`/`all`/`sum`/`min`/`max`/`unique`
over it) is a pure read of the evaluated state. Details: `specs/007-relational-queries/`.

- **Query facts:** a query *instance* is its definition hash plus its canonical captured values
  (`QueryInstanceId`); a query fact records the instance's complete membership, and member values
  are ordinary field facts. Only what evaluation actually read is recorded, in the record's
  `facts.queries` / `facts.fields` (record version 0.6) and the bundle's `read_facts`, where each
  query also carries a `result_hash`. Records list full memberships even for `count`, so large
  results make large records; this is accepted for now. Overlapping fact representations must
  agree: if a request supplies a complete `universe` and also supplies redundant query or field
  facts for the same state, the engine derives from the universe and rejects contradictions as
  `INCONSISTENT_FACTS`.
- **The evaluation snapshot:** the store answers memberships and member values as of the
  evaluated position, like every other read. `keys_at(type, position)` is the derived type index;
  `keys_by_field_at` an optional field index. The store follows an index plan from the query's
  equalities (a captured value preferred over a literal), then re-checks every candidate, so an
  index can only save work: it never defines a result and is never part of the state identity.
  Conformance: `query_snapshot`, `query_index_consistency`, `query_order_independence`,
  `module_invariant_preserved`.
- **Resulting state:** `Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS`. Filters are
  candidate-local, so only created, removed and changed bound candidates are re-classified; the
  baseline is an observed fact against S. Nothing speculative is ever written.
- **Module invariants** hold at genesis (`GENESIS_INVALID`) and on every resulting state they can
  be affected by (`INVARIANT_VIOLATED`); a sound dependency analysis skips only unaffected ones,
  and `unique` is decided by a delta rule (only touched members can collide).
- **Replay** never queries a store: plain replay uses the recorded facts; behavior replay
  re-derives them at each parent and compares, result hashes included.

**Trust boundary.** As with existence facts, a recorded membership is what the store answered;
it is not a standalone (non-)membership proof.

## Evidence

Every store has a content-addressed evidence policy, fixed in its genesis. It requires either no
governance evidence or a commit authorization matching the transition, with an optional allowlist
of execution policies. Every record cites the policy hash and any authorization hash.

**Trust boundary.** `require: commit_authorization` is a *structural* guarantee:
- A well-formed authorization, matching this exact transition, store and state, is bound to the
  commit and cited in history.
- It is **not** yet cryptographic proof of who issued it. Commit results report
  `evidence_trust: "structural"`.

This ties into the open feature-002 risks (unsigned attestations, trusted cache).

## Scope and follow-ups

- **Concurrency:** it is whole-state in 005. Records already carry observed read sets, write sets,
  and `evaluated_against` / `committed_on`, so a later entity-level rule can accept a transition
  evaluated at S42 onto S45 when nothing it observed or wrote changed, without reinterpreting old
  records.
- **Entity universe:** since feature 006, creations and removals are recorded transitions. Bulk
  import, identity reuse and garbage collection of history remain out of scope.
- **Verifiable facts:** proofs of (non-)membership or incoming references (for example an
  authenticated index) would let a reader check recorded facts without the store.
- **Signed authorizations** with trusted keys in the evidence policy would raise evidence trust from
  structural to cryptographic (`evidence_trust: "cryptographic"`).
- **Resolve behavior by content hash:** `store.commit(bundle)` with a behavior resolver
  (`behavior_hash → Module`), instead of the host passing the module. Today the module is a
  parameter, and `commit` checks first that
  `hash(module) == bundle.behavior_version == record.behavior_version`.
- **Splitting out `behavior-evidence`:** the governance documents could move to their own crate if
  the verifier ever gains heavy dependencies. Today it links no solver.
- **Changing a store's evidence policy:** the choice among a new store, a configuration transition
  or a versioned policy is left open. Records cite the exact policy, so history stays unambiguous.
