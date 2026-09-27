# The persistence contract (feature 005)

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
S + input + context ──► evaluator ──► decision + ΔS + observed read set / write set
        │  only ALLOW produces a commit bundle, bound to store and state:
        │  data_version = store:<genesis>;state:<S>;position:<n>
        ▼
commit(expected_parent = S, bundle)
   1. idempotency: a transition evaluated at n can only be record n+1
   2. whole-state concurrency: S must still be the head, otherwise STATE_CONFLICT
   3. validation: the record reproduces; read and write sets are re-derived by the engine;
      revisions, old values and declarations match; the entity universe is unchanged
   4. evidence policy (cited by every record)
   5. the backend's atomic compare-and-set: versions + record + head as one unit
        ▼
new state identity + transition record (evaluated_against == committed_on) + new head
```

## What a host implements

A `Backend` has seven methods: `genesis`, `head`, `create`, `version_at(key, position)`,
`version(key, revision)`, `record(position)`, and `commit(expected_last_record, versions, record,
new_head)`.
- **The compare-and-set:** `commit` is the only concurrency primitive, and it must be atomic and
  crash-safe.
- **As-of reads:** stored versions never change and are tagged with the position that created them,
  so reads at one position form a consistent snapshot, even while commits land.
- **Everything else is the engine's:** hosts never re-implement validation, hashing, conflicts or
  replay.

`InMemoryBackend` is the reference backend. `behavior_store::conformance::run(factory)` (Python:
`run_conformance(factory)`) runs 17 named cases against any backend. Faults and interleavings are
injected around the backend, so no hooks are needed. Six deliberately broken backends each fail
their designated case.

## Replay

- **Data replay:** re-applies each write set, recomputes every entity version and state identity,
  and checks the record chain, the parents, and `evaluated_against == committed_on`.
- **Behavior replay:** re-evaluates every transition under its recorded behavior version, and checks
  the decision, the changes and the observed reads.

Both report the first divergence: its position and kind (`state`, `parent`, `chain`, `record`,
`changes`, `decision`, `invariant`). Snapshots are a cache: `verify_snapshot` confirms them, and
they never override the history.

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
- **Fixed entity universe:** no entity is created or removed after the genesis. Import, create and
  delete belong in later *universe transitions* recorded in history, not backend CRUD.
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
