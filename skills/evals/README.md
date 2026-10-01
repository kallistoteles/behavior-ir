# Acceptance evaluations for the consumer skills

These sets are for the **maintainer**. They check whether an agent that has only a pinned release
and the three consumer skills behaves as the skills intend (feature 008, SC-004–SC-006). They
are run by hand, never by the automated test suite, because no automated test may call a live
model.

How to run one:

1. Create a scratch application repository. Install the release's wheel pinned by hash, and copy
   `skills/behavior-*` from the same release tag into its agent skills directory.
2. Give the agent the set's task file and nothing else from this repository.
3. Score the result with the set's `rubric.md`. Where the set has a `check_output.py`, run it on
   the agent's output files; it checks the mechanical parts.
4. Record the scores in `specs/008-agent-ready-package/evals/results.md`. When a failure comes
   from the skill, fix the skill, not the rubric, and run the set again.

| Set | Measures |
|---|---|
| `authoring/` | expressible requirements modelled correctly; inexpressible ones recorded as gaps (SC-004, SC-005) |
| `verification/` | responses to seeded verification outcomes (SC-006) |
| `application/` | the host boundary in an agent-built service (US4) |
