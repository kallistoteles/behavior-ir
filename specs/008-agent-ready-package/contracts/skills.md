# Contract: Skills Library

## Layout

```text
skills/                                  # consumer skills (handed to application agents)
├── README.md                            # how to use the skills of a pinned release; gap log format
├── behavior-authoring/
│   ├── SKILL.md
│   └── examples/*.py
├── behavior-verification/
│   ├── SKILL.md
│   └── examples/*.py
├── behavior-application/
│   ├── SKILL.md
│   └── examples/*.py
└── evals/                               # manual acceptance sets and rubrics (not a skill)
.claude/skills/behavior-engine-development/SKILL.md   # engine agents in this repo only
```

## SKILL.md frontmatter

```yaml
---
name: behavior-authoring
description: Model a domain as Behavior using the Python binding: entities, types, actions, lifecycle, references, queries, invariants, exact arithmetic. Use when writing or changing a behavior model.
release: 0.8.0
---
```

## Mandatory content per consumer skill

1. **Scope line:** "This skill describes Behavior release X.Y.Z. Use only what is described here
   or listed in the release's public API."
2. **The gap rule** (FR-015), verbatim in all three:
   > If the public Behavior API cannot express a requirement, record a semantic gap in
   > `SEMANTIC_GAPS.md` (format: skills/README.md). Do not work around it: no engine internals,
   > no hand-written IR, no moving the rule into host code, no silent approximation.
3. Excerpts only from `examples/*.py`, each starting with `# from examples/<file>.py`.

## Skill-specific content

| Skill | Must cover |
|---|---|
| `behavior-authoring` | the FR-012 list; the placement rules and the error codes an agent will meet (`QUERY_NOT_ALLOWED`, `NON_LOCAL_PREDICATE`, `LOSSY_CONVERSION`, `TYPE_MISMATCH` …) with the fix for each |
| `behavior-verification` | the profile and check kinds; proven, counterexample and inconclusive; a decision procedure (confirmed counterexample → fix the model; inconclusive → blocking, then classify precision debt vs. unstated expectation); the precision-debt catalogue; "never weaken a check" |
| `behavior-application` | the FR-014 host boundary; store create/evaluate/commit/replay; conflict handling; custom backends plus `run_conformance`; the consumer boundary (FR-009a) |

## Checks (automated)

| Check | Rule |
|---|---|
| examples | every `skills/*/examples/*.py` exits 0 against the installed package |
| excerpts | every fenced `python` block in a consumer `SKILL.md` appears verbatim, in order, in its cited file |
| references | every `behavior.X` / `behavior <cmd>` is in the public API manifest |
| release | the frontmatter `release` equals the release version |
| layout | `skills/` has exactly the three consumer skills; the engine skill is not under `skills/` |
| gap rule | the verbatim gap rule appears in each consumer `SKILL.md` |
