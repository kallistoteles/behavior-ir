# Quickstart: validating the Approval Model

**Feature**: 503 | Run from the repository root inside `nix develop`. Core Release 0.12.0 is
unchanged (`scripts/check-core-pin.sh`).

## 1. All gates

```sh
nix develop -c scripts/gates.sh
```

Expected: every gate passes, including:
- `models compat`, with three tuples: `state_machine`, `request_flow`, `approval`;
- the determinism check.

## 2. Scenarios

| # | Scenario | How | Expected |
|---|---|---|---|
| 1 | Thresholds | `pytest python/tests/test_approval.py -k thresholds` | (1,⊥) one approve → APPROVED; (2,⊥) one → PENDING, two → APPROVED; (2,1) one REJECT → REJECTED; (3,2) per the Outcome table; (1,1) the first committed decision decides the outcome and every later attempt is DENIED |
| 2 | One decision per actor | `-k one_decision` | a second approve, or approve then reject, by the same actor is DENIED; the counts are unchanged |
| 3 | Terminality | `-k terminal` | after APPROVED or REJECTED, every decision attempt is DENIED |
| 4 | No reject threshold | `-k no_reject_threshold` | rejections are counted; the outcome stays PENDING until `k_A` approvals |
| 5 | Concurrency, layer 2 | `-k stale_commit` | two decisions evaluated on one parent through the public `Store`: the first commits, the second gets `STATE_CONFLICT`; a retry on the new head is DENIED by the terminal guard |
| 6 | Validity, layer 3, on the child state | `cargo test -p behavior-models approval::lower::tests::child_state` | The decision action is built by the real lowering function with its preconditions omitted (unit test only). It is run through the public `Store` from parents chosen so that the check **passes on `S` and fails only on `S′ = Apply(S, CreateEntity(d))`**: one existing decision `(s, a)` (`Count_S = 1 ≤ 1`, `Count_S′ = 2`), and `A = k_A, R = k_R − 1` with a REJECT. Result: DENIED, no commit, head unchanged. A control variant with the postconditions also omitted commits the invalid state, so the postcondition is what stops it. |
| 7 | Ownership | `cargo test -p behavior-models --test approval_ownership` | hand-written create, field write and removal of `X` → the three `UNAUTHORIZED_DECISION_*` errors; a hand-written read and query of `X` → accepted |
| 8 | Subject untouched | `-k subject_untouched` | subject fields are equal before and after every successful decision |
| 9 | RequestFlow composition | `-k request_flow` | `finalize_approved` is DENIED until the outcome is APPROVED, then commits; the status stays IN_REVIEW while the outcome is APPROVED |
| 10 | Non-request subject | `-k document` | `ap_document_review` runs with no RequestFlow in the module |
| 11 | Declaration DAG | `cargo test -p behavior-models --test declaration_dag` | approval before flow, or flow before approval, in every supply order → identical module; two approvals whose subjects are each other's decision entities → `DECLARATION_CYCLE` |
| 12 | Regression | `pytest python/tests/test_model_staging.py -k pinned_cases` | every 501 and 502 manifest case → its pinned hashes |
| 13 | Properties | `pytest python/tests/test_approval_properties.py` | ≥ 1,000 random decision sequences: uniqueness, threshold consistency, terminal stability, soundness |
| 14 | Identity | `cargo test -p behavior-models --test approval_identity` | permutations (≥ 1,000) give equal ApprovalHash and BehaviorHash; metadata changes nothing; each semantic change alters ApprovalHash |

## 3. Expected terminology in failures

Every test and diagnostic says DENIED for a refused attempt. REJECT and REJECTED appear only
for the recorded choice and the derived outcome.
