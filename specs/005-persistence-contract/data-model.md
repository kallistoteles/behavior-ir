# Data Model: Persistence Contract

All documents are canonical JSON (sorted keys, no insignificant whitespace; `behavior_core::
canonical`). Each is identified by `sha256:` of the domain tag, a zero byte, and the canonical
text. This is the existing scheme (`behavior_verify::hashing::document_hash`), which drops a
top-level `hash` field before hashing.

| Document | Tag |
|---|---|
| Entity content | `behavior.entity_content.v1` (enters the state identity) |
| Entity version | `behavior.entity_version.v1` (history: content hash + revision + created_at) |
| State identity | `behavior.state.v1` (over the normalized accumulator, 384 bytes big-endian) |
| Evidence policy | `behavior.evidence_policy.v1` |
| Store genesis | `behavior.store_genesis.v1` |
| Commit bundle | `behavior.commit_bundle.v1` |
| Transition record | `behavior.transition_record.v1` |
| Decision record (unchanged, 0.4) | `behavior.transition.v1` (the existing transition hash) |

## Entity key and entity version

```text
EntityKey     { entity: String, id: String }          -- type name + id value (string form)
EntityContent { entity, declaration: Hash, id, value: {field: canonical value, …, id} }
EntityVersion { content_hash: Hash, entity, id, revision: u64 ≥ 1, created_at: position, value }
```

- **Value encoding:** `value` is encoded as in decision records (typed encode: fixed-scale
  decimals as text with their scale, options as `null` or value, enums as strings).
- **Revisions:** revision 1 is the genesis (seed) version. Each committed transition that changes
  the canonical value creates revision + 1 (R4).
- **Content hash (state element):** `c = document_hash("behavior.entity_content.v1", content)`.
  It binds the type, the declaration, the id and the value, so swapped values or a different
  declaration give different elements. It does **not** bind the revision or any position: equal
  semantic content gives equal state identity, whatever the history.
- **Entity version (history):** one historical incarnation, addressed by `(key, revision)`. Its
  `content_hash` links it to the content; `revision` and `created_at` (the position of the commit
  that created it, 0 for the seed) say which incarnation supplied the value. A stored version
  never changes.

## State and state reference

```text
StateRef { state: StateId, position: u64 }   -- position 0 = genesis, n = after the n-th record
Head     { state_ref: StateRef, acc_num, acc_den: 3072-bit, last_record: RecordId | genesis hash }
```

- **Identity:** `StateId` depends only on the content hashes of the current entities (MuHash3072,
  R3). Equal semantic content gives an equal `StateId`, even at different positions: position 42
  and position 57 may both be state `ABC`.
- **Position:** it orders the history. It does not take part in `StateId`. A transition with no
  effective change keeps the `StateId` and advances the position.
- **Current state:** the current state of a store is its head's `state_ref`.
- **Consistent snapshot:** the snapshot of state `S` at position `n` is the set of versions with
  the newest `created_at ≤ n` per key. Evaluation reads the head once and every entity *as of n*,
  so all reads belong to `S` (FR-018).

## Evidence policy

```json
{"format": "behavior.evidence_policy.v1",
 "require": "none" | "commit_authorization",
 "trusted_execution_policies": ["sha256:…"]          // optional allowlist
}
```

Unknown fields and values are refused.

## Store genesis

```json
{"format": "behavior.store_genesis.v1",
 "evidence_policy": { … },                           // embedded; its hash is cited by records
 "entity_declarations": {"Account": "sha256:…"},     // name → semantic declaration hash
 "seed": [{"entity": "Account", "value": {"id": "a1", "balance": "100.00"}}, …]
}
```

- **Store identity:** the genesis hash. It is also the chain root (`previous_record` of record 1).
- **Seed checks:** seed entities must match their declarations (field set, types, grid and range,
  entity constraints). A key may appear only once in the seed.

## Commit bundle

```json
{"format": "behavior.commit_bundle.v1",
 "evaluated_state": {"state": "sha256:…", "position": 42},
 "behavior_version": "sha256:…",
 "store": "sha256:…",                                // genesis hash (FR-019)
 "record": { …decision record 0.4, result "ALLOW",
             data_version = "store:<genesis>;state:<state id>;position:<n>" },
 "transition_hash": "sha256:…",
 "entity_declarations": {"Account": "sha256:…"},     // for the touched entities
 "read_set": [{"entity": "Account", "id": "a1", "revision": 17,
               "fields": ["balance"]}, …],           // fields actually observed (incl. inside derived
                                                     // values; short-circuited operands excluded)
 "write_set": [{"entity": "Account", "id": "a1", "field": "balance",
                "old": "100.00", "new": "80.00"}, …],
 "commit_time": "2026-09-27T12:00:00Z",
 "evidence": {"authorization": {…}, "execution_policy": {…},
              "attestation": {…}, "waivers": [ … ]}  // optional
}
```

Validation rules:
- **Entities covered:** every state-bound entity of the record is in `read_set`, and `write_set`
  equals the record's `changes` (param names dropped).
- **Hashes:** `transition_hash = document_hash("behavior.transition.v1", record)`. This is the
  transition's semantic identity: it excludes commit time and evidence. The record's
  `data_version` equals the canonical string of `store` and `evaluated_state`.
- **Engine-derived sets (FR-020):** the store re-derives `read_set` (by re-evaluating the record
  with the observed-read collector) and `write_set` (from `changes`), and refuses a bundle whose
  sets differ. An entity appears in `read_set` only if at least one of its fields was observed.
  Entities written without being read appear in `write_set` only. Every written entity must exist in the parent (FR-021).
- **Context (FR-022):** the record's `context` section is the full canonical context snapshot.
- **Parent consistency:** each read revision equals the entity's revision at the parent. Each
  `old` value equals the parent value.
- **Result:** `record.result == "ALLOW"`. Bundles exist only for allowed decisions.
- **Time:** `commit_time` is an RFC 3339 UTC timestamp. It takes part only in the bundle and record
  hashes.

## Transition record

```json
{"format": "behavior.transition_record.v1",
 "position": 43,
 "previous_record": "sha256:…",                      // record 42, or the genesis hash for position 1
 "bundle": { …commit bundle… },
 "bundle_hash": "sha256:…",
 "evaluated_against": {"state": "sha256:…", "position": 42},
 "committed_on":      {"state": "sha256:…", "position": 42},   // == evaluated_against (v1 invariant)
 "result_state":      {"state": "sha256:…", "position": 43},
 "new_versions": [{"content_hash": "sha256:…", "entity": "Account", "id": "a1",
                   "revision": 18, "created_at": 43, "value": {…}}, …],
 "evidence_policy": "sha256:…",
 "authorization": "sha256:…" | null
}
```

- **Three identities:**
  - the transition semantic hash (`bundle.transition_hash`);
  - the bundle hash, which adds commit time and evidence;
  - the audit record identity, `RecordId = document_hash("behavior.transition_record.v1",
    record)`, which adds the result state and the chain.

  Idempotency uses the semantic hash plus the parent. A bundle evaluated at position p is
  recognized as already committed if `record(p+1)` has the same semantic hash, even when the
  head has moved on (FR-009).
- **History:** the history is the chain of records from position 1 to the head's position.

## Store results and errors

| Result | Meaning |
|---|---|
| `Committed { record_id, result_state }` | Applied atomically |
| `AlreadyCommitted { record_id, result_state }` | The same semantic transition on the same parent was committed as `record(p+1)`, even with a different commit time and even if later commits followed (FR-009) |
| `STATE_CONFLICT { current, changed: [EntityKey] }` | The expected parent is not the head (FR-007) |
| `NOTHING_TO_COMMIT` | The decision is not `ALLOW` (no bundle exists) |
| `ENTITY_NOT_FOUND` | A binding names an entity that is not in the state |
| `ENTITY_DECLARATION_MISMATCH` | The behavior version declares an entity differently from the store (R5) |
| `BUNDLE_INVALID` | A hash, revision, old value, or structure check failed, or `expected_parent ≠ evaluated_state` |
| `EVIDENCE_REQUIRED` | The policy requires an authorization and none was supplied |
| `EVIDENCE_MISMATCH` | The authorization or its documents do not match the bundle, the store or the allowlist |
| `ENTITY_UNIVERSE_CHANGED` | A write names an entity that is not in the parent state (FR-021) |
| `BACKEND_ERROR` | The host backend failed; nothing was applied (atomicity) |

## Replay reports

```json
{"format": "behavior.replay_report.v1", "kind": "data" | "behavior",
 "from": {…StateRef}, "to": {…StateRef}, "checked": 1000,
 "ok": false,
 "divergence": {"position": 512, "kind": "state" | "parent" | "chain" | "record" |
                "changes" | "decision" | "invariant", "expected": "…", "found": "…"}}
```

## Backend primitives (host-implemented)

| Primitive | Contract |
|---|---|
| `genesis()` | The genesis document the store was created with |
| `head()` | The current `Head` (none before creation) |
| `create(genesis, head, versions)` | Once: stores the genesis, the seed versions and the initial head |
| `version_at(key, position)` | The version of `key` with the newest `created_at ≤ position` (as-of read; immutable data) |
| `version(key, revision)` | A specific version |
| `record(position)` | The transition record at `position` |
| `commit(expected_head_record, versions, record, new_head)` | Atomic compare-and-set, crash-safe: if the head's `last_record` is still the expected one, write the versions, the record (with its idempotency information) and the head, with accumulator and state reference, as one unit; otherwise report that the head moved, with no effect. After a crash, either everything or nothing is visible |

State transitions of a store: `created (S0)` → commit → `S1` → … Every refused commit leaves the
head unchanged.
