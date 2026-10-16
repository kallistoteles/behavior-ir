# Data model: End-to-End Behaviour Conformance (feature 506)

## 1. Reference workflow domain (contract `reference-workflow.md`)

| Item | Kind | Definition |
|---|---|---|
| `Role` | authored enum | `Requester`, `Approver` |
| `User` | authored entity | `name: String`, `role: Role` |
| `BudgetStatus` | authored enum | `Open`, `Closed` |
| `Budget` | authored entity | `status: BudgetStatus`, `limit: Int`; invariant `limit >= 0` (gives VO-001 its negative control) |
| `RequestApproved` | authored command | `request: Id<PurchaseRequest>`, `by: Id<User>` |
| `purchase_request` | RequestFlow | actor `User`; status `PurchaseRequestStatus` (`Draft`, `Submitted`, `InReview`, `Approved`, `Rejected`); payload `title: String`, `amount: Int`; assignee; creation `create` → `Draft`; transitions `submit` (Draft→Submitted, the requester), `start_review` (Submitted→InReview, `reviewer` input), `finalize_approved` (InReview→Approved, requires `approved(request)`, emits `RequestApproved`), `finalize_rejected` (InReview→Rejected, requires `rejected(request)`) |
| `budget` | StateMachine | over `Budget.status`; entry `Open`; `close` (Open→Closed) |
| `purchase_request_approval` | Approval | subject `PurchaseRequest`, actor `User`, `k_A = 2`, `k_R = 1`; Open: `request.status == InReview`; Eligible: some `User` with `id == actor` and `role == Approver` (a query); context `actor: Id<User>`; actor expression `actor` |
| `set_role` | hand-written action | `user: User` (state), `role: Input Role`; sets `user.role` |
| `decisions` | declared read | every decision row: subject, actor, choice |

**Seed (`S₀`)**:
- users `alice` (Requester), and `bob`, `carol` and `dave` (Approver);
- budget `b1` (`Open`, 1000);
- no request and no decision.

`commit_time` is fixed (`2026-10-16T00:00:00Z`). The genesis is v2 with evidence policy `none`
(Core's fixture `governance-v2/evidence-policy-none.json`), because the module declares a
command.

## 2. Scenario

`{id, level (1–4), story, preconditions, operation, expected (key into expected.json),
verification}`. The ids are C01–C19, with C13a, C13b, C16a and C16b. A scenario may produce
several **checks**, for example C14's four negative controls. Each check is a report entry.

## 3. Expected outcomes (`expected.json`)

A JSON object keyed by scenario id. Values are stated by hand from the workflow definition
(FR-023), for example:

```json
{
  "C04": {"result": "ALLOW", "outcome_after": "PENDING", "history_delta": 1},
  "C05": {"outcome": "APPROVED", "finalize": "ALLOW", "status_after": "Approved",
          "commands": [{"command": "RequestApproved", "request": "r1", "by": "alice"}]},
  "C06": {"outcome": "REJECTED", "finalize_rejected": "ALLOW", "status_after": "Rejected"},
  "C07": {"result": "DENY", "history_delta": 0},
  "C08": {"result": "DENY", "history_delta": 0},
  "C10": {"first": "COMMITTED", "second": "STATE_CONFLICT", "history_delta": 1},
  "C11": {"commit": "STATE_CONFLICT", "reevaluated": "DENY"}
}
```

The complete table, with every scenario and reason code, is in contract
`reference-workflow.md` §3.

## 4. Check outcome and statuses

| Status | Meaning | Owner required |
|---|---|---|
| `passed` | the check met its expectation | no |
| `failed` | it did not; an unexpected exception or a plain skip also counts as failed | no |
| `inconclusive` | no definite answer, such as a solver unknown or timeout | yes |
| `unverifiable` | historical inputs are incomplete | yes |
| `unsupported` | the capability is missing in the pinned release or the environment (including quick mode for C19, or C16a without its Rust half) | yes |
| `blocked` | an open upstream issue the check depends on | yes (the issue) |

Only `passed` counts as passed.

## 5. Readiness registry

`{issue: "behavior-ir-core#29", affects: [], reason: "506 uses run_conformance, not Core's
binding fixtures"}` and `{issue: "behavior-ir-core#31", affects: [], reason: "506 compares
identities and records, which carry no source locations"}`. A scenario listed under `affects`
reports `blocked` while the issue is open.

## 6. Authoring identity (C16a)

`{behavior_hash, items: {name: item_hash}, models: {model_id: {kind, model_semantic_version,
lowerer_version, model_hash}}, generated: {approval_id: …}}`, sorted. Source locations are
excluded.

## 7. Conformance report

```json
{
  "format": "behavior.conformance_report.v1",
  "mode": "quick | full",
  "core_release": {"version": "0.13.0", "commit": "1509497…"},
  "engine": { … `behavior engine-info` … },
  "models": {"state_machine": [2, 1], "request_flow": [2, 1], "approval": [1, 1]},
  "profiles": {"evaluation": "command_intents", "verification": {"rlimit": 20000000}},
  "readiness": [ … §5 … ],
  "checks": [
    {"id": "C14.altered_facts", "scenario": "C14", "level": 4, "status": "passed",
     "reason": null, "owner": null, "evidence": null}
  ],
  "summary": {"passed": 0, "failed": 0, "inconclusive": 0, "unverifiable": 0,
              "unsupported": 0, "blocked": 0}
}
```

The keys are sorted, the `checks` entries are sorted by `id`, and there are no timestamps.
`evidence` is optional, deterministic data a check attaches whatever its status. C19, for
example, attaches the verified module's BehaviorHash, the attestation hash, the verifier and
solver versions, the profile and every check's outcome (FR-025). `complete` is true only when
every check passed.
