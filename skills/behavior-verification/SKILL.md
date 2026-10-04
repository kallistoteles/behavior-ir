---
name: behavior-verification
description: Run Behavior verification and act on its results (proven, counterexample, inconclusive), fixing the model rather than weakening checks, and telling verifier precision debt from a missing requirement. Use after writing or changing a behavior model, and whenever an attestation is not verified.
release: 0.10.3
---

# Verifying behavior

This skill describes **Behavior release 0.10.3**. Use only what is described here or listed in the
release's public API.

> If the public Behavior API cannot express a requirement, record a semantic gap in
> `SEMANTIC_GAPS.md` (format: skills/README.md). Do not work around it: no engine internals,
> no hand-written IR, no moving the rule into host code, no silent approximation.

## Running it

Verification needs the Z3 SMT solver at the version the release names: `z3` on PATH, or the
path in `BEHAVIOR_Z3`. Without it, verification raises an error that names the prerequisite.
Nothing else needs the solver.

```python
# from examples/outcomes.py
model = BehaviorModule(entities=[Customer, Order],
                       actions=[place_order, place_order_unchecked, raise_limit])
attestation = verify(model, Profile(checks=["postcondition"]))
outcome = {c["action"]["name"]: c["outcome"] for c in attestation.checks}
```

- `verify(model)` without a profile runs every check.
- `Profile(checks=[...])` selects kinds. The deterministic solver budget is `rlimit`.
- On the command line: `behavior verify module.json` prints the same attestation and exits 0
  only when it is verified.
- The attestation is a content-addressed record bound to the model's `behavior_version`. Keep it
  with the model.

## The checks

| Profile kind | Check name(s) | Question |
|---|---|---|
| `preservation` | `preservation`, `referential_integrity` | Can an allowed transition end in a state that breaks an entity constraint, an invariant, a module invariant, or leave a `Ref` pointing at a removed entity? |
| `postcondition` | `postcondition` | Can an allowed transition violate an `ensures`? |
| `evaluation_error` | `evaluation_error` | Can evaluation fail (division by zero, overflow, a fixed-scale range) on a reachable path? This includes every declared read (`action` is `read:<name>`) |
| `dead_action` | `dead_action` | Is an action impossible to run? (a warning) |
| `redundancy` | `redundant_precondition` | Is a precondition always true when reached? (a warning) |
| `vacuity` | `always_true`, `always_false` | Is a `@rule` constant for every valid entity? (a warning) |

For the warning kinds, the outcome `counterexample` means a witness exists, so there is **no**
warning. To see what fired, read the findings, not the checks.

## Reading an attestation

- **`attestation.checks`** has one dict per check, with these keys:
  - `kind`: the **check name**, e.g. `redundant_precondition`, not the profile kind;
  - `action`: `{name, hash}`, or `None` for rules;
  - `subject`: `{kind, name, hash, loc}`, plus `param` for a rule bound to a parameter;
  - `key`;
  - `outcome`;
  - `reason`, present only when the outcome is inconclusive;
  - `cached`.
- **`attestation.findings`** lists only what needs attention. Each finding has:
  - `kind`: the check name, or `inconclusive`;
  - `severity`: `blocking` or `warning`;
  - `explanation`, `cites`, `locs`, `hash`;
  - `counterexample`, for confirmed counterexamples.

  Blocking findings are the ones to act on:
  `[f for f in attestation.findings if f["severity"] == "blocking"]`.
- The same subject can appear in several checks. For example, one expression used in three
  effects gives three `evaluation_error` checks: one per place it is evaluated.

## The three outcomes

- **`proven`**: the property holds for every valid state, input and context the model allows. It
  holds for the semantics the model states, as modelled; requirements you never wrote down are
  not checked.
- **`counterexample`**: the verifier found a failing case, **and the engine itself reproduced
  it**. The finding carries the request (`state`, `input`, `context`, `facts`) and the decision
  record, so you can re-run it:

```python
# from examples/outcomes.py
finding = next(f for f in attestation.findings if f["kind"] == "postcondition")
cx = finding["counterexample"]
again = evaluate(model, cx["record"]["action"]["name"], state=cx["state"], input=cx["input"],
                 context=cx["context"], data_version="verification", facts=cx["facts"])
assert again.result == "DENY"
```

- **`inconclusive`**: not decided. Its `reason` says why: for example `counterexample_not_reproduced`
  (the solver's candidate was not a real failure), a resource limit, or `encoding_error`.
  **Inconclusive is blocking.** It is never evidence that the property holds.

`attestation.verified` is true only when nothing blocking remains.

## What to do: the decision procedure

0. **Read the model against the domain text first.** Verification checks what the model says,
   not what the domain means. A rule that is too broad (a query missing its owner filter) or
   missing entirely produces no finding at all.
1. **Counterexample.** Read the counterexample as a concrete story: this state, this input, this
   step fails. Then decide whether the model allows something the domain forbids.
   - **Yes:** fix the model. Add the missing precondition, constraint, type or guard, then verify
     again.
   - **No, the domain really allows it:** the rule you wrote is wrong for the domain. Change it
     only with the domain owner's agreement, and say so. Never change it just to make
     verification pass.
   - **The domain is silent** (for example, no stated upper bound on a customer's order total):
     report the finding to the domain owner and keep it blocking. Do not invent a rule.
   - **The failing state is unreachable only because of a rule the language cannot express**
     (for example, a per-customer invariant): record the gap in `SEMANTIC_GAPS.md` with the
     finding as evidence, and keep the finding blocking.

```python
# from examples/fix_the_model.py
@action
def withdraw_guarded(account: Account, *, amount: Input[Money]):
    requires(amount > Money(Decimal("0")))
    requires(amount <= account.balance)
    set_(account.balance, account.balance - amount)
```

2. **Inconclusive.** Keep it as a blocking finding and classify it:
   - **Known precision debt** (the catalogue below): the property holds, but this release cannot
     prove it. Report it as blocking and inconclusive, with the catalogue entry that explains it.
     Do not claim it is proven, do not weaken or remove the rule, and do not ask for a verifier
     assumption.
   - **Anything else:** look for a guarantee the model relies on but does not state, and state it
     in the model: a constraint, a precondition, a type. In this release an unstated expectation
     usually shows up as a confirmed **counterexample** instead, so step 1 applies.

```python
# from examples/stated_vs_unstated.py
assert outcome([]) == "counterexample"  # unstated: state the guarantee
assert outcome([non_negative_amount]) == "inconclusive"  # stated: precision debt
```

3. **Never make a check pass by weakening it.** That means deleting an invariant, loosening a
   postcondition, excluding a check kind from the profile, or moving the rule into host code.
   Each of these hides a real problem.

## Verifying declared reads

`evaluation_error` also checks every declared read, over every valid state. Its checks have
`action` named `read:<name>`. A projection's derived values are checked for a member that passes
the query's filter, so a filter that excludes the failing case proves the read. A counterexample
holds the read record that failed.

```python
# from examples/verifying_reads.py
attestation = verify(model, Profile(checks=["evaluation_error"]))
division = {c["action"]["name"]: c["outcome"] for c in attestation.checks
            if "division by zero" in c["subject"]["name"]}
assert division == {"read:all_averages": "counterexample",
                    "read:measured_averages": "proven",
                    "read:average_ph": "counterexample"}
```

## Verifying a migration

`verify_migration(migration)` checks a migration before it touches a store. For every migrated
type it assumes a valid source entity (the source rules, plus every source requirement). It then
checks:

- every narrowing (`migration_narrowing`) and every other evaluation error (`evaluation_error`);
- every target constraint and invariant on the transformed value (`migration_constraint`);
- `referential_integrity` and target `module_invariant`s.

Outcomes are read as for actions. A counterexample names a source entity and the refusal that
running its transform gives. A proof that needs a requirement lists it in the check's `under`: it
holds for every source state the requirement admits, which the store checks when the migration is
applied. Referential integrity is proven when references are carried over unchanged. A module
invariant is proven when the source states it too and the migration copies every field it reads.
Anything else is inconclusive (precision debt) and is still checked in full on application.

```python
# from examples/verifying_a_migration.py
narrowing = next(c for c in proven.checks if c["kind"] == "migration_narrowing")
assert narrowing["outcome"] == "proven" and narrowing["under"] == ["every_ticket_has_region"]
cx = next(f for f in unproven.findings if f["kind"] == "migration_narrowing")["counterexample"]
assert cx["value"]["region"] is None and cx["refusal"]["code"] == "MIGRATION_TRANSFORM_ERROR"
check = next(c for c in debt.checks if c["kind"] == "module_invariant")
assert check["outcome"] == "inconclusive" and not debt.verified
```

The command line has `behavior migration verify <source> <target> <migration>`.

## Precision debt catalogue (release 0.10.3)

These properties can hold and still be inconclusive. They are verifier limits, not model errors.

| Pattern | Why it is inconclusive | Example |
|---|---|---|
| A query whose filter reads a field the action changes (a **changed capture**), e.g. `ensures(count(orders over customer.credit_limit) == 0)` after raising the limit | The query after the effect is treated as an unrelated set | `precision_debt.py`, `raise_limit` |
| `min_` / `max_` **after a removal or change** of a member | The next extremum is unknown | `precision_debt.py`, `remove_cheapest` |
| A property of a set that relies on an **entity constraint of its unknown members** | Constraints are used for the transition's own entities, not for unknown members of a set | `stated_vs_unstated.py` |
| Range checks inside `sum_` (partial sums of a fixed-scale type) that the model bounds | Members are summed in an order the verifier does not model | an **inconclusive** `evaluation_error` on a `sum_` |
| A uniqueness guard written as `count(select(T).where(lambda e: e.key == value)) == 0` | Only the form `not_(any_(select(T), lambda e: e.key == value))` is matched with the `unique` module invariant; write the guard that way | `hire` keeping `unique(select(Employee), by=…)` |

**`sum_` overflows are not always debt.** An unbounded sum of fixed-scale values over an unbounded
set can exceed the type's range. If that is a confirmed **counterexample**, it is real.
- **Your own arithmetic.** Compare in a form that stays in range: write
  `amount <= customer.credit_limit - total` rather than `total + amount <= customer.credit_limit`.
- **The sum itself.** Ask the domain owner whether a bound exists. If the only bound is a rule
  per group, it is a semantic gap.

A guarantee that the model **states** belongs in this catalogue. An expectation the model does
**not** state is a model problem: state it.

## Command line

`behavior verify module.json --profile profile.json --cache .behavior/verify-cache` caches results
by content. Exit codes:

| Code | Meaning |
|---|---|
| 0 | verified |
| 1 | not verified |
| 2 | the module was not admitted |
| 3 | the solver is unavailable |
