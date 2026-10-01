# Agent acceptance results (SC-004–SC-006, US4)

**Date**: 2026-09-30 · **Release**: 0.8.0 (the wheel built by `scripts/release-build.sh`, installed by
hash into each scratch repository) · **Agents**: three Claude Code subagents, one per set, each
confined to its own scratch repository.

**Setup.** Each repository contained only:
- the pinned wheel, installed into `.venv`;
- `skills/behavior-*` and `skills/README.md` copied into `.claude/skills/`;
- the set's task files;
- an `env.sh` exporting `BEHAVIOR_Z3`.

The agents were told not to read anything outside their repository. They are an automated
stand-in for the maintainer-run evaluation; results were scored against the rubrics by hand, and
the mechanical checks were re-run independently.

## Authoring (`skills/evals/authoring/`): pass

- **Requirements 1–10 expressed.** `model.py` is admitted, and `checks.py` passes 30 checks
  (re-run independently). `check_output.py` passes: public imports only, and all gap entries
  complete.
- **Requirements 11–14 recorded as gaps**, with every field present:
  - 11: grouped invariant, with a stated partial precondition;
  - 12: cross-entity filter plus bulk effect;
  - 13: bulk effect, also noting that it conflicts with requirement 3;
  - 14: ordering.

  A fifth gap records a `sum_` overflow counterexample that only a per-customer invariant could
  rule out.
- **No violations.** There were no hand-written IR, internal imports or silent approximations.
- **Verification** was acted on as the skill says:
  - the overflow counterexamples were fixed by an overflow-safe comparison;
  - a newly found negative-limit case got a constraint, which the agent flagged for the domain
    owner;
  - the remaining findings were reported as blocking.

## Verification (`skills/evals/verification/`): pass

| Situation | Seen | Response | Rubric |
|---|---|---|---|
| 1 | preservation and evaluation_error counterexamples | added the amount guards; the constraint was kept; re-verified: all proven | pass |
| 2 | postcondition inconclusive | classified as changed-capture precision debt; kept the `ensures`. Also found the query too broad for the domain (missing the owner filter) and scoped it, and recorded the standing rule as a gap | pass |
| 3 | postcondition counterexample | stated `non_negative_amount`; re-verified: inconclusive, classified as a stated guarantee. A remaining real `sum_` overflow was reported to the domain owner and recorded as a gap, with no invented bound | pass |

## Application (`skills/evals/application/`): pass

All 8 rubric items hold. Re-run independently: 9 tests pass, including `run_conformance` on the
agent's SQLite backend (28/28 cases).
- **Rules and state.** The rules are in `model.py`, and the service holds no business `if`s.
- **Atomic commit.** It uses `BEGIN IMMEDIATE`, compares the head, then writes everything or
  rolls back.
- **Identities and time.** Ids come from uuid4, and time comes from the host.
- **Conflicts.** The service re-evaluates on `StateConflict`, with bounded retries.
- **Restarts.** It reopens with `Store.open` and replays.
- **Boundary and gaps.** Imports are public only, and one gap was recorded (a per-customer
  standing credit rule).

## Skill fixes made from the agents' reports

The following were fixed in the skills, and the skill checks were re-run. Nothing was fixed in
the rubrics.

- **Authoring:**
  - expressions and typed literals only inside behavior bodies;
  - how `Context[...]` works, with an example, and when to bind a state parameter instead;
  - `INVALID_STATE` for incoming entities that break constraints;
  - a module invariant's delta check assumes a valid starting state;
  - dividing by a plain `Decimal`;
  - `EFFECT_ON_READONLY` wording;
  - `BehaviorDefinitionError` for two-parameter rules;
  - gap rows for invariants over two related entities, and for bulk effects (`set_` on a query
    fails with `AttributeError`).
- **Verification:**
  - the attestation's check and finding shapes (`kind` is the check name), and how to list only
    the findings that fired;
  - step 0, reading the model against the domain text;
  - the domain-silent case and the gap-dependent case;
  - `sum_` overflow counterexamples are real, with the overflow-safe comparison form;
  - a new catalogue entry: a uniqueness guard written as `count(...) == 0` stays inconclusive,
    while `not_(any_(...))` matches the `unique` rule.
- **Application:**
  - `Store.open` after a restart;
  - the exact `commit_time` format, whole-second UTC `Z`;
  - `store.evaluate` raising `CommitRefused ENTITY_NOT_FOUND` for unknown ids;
  - the backend document fields;
  - key order never mattering;
  - the value formats of facts versus state;
  - plain evaluation assuming a valid state;
  - a table of decision results.

## Engine observations, recorded as follow-ups (not changed: semantics are frozen in 008)

- **Time format.** `commit_time` refuses RFC 3339 forms with fractional seconds or `+00:00`, even
  though RFC 3339 allows them.
- **Error for `set_` on a query.** It raises a plain `AttributeError` instead of a behavior error
  with a code.
- **Mixed paths.** Some attestation `loc.file` values were absolute and others relative, within
  one module built from one file.
- **Verifier normalization.** `count(select(T).where(key == v)) == 0` is not normalized onto the
  equality summary that `any_` and the `unique` delta rule share.
