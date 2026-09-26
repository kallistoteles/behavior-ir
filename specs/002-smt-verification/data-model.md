# Data Model: SMT Verification of Behavior Modules

Extends feature 001's data model (`specs/001-verifiable-behavior-ir/data-model.md`). JSON shapes
are in [contracts/](contracts/).

## Changes to the behavior model (feature 001)

### Entity constraint (new)

| Field | Rules |
|-------|-------|
| `name` | unique among derived values, invariants, constraints, and actions |
| `entity` | the entity type it constrains |
| `param` | the name bound to the instance in the body |
| `body` | Bool expression over that one entity (no other parameters) |
| `hash` | `behavior.constraint.v1`; module entry kind 7 |

Checked on every incoming entity of its type in any role, and on every state entity of its type
on S'. State invariants (feature 001) are unchanged: state entities only, on S and S'.

### Evaluation results (extended)

`ALLOW`, `DENY`, `ERROR`, `INVALID_INPUT`, `INVALID_STATE`, and new: `INVALID_CONTEXT`
(a context entity breaks an entity constraint) and `INVALID_BINDING` (reason
`STATE_ALIAS_NOT_ALLOWED`: two state parameters bound to entities with the same type and `id`).

```text
request ─shape/type─► INVALID_INPUT
   ▼
binding: distinct state identities ─► INVALID_BINDING
   ▼
entity constraints: state ─► INVALID_STATE, input ─► INVALID_INPUT, context ─► INVALID_CONTEXT
   ▼
state invariants on S ─► INVALID_STATE
   ▼
preconditions ─► DENY / ERROR
   ▼
effects → ΔS ─► ERROR
   ▼
S' checks: postconditions, state invariants, entity constraints on state ─► DENY / ERROR
   ▼
ALLOW
```

### Change-set entry (extended)

`{param, entity, id, field, old, new}`: `entity` and `id` identify the state cell; `param` is kept
for readability. Records carry `record_version "0.2"`.

## Verification

### Verification profile

| Field | Content |
|-------|---------|
| `checks` | subset of `preservation`, `postcondition`, `evaluation_error`, `dead_action`, `redundancy`, `vacuity` (default: all) |
| `rlimit` | solver resource budget per check (deterministic) |
| `wall_clock_guard_ms` | safety cap; results stopped by it are not reproducible and not cached |
| `hash` | content hash of the profile |

### Check

| Field | Content |
|-------|---------|
| `kind` | `preservation`, `postcondition`, `evaluation_error`, `dead_action`, `redundant_precondition`, `always_true`, `always_false` |
| `subject` | action name and hash; plus invariant/constraint, postcondition, precondition, rule, or expression it concerns (name, hash, location) |
| `key` | cache key (research R8) |
| `outcome` | `proven`, `counterexample`, or `inconclusive` (with reason `solver_unknown`, `resource_limit`, `wall_clock_guard`, `counterexample_not_reproduced`) |
| `cached` | whether the outcome came from the cache (not part of the attestation hash) |

### Finding

A check whose outcome is not `proven` (for dead action, redundancy, vacuity: whose property holds).

| Field | Content |
|-------|---------|
| `kind` | as Check, plus `inconclusive` |
| `severity` | `blocking` (preservation, postcondition, evaluation_error, inconclusive) or `warning` (dead_action, redundant_precondition, always_true, always_false) |
| `hash` | finding hash: `behavior.finding.v1` over kind and cited hashes |
| `cites` | content hashes and source locations |
| `counterexample` | `{state, input, context, record}` when applicable; `record` is the confirming decision record |
| `explanation` | human-readable text |

### Verification attestation

| Field | Content |
|-------|---------|
| `behavior_version` | exact behavior hash |
| `profile` | the profile (with its hash) |
| `verifier_version`, `solver_version` | identify the verifier build and solver |
| `result` | `verified` (no blocking finding) or `not_verified` |
| `checks` | every check with outcome, sorted by (action, kind, subject hash) |
| `findings` | findings sorted by finding hash |
| `hash` | `behavior.verification.v1` over the canonical attestation without the `hash` field and without per-check `cached` flags |

## Governance

### Waiver

`behavior_version`, `finding_hash`, `profile_hash`, `verifier_version`, `rationale`,
optional `expires_at` (RFC 3339 UTC). No reviewer field. `hash`: `behavior.waiver.v1`.

### Signed attestation

`waiver_hash`, `key_id` (`ed25519:<hex public key>`), `signature` (hex, 64 bytes) over
`"behavior.waiver.v1" ‖ 0x00 ‖ waiver hash bytes`. Not part of the waiver hash.

### Execution policy

| Field | Content |
|-------|---------|
| `require` | `verified` or `verified_or_waived` |
| `required_profiles` | profile hashes an attestation must use (empty: any) |
| `waivable` | finding kinds that may be waived (default `["inconclusive"]`) |
| `forbidden` | finding kinds that may never be waived (checked before `waivable`) |
| `trusted_keys` | `[{key_id, principal, roles}]` |
| `required_roles` | finding kind → roles that may waive it (default: any trusted role) |
| `accept_expired` | Bool (default `false`) |
| `hash` | `behavior.policy.v1` |

### Commit authorization

| Field | Content |
|-------|---------|
| `behavior_version` | behavior hash |
| `transition_hash` | `behavior.transition.v1` over the canonical decision record |
| `policy_hash` | policy used |
| `verification` | attestation hash and result, or `unverified` |
| `waivers_used` | `[{waiver_hash, finding_hash, key_id, signature, principal}]` |
| `now` | caller-supplied time used for expiry |
| `decision` | `allow` or `refuse` |
| `reasons` | why (e.g. `not_verified`, `finding_not_waivable`, `waiver_expired`, `untrusted_key`) |
| `hash` | `behavior.authorization.v1` |

Authorizations are never invalidated by later policy changes; they cite the policy they used.
