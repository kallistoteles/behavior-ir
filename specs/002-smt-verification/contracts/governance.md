# Contract: Waivers, Signed Attestations, Execution Policy, Commit Authorization v1

All objects are canonical JSON. Hashes are `SHA-256(tag ‖ 0x00 ‖ canonical bytes)` over the
object without its `hash` field (research R9).

## Waiver (`behavior.waiver.v1`)

```json
{"behavior_version": "sha256:…", "finding_hash": "sha256:…", "profile_hash": "sha256:…",
 "verifier_version": "0.2.0", "rationale": "solver cannot decide the nonlinear margin rule; reviewed by hand",
 "expires_at": "2026-12-31T00:00:00Z"}
```

`expires_at` is optional. There is no reviewer field.

## Signed attestation

```json
{"waiver_hash": "sha256:…", "key_id": "ed25519:<64 hex chars>", "signature": "<128 hex chars>"}
```

The signature is Ed25519 over the bytes `"behavior.waiver.v1" ‖ 0x00 ‖ waiver-hash-bytes` (the
32 raw bytes, not the display string). One waiver may have several signed attestations.

## Execution policy (`behavior.policy.v1`)

```json
{"policy_version": "1",
 "require": "verified_or_waived",
 "required_profiles": [],
 "waivable": ["inconclusive"],
 "forbidden": ["evaluation_error"],
 "trusted_keys": [{"key_id": "ed25519:…", "principal": "anna", "roles": ["risk_reviewer"]}],
 "required_roles": {"inconclusive": ["risk_reviewer"]},
 "accept_expired": false}
```

## Authorization request and result

`authorize(policy, behavior_version, record, attestation | null, waivers, signed_attestations,
now)` where `now` is an RFC 3339 UTC time supplied by the caller.

A waiver is **accepted** for a blocking finding only if: its `behavior_version`,
`finding_hash`, `profile_hash`, and `verifier_version` match the attestation; it has not expired
at `now` (unless `accept_expired`); the finding kind is not `forbidden` and is `waivable`; and at
least one signed attestation over its hash verifies with a key in `trusted_keys` whose roles meet
`required_roles` for that kind. Everything else about a waiver is ignored.

Decision:

1. No attestation, or its `behavior_version` differs from the record's → refuse (`unverified`),
   unless `require` is ever relaxed in a future policy version.
2. `required_profiles` non-empty and the attestation's profile hash not in it → refuse.
3. Attestation `verified` → allow.
4. `require` is `verified_or_waived` and every blocking finding has an accepted waiver → allow.
5. Otherwise refuse, with one reason per uncovered finding.

## Commit authorization (`behavior.authorization.v1`)

```json
{"authorization_version": "1",
 "behavior_version": "sha256:…",
 "transition_hash": "sha256:…",
 "policy_hash": "sha256:…",
 "verification": {"attestation_hash": "sha256:…", "result": "not_verified"},
 "waivers_used": [{"waiver_hash": "sha256:…", "finding_hash": "sha256:…",
                   "key_id": "ed25519:…", "signature": "…", "principal": "anna"}],
 "now": "2026-09-25T12:00:00Z",
 "decision": "allow",
 "reasons": [],
 "hash": "sha256:…"}
```

`transition_hash` is `SHA-256("behavior.transition.v1" ‖ 0x00 ‖ canonical decision record)`.
When several signatures could justify a waiver, the one with the smallest `key_id` that
satisfies the policy is recorded, so the authorization is deterministic.
