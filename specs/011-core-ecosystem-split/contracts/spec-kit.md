# Contract: Spec Kit in two repositories

## Installations (FR-032)

| | Core (`behavior-ir-core`) | Ecosystem (`behavior-ir`) |
|---|---|---|
| Spec Kit | same version (1.0.11.dev0), Claude integration, `sh` scripts | unchanged |
| Tracked | `.specify/` (minus `feature.json`), `.claude/skills/speckit-*` | same |
| Constitution | 1.0.0 → 1.0.1 (PATCH: the quality-gate section names `scripts/gates.sh`, FR-034); principles unchanged | 1.1.0: adds "Bindings and Packaging" (bindings never implement semantics; only `behavior-engine`; the exact core pin; equivalence fixtures; the release package bundles the core) and states the 500+ range |
| Extra skill | `behavior-engine-development` | consumer skills (`skills/`) |

## Numbering (FR-033, SC-012)

- Existing specs keep their numbers wherever they live:
  - core: 001–007, 009, 010, plus a reference copy of 011;
  - ecosystem: 008 and 011.
- Core: new features continue from 012. Because 011 is present, Spec Kit's automatic numbering
  gives 012. `--number 012` is still passed for the first one, for clarity.
- Ecosystem: the first new feature is created with
  `create-new-feature.sh --number 500 …` (through `/speckit-specify`). After that, Spec Kit's
  highest-plus-one gives 501, 502, and so on.
- A reference to the other repository names it, for example "core 009" or "ecosystem 500".
- Each repository's README has a "Specifications" line that states its range.

## Cross-repository features

A feature that needs both repositories is two features:

1. A core feature (012+) that ends in a Core Release.
2. An ecosystem feature (500+) whose first task updates the pin to that release.

The ecosystem spec links the core spec by repository and number.

## Gates (FR-034)

- Each constitution's quality-gate section names `scripts/gates.sh`.
- `/speckit-implement`'s completion validation runs `scripts/gates.sh`, and `*-ci.yml` runs the
  same script. A feature Spec Kit reports complete therefore passes the same checks as CI.

## Local state (FR-035)

- `.specify/feature.json` stays gitignored (`.specify/.gitignore`).
- No workflow or script used by CI reads it, `specs/`, or `.claude/`.
- `scripts/check-workflows.sh` greps `.github/workflows/*.yml` and the scripts they call for
  `.specify` and `.claude`, and fails on a match.
- `/speckit-taskstoissues` creates issues in the repository that owns the feature.
