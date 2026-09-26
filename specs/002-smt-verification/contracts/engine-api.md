# Contract: Engine API additions (feature 002)

Extends `specs/001-verifiable-behavior-ir/contracts/engine-api.md`.

## Wire IR 0.2: entity constraints

`ir_version` `"0.2"` adds a top-level list (0.1 documents remain valid and have none):

```json
"constraints": [{"name": "non_negative_limit", "entity": "Employee", "param": "e",
                 "body": <Bool expr over e>, "loc": {...}}]
```

Hash tag `behavior.constraint.v1`, body encoded like an invariant; module entry kind 7. A module
without constraints hashes exactly as in 0.1.

## Rust (`behavior-verify`)

```rust
pub struct Profile { pub checks: Vec<CheckKind>, pub rlimit: u64, pub wall_clock_guard_ms: u64 }
pub fn verify(module: &Module, profile: &Profile, cache: Option<&Path>, solver: &dyn Solver)
    -> Attestation;
pub fn waiver_hash(waiver_json: &str) -> Result<String, GovernanceError>;
pub fn authorize(policy_json: &str, module: &Module, record_json: &str,
                 attestation_json: Option<&str>, waivers_json: &[String],
                 signed_json: &[String], now: &str) -> Result<Authorization, GovernanceError>;
```

`Solver` is a trait (`fn check(&self, query: &str) -> SolverAnswer`); `Z3Process` runs the
pinned `z3` binary (path from `BEHAVIOR_Z3`, else `z3` on `PATH`).

## CLI

```text
behavior verify <wire.json> [--profile profile.json] [--cache DIR] [--out attestation.json]
behavior waiver-hash <waiver.json>
behavior authorize <wire.json> <record.json> --policy policy.json
                   [--attestation attestation.json] [--waiver waiver.json]...
                   [--signature signed.json]... --now 2026-09-25T12:00:00Z
```

Exit codes: `verify` 0 verified, 1 not verified, 2 admission failure, 64 usage;
`authorize` 0 allow, 1 refuse, 2 invalid input, 64 usage. Outputs are canonical JSON on stdout
(`verify` also writes `--out` when given).

## Python

```python
from behavior import verify, authorize, Profile

attestation = verify(model, profile=Profile(rlimit=20_000_000), cache=".behavior/verify-cache")
attestation.result            # "verified" | "not_verified"
attestation.findings          # list of Finding (kind, severity, hash, explanation, counterexample)
attestation.json              # canonical attestation text (the artifact)

auth = authorize(model, decision, policy=policy_dict, attestation=attestation,
                 waivers=[waiver_dict], signatures=[signed_dict], now="2026-09-25T12:00:00Z")
auth.decision                 # "allow" | "refuse"
auth.json                     # canonical commit authorization
```

DSL addition: `@constraint` (one entity parameter, returns Bool) and
`BehaviorModule(constraints=[...])`. Values cross PyO3 natively (feature 001, research R12).
