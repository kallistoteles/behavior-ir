# Implementation log: model type identity (feature 505)

**Branch**: `505-model-type-identity`, from `main` `8c44081`. **Core Release**: 0.13.0, unchanged.

## Baseline (T001)

The manifest at `main` `8c44081` (`git show main:models/fixtures/compat.json`). T018 compares the
BehaviorHash, the generated hashes and the ApprovalHashes of every case against these values.

| Tuple | Case | BehaviorHash | ApprovalHash(es) |
|---|---|---|---|
| approval 1/1 | `ap_purchase_request` | `sha256:2a301cd86097470ff113c79b4db213d95e143a8589cb4d4870df28885b636b12` | `purchase_request_approval` `sha256:5f75c1365aa9d8597a211847253772e86d395a871f8c7186d146f627389d9ecd` |
| approval 1/1 | `ap_two_approvals` | `sha256:8f515e2456ba68e36806814fdd46a6272df934ec3a63407bb98bbb17c3443eb3` | `finance_approval` `sha256:59257e0476ec3aca1828851a4dbc36627fb829fee8b9d9d44317851a7e9a6575`, `legal_review` `sha256:408a18840eccdb6eec10dc1f6258d5816a7990fbf7a77e2e27e414515ff2dda8` |
| approval 1/1 | `ap_document_review` | `sha256:400dbdc34b7a54c2567556d72e9593fe2a6ae2fe28e6d7dd0f8d3a7ca13db9ef` | `document_review` `sha256:29c0a010bfe98e14e8c544f393553e500699ca847b59f79e39970316829dbb5e` |
| request_flow 1/1 | `rf_purchase_request` | `sha256:6a51f91049227d70d7b61dcd53f1e4e23127c8329e6ad0cb21f6bc1d2b182062` |  |
| request_flow 1/1 | `rf_two_flows_local_submit` | `sha256:98c6b300c763d24b699bdb30f60c24e2ac20f6aea76ea6a3c4a76a6fb49f920b` |  |
| request_flow 1/1 | `rf_with_standalone_machine` | `sha256:8cabd65707a101b365b714f6db4aea285a7244aa1df55521cbb0f5b38ce6ccf2` |  |
| state_machine 1/1 | `order` | `sha256:eaeae2756f0624ccdb8acb57711fc4a9b07c032256c6f35d87aaa4fb1c65d970` |  |
| state_machine 1/1 | `purchase_request` | `sha256:02286fb386b82177012dfc4328b39b06bbe4ffbeb200da2b02ea59b69dbaa10a` |  |
| state_machine 1/1 | `request_flow_reference` | `sha256:7192caa4714bd3a2eff38a4058a2803ed04f718222316e3bf1157b08bff75d30` |  |
| state_machine 1/1 | `two_machines_same_local_ids` | `sha256:5bd2c5c65f10f56a6097a1942e342e6ff5c3db0ebc002a8e67d01f837efeb88f` |  |

## Lowering and Approval unchanged (T018)

The new manifest compared case by case with the baseline. Only StateMachine and RequestFlow
model hashes (and the embedded machine hashes recorded with flows) differ.

| Case | BehaviorHash | generated | ApprovalHash | Model hashes changed |
|---|---|---|---|---|
| `ap_document_review` | equal | equal | equal |  |
| `ap_purchase_request` | equal | equal | equal | purchase_request (embedded: purchase_request) |
| `ap_two_approvals` | equal | equal | equal | ticket |
| `order` | equal | equal | – | order |
| `purchase_request` | equal | equal | – | purchase_request |
| `request_flow_reference` | equal | equal | – | request |
| `rf_purchase_request` | equal | equal | – | purchase_request (embedded: purchase_request) |
| `rf_two_flows_local_submit` | equal | equal | – | leave_request, purchase_request (embedded: leave_request, purchase_request) |
| `rf_with_standalone_machine` | equal | equal | – | budget, purchase_request (embedded: purchase_request) |
| `two_machines_same_local_ids` | equal | equal | – | invoice, quote |

Defects: none

## Pinned v2 model hashes (T017)

`check-models-compat.py --write --supersede` recorded these tuples. Each replacement lists the tuple it
supersedes, with the same case names; `ap_document_review`'s tuple has no co-supplied model and
is unchanged.

| Tuple | Supersedes | Case | Model hashes |
|---|---|---|---|
| approval 1/1 (co-supplied request_flow 2, state_machine 2) | approval 1/1 (co-supplied request_flow 1, state_machine 1) | `ap_purchase_request` | `purchase_request` `sha256:9ca938b9…f79c62`, `purchase_request_approval` `sha256:5f75c136…9d9ecd` |
| approval 1/1 (co-supplied state_machine 2) | approval 1/1 (co-supplied state_machine 1) | `ap_two_approvals` | `finance_approval` `sha256:59257e04…9a6575`, `legal_review` `sha256:408a1884…f2dda8`, `ticket` `sha256:ced89694…8b950a` |
| request_flow 2/1 (embedded state_machine 2) | request_flow 1/1 | `rf_purchase_request` | `purchase_request` `sha256:3e513364…a90266` |
| request_flow 2/1 (embedded state_machine 2) | request_flow 1/1 | `rf_two_flows_local_submit` | `leave_request` `sha256:3c992f16…56ccf1`, `purchase_request` `sha256:bdae1b1c…aea59e` |
| request_flow 2/1 (embedded state_machine 2) | request_flow 1/1 | `rf_with_standalone_machine` | `budget` `sha256:dac125f7…3ab5b3`, `purchase_request` `sha256:bdae1b1c…aea59e` |
| approval 1/1 | – | `ap_document_review` | `document_review` `sha256:29c0a010…9dbb5e` |
| state_machine 2/1 | state_machine 1/1 | `order` | `order` `sha256:16e07bf1…e64ccf` |
| state_machine 2/1 | state_machine 1/1 | `purchase_request` | `purchase_request` `sha256:8868892b…bc98fb` |
| state_machine 2/1 | state_machine 1/1 | `request_flow_reference` | `request` `sha256:f3867103…533fbe` |
| state_machine 2/1 | state_machine 1/1 | `two_machines_same_local_ids` | `invoice` `sha256:d2256e85…1877e8`, `quote` `sha256:7c4f8d9b…aacde2` |

## Deviations, with reasons

- **`params()` was removed in T013, not T003.** StateMachine and RequestFlow still used it until
  their v2 documents (T008, T013). Keeping it until its last caller went kept T004's check
  meaningful: with every kind still at version 1, the whole manifest, including the approval
  tuples, reproduced after the helpers moved.
- **The identity error's shape.** An unresolved reference is reported the way every identity
  failure already is: `CORE_ADMISSION_FAILED` carrying `MODEL_IDENTITY_FAILED` with a message
  naming the model, the referring item and the type (`identity_error`). The contract's
  `MODEL_IDENTITY_FAILED` is that inner code.
- **One supersession condition beyond the contract's test table.** A tuple listed by more than
  one replacement fails (`superseded by more than one tuple`), as contract §3(b) ("exactly one")
  requires. It has no dedicated script test (analysis finding C2, LOW).

## Review fixes (supersession)

- **[P1] Accepted history after the merge.** Once this change is on `main`, the merge-base
  manifest holds the replacement tuples with `supersedes`, but not their predecessors, so an
  unchanged run failed with `never established`. Entries the same tuple already listed at the
  merge base are now history and are not validated again; only newly introduced entries are.
  An established tuple's `supersedes` list is kept byte-identical like its cases.
- **[P2] Replacements must be built.** A superseding tuple must be a tuple of the current
  build, so the cases it takes over are reproduced. Otherwise an unbuilt v2 could supersede v1
  while the built v3 lost a case.
- **[P2] Repeated bumps before the merge.** `--write --supersede` took a branch-local v2 as
  v3's predecessor and dropped v2's supersession of v1, so the next gate run rejected v3
  (`never established`). Predecessors now resolve against the merge base. A replaced
  branch-local tuple is dropped, and its established predecessors carry over (validated against
  the new tuple, or the writer fails). Test: "two bumps before the merge".
- Script tests: "accepted supersession history after the merge" (PASS), "accepted supersession
  history edited" (FAIL) and "superseded by a tuple that is not built" (FAIL). Contract §3 and
  §6 updated.

## Gates (T022)

- `BEHAVIOR_FULL_PROPERTIES=1 cargo test -p behavior-models`: 206 passed, 0 failed (5 m 17 s).
- `scripts/gates.sh` on the final code: `gates: OK`. This covers the core pin (v0.13.0, so no
  Core change, FR-012), the public surface, fmt, clippy `-D warnings`, `cargo test`, pytest
  (546 passed, 4 skipped, 3 xfailed: the behavior-ir-core#31 limitation), mypy, models compat,
  determinism, ownership, terms, workflows and the script tests (`test_models_compat: OK`). The
  log is `505-gates-final2.log` (after the review fixes), SHA-256
  `313393f11fb563d2e555df46050ce65265f6bdfae1d5da1d54692dca74a4117e`.

## Quickstart (T021)

| # | Scenario | Result |
|---|---|---|
| 1 | StateMachine declaration dependencies | `identity`: 11 passed, among them `declaration_dependencies_change_the_machine_hash` and `unrelated_declarations_and_order_keep_the_machine_hash` |
| 2 | RequestFlow declaration dependencies | `request_flow_identity`: all passed, among them `declaration_dependencies_change_the_flow_hash`, `a_transition_context_type_changes_the_embedded_machine_hash` and `unrelated_declarations_keep_the_flow_hash` |
| 3 | Unresolvable reference | `identity::tests`: 4 passed (missing, other kind, entity reference) |
| 4 | Lowering unchanged | The T018 table: every BehaviorHash and generated hash is equal |
| 5 | Approval unchanged | `approval_identity`: 8 passed; every ApprovalHash is equal (T018) |
| 6 | Gate supersession | `test_models_compat: OK` (the contract §6 cases and the review cases) |
| 7 | Manifest | `check-models-compat: OK`: 5 tuples, one per case, with replacements listing their predecessors |
| 8 | Pinned Python cases | `test_model_staging.py`: 9 passed, without edits |
| 9 | Determinism (full tier) | 206 passed, including the RequestFlow permutation property at 1,000 cases |
| 10 | Gates | `gates: OK` |
