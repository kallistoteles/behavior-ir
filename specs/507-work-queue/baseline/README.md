# Pinned baselines

These files are baselines **per Core pin**. Their tests (`scripts/tests/test_pinned_identity_baseline.sh`,
`scripts/tests/test_conformance_506_bytes.sh`) are strict, and the files are never edited to make
a test pass.

| Pin | Taken by | Justification |
|---|---|---|
| Core 0.13.0 | feature 507 (T001) | before the transition-binding extension |
| Core 0.15.0 | feature 508 (T013) | `specs/508-core-release-0-15/evidence/`: the semantic snapshots of 0.13.0 and 0.15.0 are byte-identical (`semantic-comparison.txt`); the compat print and the 506 quick report differ from 0.13.0 only in release metadata (`core_release`, `engine`, `readiness`); the verification changes are in `verification-comparison.md` |

`compat-print.txt` holds every receipt of the reference cases under the pin (22 since 0.15.0;
the 0.13.0 file held the 16 receipts that existed before feature 507's cases).
