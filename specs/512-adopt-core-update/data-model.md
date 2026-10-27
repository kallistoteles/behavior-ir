# Data and evidence model

All records below are build/review evidence, not persistent application state.

| Record | Required identity and validation |
|---|---|
| Baseline artifact | Published Behavior 0.14.0 wheel/source identity, SHA-256, exact Core 0.15.0/verifier 0.10.0 and installed environment |
| Selected Core release | Published tag, immutable commit, manifest, verifier version and checked asset hashes; merged-only branches are invalid |
| Semantic comparison | All reference receipts/Model identities and 506/507 histories; normalize only top-level nonsemantic Core release metadata |
| Verification comparison | Exact module/profile/verifier and real obligation inventory, outcomes and allowed/refused runtime controls; kept separate from semantic comparison |
| Safety evidence | Exact module/policies/dependencies and existing proof contract; regenerated inventory/evidence may change with Core identity; old artifacts stay immutable |
| Growth observation | Starting/ending population, repetition, eight creation cycles, per-phase/whole timing, output bytes, versions, environment and replay results |
| Frozen performance profile | Baseline digest, fixed workloads/environment, measurable acceptance ratios/budgets selected before candidate measurement |
| Readiness ledger | Completed checks, exact tested source/package identities, blocking prerequisites and deferred nonblocking findings |

Readiness progression: preparation → qualifying Core release selected → candidate built → acceptance passed. No later state may be claimed while a prerequisite is missing. Publication is a separate explicit action after merge.
