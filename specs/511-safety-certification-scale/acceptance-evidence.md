# Acceptance evidence

G3 passed all 60 pairs and complete stage/identity assessment. Full release gates, frozen controls,
installed reference/public regression and quickstart reconciliation passed. Formal claims, trusted structural
correspondence, empirical observations and host obligations remain separate evidence classes.

| Requirement | Evidence class | Recorded evidence and scope |
| --- | --- | --- |
| FR-001 | Structural correspondence + concrete kernel checks | Full independent Core inventory, exact operations and expected policies: `inventory.rs`/`prepare_bundle`; all frozen counts/identities passed [complete assessment](evidence/final-runs/assessment.json). Rust extraction remains trusted correspondence. |
| FR-002 | Compatibility tests | Unchanged v1 predicates; 328 Rust / 719 Python tests including 82 actual membership proofs, all tied to the final checker identities. [Regression record](evidence/v1-regression/membership-execution-final.json); [Installed final corpus passed](evidence/v1-regression/reconciliation.json). |
| FR-003 | Content identity + type/proof checks | [Frozen candidate](evidence/candidate-builds/final-01.json): checker 3, statement 2, evidence/bundle v2; exact module/policy/witness/source/toolchain binding. [Final artifact match](evidence/delivery-gates/final-artifact-binding.json). |
| FR-004 | Explicit assumptions / host obligations | Unestablished genesis, excluded migrations, exact approved module/complete-policy set and fresh enforced activation duties remain explicit in records and assessment. |
| FR-005 | Machine-checked refinement | Kernel-checked UTF8 flatten/injectivity and `checked_from_parts`; unchanged twelve baseline obligations retained. See [root mapping](proof-obligations.md). Rust encoding is trusted. |
| FR-006 | Machine-checked permutation | `replayMerge_perm`/`replayTrace_perm` consume all original keys with multiplicity. All 120 roles passed; [finite-premise controls](evidence/final-controls/extracted-premises.json). |
| FR-007 | Machine-checked soundness | `traceChecked_sound`: strict adjacent full-key order plus transitivity and permutation imply original uniqueness for arbitrary proposed trace. [Root mapping](proof-obligations.md). |
| FR-008 | Unchanged mathematical predicate | Original `allowed`/`checkAuthority` preserved; denied permissions fail. [Generic controls](evidence/final-controls/generic-premises.json), [actual mutations](evidence/final-controls/proof-integrity.json). |
| FR-009 | Source assessment + measurements | [Verified algorithms research](verified-algorithms-research.md), [candidate comparison](evidence/candidate-comparison.json), [selection](design-decision.md); no new dependency. |
| FR-010 | Machine-checked adapters | Pinned permutation/order/string laws and bridge lemmas checked; exact intended-type/transitive-axiom audit plus fresh replay for every actual role. |
| FR-011 | Machine-checked generic + concrete proof | All 16 audited roots and all 12 tool stages passed in 120 roles. [Complete assessment](evidence/final-runs/assessment.json). |
| FR-012 | Integrity checks | Only propext/Classical.choice/Quot.sound; actual proof-hole, custom/native-axiom and weakened-statement mutations rejected. [Integrity controls](evidence/final-controls/proof-integrity.json). |
| FR-013 | Evidence separation | Semantic records exclude timing/path/resource noise; operational measurements separate. All 20 requests reproduce identity across 3 pairs and producer/consumer. [Reproducibility](evidence/reproducibility/semantic-repeats.json). |
| FR-014 | Empirical full-chain result | Three sequential cold pairs each for A256/A512 passed full checking under 300 s/role. [Floor records](evidence/final-runs/mandatory-floors.json). |
| FR-015 | Frozen operational contract | Original immutable [profile](evidence/evaluation-profile.json) and hash; independently frozen exact build; [all budgets passed](evidence/final-runs/assessment.json). |
| FR-016 | Empirical growth | [Six-axis results](scalability-results.md): all 20 cases × 3 pairs, material mixed/shared/generated interactions, coupled dimensions labeled; no asymptotic guarantee invented. |
| FR-017 | Measurement scope | Whole-role wall, waited-child CPU, sampled tree memory and actual stage flags; heartbeat/depth consumption unavailable explicitly. [Aggregates](evidence/final-runs/scaling-summary.json). |
| FR-018 | Controlled observations | All 60 pairs sequential against installed wheel with fresh workspaces, no cached request results and denied producer-directory search. Prototype import reuse remains diagnostic only. |
| FR-019 | Fail-closed diagnostics | Bad transport/premise, unsupported shapes and exhausted obligations remain distinct; none certifies. [Transport controls](evidence/final-controls/transport.json) and generic/extracted checks; invalid witness is not module unsafety. |
| FR-020 | Positive/negative controls | Frozen A512 transport 9 controls, actual A1 proof 7 mutations, actual D1 finite-premise 7 controls, exact generic fixtures all passed; malformed driver 01 quarantined. [Controls](evidence/final-controls/). |
| FR-021 | Determinism | All 20 exact requests have identical semantic evidence across 3 complete pairs and producer/consumer. Rust alternate-valid-witness, display-label and path regressions passed. [Reproduction](evidence/reproducibility/semantic-repeats.json). |
| FR-022 | State transparency | Full state-transparency and Core replay tests, installed ordinary smoke without Lean, exact unchanged Core IR/receipts. [Release check](evidence/delivery-gates/release-check.json). |
| FR-023 | Compatibility / immutability | Published ordinary fixtures/Core pin unchanged; Safety manifest adds 102 lines and deletes 0, preserving six established tuples. All 22 normal baseline receipts unchanged. [Immutability](evidence/immutability-audit.json). |
| FR-024 | Delivery execution | [Full gates](evidence/delivery-gates/full-gates.json), [default release check](evidence/delivery-gates/release-check.json), explicit complete 506/507 and exact rebuild passed. [Artifact binding](evidence/delivery-gates/final-artifact-binding.json). |
| FR-025 | Investigated separation | Direct verified sort, inverse/index and trace candidates compared; independent nonbalanced witness passed complete public checker. [Decision](design-decision.md). |
| FR-026 | Trusted correspondence + kernel evidence | Consumer re-admits exact expected module, independently extracts complete inventory/policies, then checks arbitrary proposed trace. Rust correspondence is trusted/tested, not declared formally proved. |
| FR-027 | Strict data parser + adversarial tests | Strict finite postorder data, typed indices/choices and request-derived bounds; no code/olean authority in input. [Scope controls](evidence/final-controls/transport.json); real opaque certificates only after complete Rust/Lean checks. |
| FR-028 | Cold execution contract | Installed exact wheel, fresh workspaces/processes, original request/bundle only and no producer-directory access; pinned dependencies distinct from prior result cache. [Quickstart](quickstart.md). |
| FR-029 | Reference observations | [Actual platform/tool identity](evidence/platform-checks/environment.json): x86_64Linux, Xeon X5670 KVM, 8 CPUs; ARM NotRun explicit and no ARM timings claimed. |
| FR-030 | Separate assessments | Soundness under stated TCB, retained v1 language, no universal producer completeness and finite measured capacity assessed separately. [Installed corpus passed](evidence/v1-regression/reconciliation.json); [assessment](remediation-assessment.md). |

## Success criteria status

| Criterion | Current evidence |
| --- | --- |
| SC-001 | [All three A256/A512 pairs passed](evidence/final-runs/mandatory-floors.json), whole-chain budgets unchanged. |
| SC-002 | All 60 pairs match original exact requests, complete inventory and policy identities; all required stages present. [Assessment](evidence/final-runs/assessment.json). |
| SC-003 | All original/root and refinement statements machine-checked, typed, axiom-audited and fresh-replayed. [Mapping](proof-obligations.md). |
| SC-004 | Actual invalid transport, permissions, guards/bindings/coverage, identities, witness and proof mutations reject. [Final controls](evidence/final-controls/). |
| SC-005 | All six axes/material interactions measured with 3 pairs each; descriptive growth/couplings and residual limits documented. [Results](scalability-results.md). |
| SC-006 | All 20 requests reproduce exact semantic evidence; no timing/path noise; distinct valid witnesses need not share identity. [Records](evidence/reproducibility/semantic-repeats.json). |
| SC-007 | All existing full gates and installed release checks passed; [installed corpus](evidence/v1-regression/reconciliation.json) and [quickstart](evidence/quickstart-validation/execution.json) passed. [Delivery](evidence/delivery-gates/). |
| SC-008 | Root causes established by isolated controls; full frozen profile resolved without larger limits; residual authority cost/finite capacity explicit. [Results](scalability-results.md). |
| SC-009 | Arbitrary-witness soundness and original-guarantee assembly proved; producer correctness not assumed. Rust/Core/probe/encoding trust stays explicit. |
| SC-010 | Every frozen case and repeat passed both 300 s budgets; same exact delivered/rebuilt artifact. [Manifest](evidence/candidate-builds/final-01.json) and [assessment](evidence/final-runs/assessment.json). |
| SC-011 | Complete source v1 regression, exact hashes/receipts and 82 proof instances passed with same frozen proof/checker identity. [Installed reference/public API reconciliation passed](evidence/v1-regression/reconciliation.json). |

## Provenance corrections

Retain discarded invocations alongside passing controls: initial fresh-check PATH error, initial
encoding fixture with duplicate semantic actions, initial pytest prepend/import failures, the
whitespace-only CPU comparison failure and the first full-gate disk exhaustion are not semantic
counterexamples or passing evidence. See `remediation-assessment.md` and `evidence/delivery-gates/`.

The final full-gate retry corrected eight stale fixture/version expectations; all 719 tests then
passed. The first extracted-data diagnostic attempt had a pinned-Lean error-text parser mismatch,
not a candidate proof failure. Its valid false-premise proofs/audit/fresh replay and the invalid
parser result are retained; complete fresh attempt 02 passed all 7 controls. See
[control provenance](evidence/final-controls/invalid-driver-attempt.json).
