# Contract: Verification Attestation v1

The canonical output of `verify`. Canonical JSON (sorted keys, compact, no floats). Its hash is
`SHA-256("behavior.verification.v1" ‖ 0x00 ‖ canonical bytes)` computed without the `hash` field
and without per-check `cached` flags, so cached and fresh runs produce the same attestation hash.

```json
{
  "attestation_version": "1",
  "behavior_version": "sha256:…",
  "profile": {"checks": ["dead_action", "evaluation_error", "postcondition", "preservation", "redundancy", "vacuity"],
              "rlimit": 20000000, "wall_clock_guard_ms": 60000, "hash": "sha256:…"},
  "verifier_version": "0.2.0",
  "solver_version": "z3 4.16.0",
  "result": "not_verified",
  "checks": [
    {"kind": "preservation", "action": {"name": "approve", "hash": "sha256:…"},
     "subject": {"kind": "invariant", "name": "within_budget", "hash": "sha256:…",
                 "loc": {"file": "demo.py", "line": 51}},
     "key": "sha256:…", "outcome": "counterexample", "cached": false}
  ],
  "findings": [
    {"hash": "sha256:…", "kind": "preservation", "severity": "blocking",
     "cites": {"action": "sha256:…", "invariant": "sha256:…"},
     "locs": [{"file": "demo.py", "line": 56}, {"file": "demo.py", "line": 51}],
     "explanation": "approve can break within_budget: project.spent + purchase.amount > project.budget",
     "counterexample": {
       "state": {"project": {"id": "e0", "budget": "0", "spent": "0"},
                 "purchase": {"id": "e1", "amount": "0.01", "status": "pending", "approved_by": null}},
       "input": {},
       "context": {"actor": {"id": "e2", "role": "manager", "approval_limit": "0.01"}},
       "record": {"result": "DENY", "…": "the confirming decision record"}
     }}
  ],
  "hash": "sha256:…"
}
```

## Rules

- `checks` lists every check of the profile, sorted by (action name, kind, subject hash);
  checks not tied to an action (vacuity of rules) use `action: null` and sort first.
- `outcome`: `proven`, `counterexample`, or `inconclusive`. Inconclusive checks carry
  `"reason"`: `solver_unknown`, `resource_limit`, `wall_clock_guard`, or
  `counterexample_not_reproduced`.
- For `dead_action`, `redundant_precondition`, `always_true`, and `always_false`, "proven" means
  the property was shown to hold, which is itself the (warning) finding; "counterexample" means a
  witness shows it does not hold, which is not a finding.
- `findings` are sorted by `hash`. A finding hash depends only on kind and cited hashes.
- `result` is `verified` exactly when no finding has severity `blocking`.
- Counterexample ids are generated as `e0`, `e1`, … in parameter order; values use the engine's
  request encoding (decimals as normalized strings).
- Two runs with the same behavior version, profile, verifier version, and solver version produce
  byte-identical attestations, except that `wall_clock_guard` outcomes are marked
  `"reproducible": false` and make the attestation non-canonical (never cached, never attested
  as verified).
- Every inconclusive check yields a blocking finding with `"kind": "inconclusive"`, `"check"`
  naming the check kind, no counterexample, and a hash over `check`, the action, the subject, and
  the parameter position (FR-009). This is the finding a waiver refers to.
- Findings bound to an action parameter include the parameter's position (not its name) in the
  finding hash, so `transfer` breaking a rule for `from_` and for `to` are distinct findings.
