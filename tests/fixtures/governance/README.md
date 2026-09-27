# Governance fixtures (feature 002)

- `keys.json`: two fixed Ed25519 key pairs (32-byte seeds as hex, with their `key_id`s).
  **Test-only**: the seeds are public, so no real policy may ever trust these keys.
- `require_verified.json`: commits only attested-verified transitions.
- `preservation_profile.json`: a verification profile with only preservation checks (the purchase
  fixtures with two-decimal Money have a genuine range-overflow evaluation error under the full
  profile: `spent + amount` may exceed 26 integer digits).
- `forced_inconclusive_profile.json`: preservation checks with `rlimit: 1`. **Forced
  inconclusive**: the modules it is used with are easy for the solver; the resource limit alone
  makes every check inconclusive (reason `resource_limit`), so waivers can be tested. Since
  feature 004 no fixture is inconclusive on its own.
- `verified_or_waived.json`: trusts key A as `risk_reviewer`; only `inconclusive` findings are
  waivable, `evaluation_error` findings never are.

Waivers and signed attestations are not stored here: they bind a behavior version and a finding
hash, so the tests build them from a fresh attestation and sign them with the seeds in
`keys.json` (`behavior sign-waiver`, `behavior_verify::governance::sign_waiver`, Python
`sign_waiver`).
