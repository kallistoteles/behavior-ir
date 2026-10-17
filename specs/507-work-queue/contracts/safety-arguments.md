# Contract: Safety arguments (feature 507)

These are the structural and inductive arguments behind VO-003, VO-005, VO-007 and VO-011 (spec
Q30, FR-033). Each argument states its premises, its transition domain, its reasoning and its
conclusion. Every premise names its executable enforcement: a build error, a generated guard or
postcondition found in the lowered IR, or an SMT check. Every premise also names the negative
control showing that the conclusion fails without it. A negative control runs on a hand-authored
module that mirrors the generated IR with that premise removed. The shipped lowering has no
switch. The 507 report lists each argument as a `structural` dependency and its negative
controls as separate `test` dependencies. The obligation passes only when every dependency
passes; passing premise checks alone does not establish the complete obligation.

**Transition domain.** Unless stated otherwise, every argument ranges over **ordinary
transitions**: commits of actions of the admitted module, evaluated by Core and accepted by the
Store. Migrations are outside this domain (§5), and so are genesis imports (§6) and anything
outside Core.

## Premises

| ID | Premise | Enforcement | Negative control |
|---|---|---|---|
| P1 | Every write to `attempts` of an `E` entity is made by `<id>__enqueue` (value 0, at creation) or `<id>__claim` (value `attempts + 1`, through its subject) | WorkQueue ownership pass (`UNAUTHORIZED_WORK_WRITE`), `AUTHORED_WORK_EFFECT`, `WORK_ITEM_BINDING`; IR test: claim's generated effect is `attempts := attempts + 1` | NC-writer: in a mirror without the WorkQueue pass, a hand-written action lowering `attempts` lets an old generation complete; the real build rejects the writer |
| P2 | `E` entities are created only by enqueue and never removed | `UNAUTHORIZED_WORK_CREATION`, `UNAUTHORIZED_WORK_REMOVAL` | NC-remove: a removal action rejected by the build |
| P3 | `status` of `E` changes only through the embedded machine, whose transitions are exactly the table in data-model §4 | StateMachine ownership (narrowed exemption, R3), `ENTRY_STATE_*`; IR test: the source and target sets of every `<id>__*` action | NC-status: a hand-written status write rejected |
| P4 | renew, complete, retry and fail require `attempts = generation`, and claim requires `attempts = expected_generation` | IR test: the generated requirement is present and ahead of authored guards; SMT: the postconditions `attempts = generation` and `attempts = expected_generation + 1` are proven | NC-generation: a mirror with the comparison weakened (`attempts ≥ generation` instead of `=`) lets an old-token completion pass, as in W07 |
| P5 | Every capability requires a live lease (`Current`) or an expired lease (`Expired`) as the table says, with explicit `is_some` | IR test on the generated requirements; runtime W08 and W10 | NC-expiry: without `now < lease_until` a late completion passes |
| P6 | M is a constant; `attempts ≤ M` and `READY ⇒ attempts < M` are part of `ValidWork`, which SMT preservation establishes; claim additionally requires `attempts < M`, so an exhausted item is an ordinary DENY rather than an invariant failure | SMT preservation and `evaluation_error` (no overflow); IR test of the claim guard | NC-bound: without the claim and retry guards **and** the bound conjuncts of `ValidWork`, a fourth claim commits; without only the guards, the invariant refuses READY at attempts = M |
| P7 | No transition has a terminal source state | IR test on P3's source sets | the transition table is fixed by the model; the IR check is the enforcement (SA-TERM) |

## SA-MONO (VO-003): attempts are monotone and change only by claim

**Domain**: ordinary transitions.

**Reasoning**:
- By P1, an ordinary transition either leaves `attempts` of an existing item unchanged or is a
  claim on it. A claim sets it to the old value + 1, which SMT proves locally through
  `attempts = expected_generation + 1` together with the requirement
  `attempts = expected_generation`.
- By P2, no item is removed and recreated under the same id, and Core forbids reusing an id.
- **Conclusion**: for every item and every ordinary transition, `attempts' ∈ {attempts,
  attempts + 1}`, and `attempts' = attempts + 1` only for a committed claim on that item.

**Local SMT part**: the per-action postconditions (P4). **Global part**: this argument.

## SA-FENCE (VO-005): an earlier generation never again satisfies `Current`

**Domain**: ordinary transitions after a committed claim to generation k′.

**Claim**: if a committed claim on item w produced generation k′, then for every later state
S and every request with generation k < k′, `Current(w, ·, k, ·)` is false in S.

**Reasoning**:
- By SA-MONO, `attempts(w)` never decreases after the claim, so in every later state
  `attempts(w) ≥ k′ > k`.
- `Current` requires `attempts = k` (P4). It is therefore false.
- The same worker identity does not help, because `Current` compares the generation as well as
  the owner.

**Conclusion**: renew, complete, retry and fail with a stale generation are denied in every
later state. P5 adds that a current generation with an expired lease is also denied.

**Not claimed**: anything about physical workers, or about states produced by migrations (§5).

## SA-TERM (VO-007): terminal items stay terminal and exist forever

**Domain**: ordinary transitions.

**Reasoning**:
- By P3 and P7, no ordinary transition moves an item out of SUCCEEDED, FAILED or CANCELLED.
- By the ownership table (FR-019), every other field of a terminal item is written only by
  capabilities whose source states are non-terminal, so none of them applies.
- By P2, no item is removed.

**Conclusion**: a terminal item's status and fields never change, and it is never removed.

## SA-OWN (VO-011): no action bypasses WorkQueue ownership

**Domain**: the final admitted module, whatever produced each action: hand-written,
StateMachine, RequestFlow, Approval or another queue.

**Reasoning**:
- Core field values change only through action effects and creations, and `actions()` lists
  every action.
- The WorkQueue pass inspects every parameter of type `E` in every action against the table,
  and the StateMachine pass inspects `status`.
- **Conclusion**: if the build succeeds, every write to an owned field is one the table allows,
  through the subject.

This argument extends the composition argument of 501 and 503 (models/README.md, Ownership).

## SA-EXCL (VO-008): two competing claims cannot both commit

**Domain**: commits to one Store.

**Reasoning**:
- Core's Store contract: a commit requires `expected_parent = head` and fails otherwise with
  `STATE_CONFLICT`. Two claims evaluated against the same head h can therefore both commit only
  if the first commit leaves the head at h, which a successful commit never does.
- After the first commit, the second would also be denied on re-evaluation (status LEASED,
  `attempts ≠ expected_generation`).

**Evidence class**: `contract`. This holds for a conforming backend (Core backend conformance,
run in full mode as in 506 FR-018).

## §5 Migrations

A migration is not an ordinary transition. Core checks that the migrated state is valid (the
invariant, and therefore `ValidWork`), and nothing more. SA-MONO, SA-FENCE and SA-TERM are
extended across a migration only under the host migration contract (spec, Migration contract
rules 1–7). The report lists it as an assumption.

NC-mig-reset and NC-mig-reactivate (research R11) show that a migration breaking rule 1 or
rule 3 makes an old generation satisfy `Current` again, while `ValidWork` still holds.

## §6 Genesis

Core checks `ValidWork` at genesis (`GENESIS_INVALID`). For an item imported at genesis, the
arguments start at genesis:
- its generation k is what the import says;
- SA-FENCE applies to every claim committed after genesis;
- no claim is implied for the imported lease, and its deadline is not assumed live.
