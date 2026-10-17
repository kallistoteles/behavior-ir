# Verification comparison: Core 0.13.0 → 0.15.0 (feature 508, spec Q8.2)

Reports: `0.13.0-506-full-report.json`, `0.15.0-506-full-report.json`,
`0.13.0-507-full-report.json`, `0.15.0-507-full-report.json` (all full mode).

## Expected and observed changes

| Item | 0.13.0 | 0.15.0 |
|---|---|---|
| Verifier | 0.9.0 | 0.10.0 |
| Solver | z3 4.16.0 | z3 4.16.0 |
| 506 C19 status | `blocked` by behavior-ir-core#35 | `passed` |
| 506 C19 attestation result | `not_verified` (2 inconclusive findings) | `verified` (no findings) |
| C19 module | `sha256:a04ba37…531779` | `sha256:a04ba37…531779` (unchanged) |
| Uniqueness postcondition, `purchase_request_approval__approve` | inconclusive (counterexample_not_reproduced) | proven |
| Uniqueness postcondition, `purchase_request_approval__reject` | inconclusive (counterexample_not_reproduced) | proven |
| C19 attestation identity | (0.9.0 attestation) | `sha256:d862a010…c482d34` (fresh, verifier 0.10.0) |
| 506 readiness registry | #35 (affects C19), #29, #31 (affect nothing) | empty |
| 506 full conformance | NOT complete | complete (39 passed) |
| 507 checks (61) | all passed | all passed, no status change |
| 507 obligations (12) | all passed | all passed, no status change |
| 507 SMT evidence (preservation, postconditions, claim evaluation_error) | proven | identical |

No other check changed status in either suite. Fresh attestation, report and cache identities
differ by design (verifier 0.10.0) and are not compared. Historical 0.9 attestations are not
reinterpreted.
