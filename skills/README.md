# Behavior skills

Agent skills for building applications with **Behavior**. Each skill describes exactly one
Behavior release. The frontmatter field `release` says which one.

| Skill | Use it when |
|---|---|
| [`behavior-authoring`](behavior-authoring/SKILL.md) | writing or changing a behavior model: entities, types, actions, lifecycle, queries, invariants, exact arithmetic |
| [`behavior-verification`](behavior-verification/SKILL.md) | running verification and acting on its results |
| [`behavior-application`](behavior-application/SKILL.md) | building the host application around a model: stores, commits, replay, backends |

`evals/` holds the maintainer's acceptance sets for these skills. It is not a skill.

## Using the skills of a release

1. **Pick a release.** The skills of a release live in this directory at the tag `vX.Y.Z` of the
   Behavior repository.
2. **Install the same release** into the application project, pinned by file and hash:

   ```text
   # requirements.txt
   behavior @ file:///path/to/behavior-X.Y.Z-cp313-abi3-manylinux_2_28_x86_64.whl --hash=sha256:<from SHA256SUMS>
   ```

   Then run `pip install --require-hashes -r requirements.txt`. No Rust toolchain is needed.
   Verification also needs the Z3 SMT solver at the version the release names (`z3` on PATH, or
   `BEHAVIOR_Z3`).
3. **Copy** `behavior-authoring/`, `behavior-verification/` and `behavior-application/` from that
   tag into the project's agent skills directory (for example `.claude/skills/`). There is no
   installer.
4. **Check the match.** `python -c "import behavior; print(behavior.versions())"` must report the
   same release as the skills' `release` field. If they differ, stop and fix the pin; do not mix
   releases.

## The consumer boundary

An application uses Behavior **only through the released package**: the names exported by the
`behavior` module and the `behavior` command line of its release. It never imports engine
internals or depends on paths inside the Behavior repository. A local checkout of Behavior is a
contributor's workflow, never the way an application depends on it.

## The semantic gap rule

Every skill carries this rule:

> If the public Behavior API cannot express a requirement, record a semantic gap in
> `SEMANTIC_GAPS.md` (format: skills/README.md). Do not work around it: no engine internals,
> no hand-written IR, no moving the rule into host code, no silent approximation.

Gaps are the most valuable output of an application built on Behavior: they are the input for
the language's next features. Recording one is success, not failure.

### `SEMANTIC_GAPS.md` format

Keep one file at the application's root. It has one section per gap, with fixed fields, so
entries from many projects can be collected mechanically:

```markdown
# Semantic gaps

## GAP-001: At most three open orders per customer

- **Date**: 2026-10-02
- **Behavior release**: 0.8.0
- **Requirement**: A customer may have at most three open orders at any time.
- **Why inexpressible**: needs a grouped module invariant (a count per customer over all
  customers); module invariants are closed and queries cannot group. Closest construct: a
  precondition `count(orders of customer, open) < 3` on `place_order`, which covers only that
  action.
- **What was done instead**: the precondition on `place_order`; the risk is that other actions
  that open orders are not covered.
- **Severity**: degraded
- **Evidence**: a per-customer invariant draft (`@invariant def f(c: Customer)` counting that
  customer's open orders) was rejected with `QUERY_NOT_ALLOWED`, because queries are not allowed
  in per-entity invariants.
```

Rules:

- **IDs** increase and are never reused. Entries are never deleted; a resolved gap gets a line
  `- **Resolved**: <release> …`.
- **Severity** is one of:
  - `blocking`: the requirement cannot be met;
  - `degraded`: met only partially or outside Behavior;
  - `cosmetic`: expressible, but awkwardly.
- **What was done instead** is `nothing`, unless a host-side interim was explicitly reviewed and
  its risk is stated. An unmentioned workaround is never acceptable.
