# Feature Specification: Core and Ecosystem Repositories

**Feature Branch**: `011-core-ecosystem-split`

**Created**: 2026-10-03

**Status**: Draft

**Input**: User description: the "Behavior Repository & Ecosystem Architecture" specification
(conversation of 2026-10-03). Behavior separates its semantic core from its authoring and
developer ecosystem. **Core defines meaning; everything else defines ways to author and use that
meaning.** The core repository is `git@github.com:kallistoteles/behavior-ir-core.git`, cloned next
to this one (`../behavior-ir-core`).

## Context

Behavior has one repository today (`behavior-ir`). It holds both the deterministic semantic
kernel (wire IR, admission, hashing, evaluation, reads, queries, lifecycle, migrations,
verification, the persistence contract, replay, governance) and everything built on top of it
(the Python binding and its authoring DSL, examples, consumer skills, release packaging).

The kernel's fundamental work has stabilized: schema evolution (009) and first-class reads (010)
are done, and the runtime model is complete except for general invocation and command intents.
The boundary is already mostly respected in code. The engine never depends on the Python layer,
and the binding uses the engine's public API. It is not enforced, though, and nothing would stop
the two from growing together.

This feature makes the boundary the architecture. It creates two repositories with independent
identities, a one-way dependency, a public core contract and conformance fixtures that every
binding is checked against. Behavior IR becomes the interface between innovation above it and
stability below it:

```text
HIGHER-LEVEL MODELS      state machine / workflow / …        (behavior-ir)
        ↓ deterministic lowering
BINDINGS                 Python / TypeScript / Java / …     (behavior-ir)
        ↓
BEHAVIOR IR              the semantic assembly language
        ↓ admission
BEHAVIOR CORE            deterministic semantic authority    (behavior-ir-core)
```

**Terminology** (used throughout):

- **Behavior Core**: defines the fundamental semantics.
- **Binding**: gives a programming language access to those semantics; it never implements them.
- **Model**: a higher-level modeling abstraction (state machine, workflow, approval) that lowers
  completely to ordinary Behavior IR. Models are not "plugins" or "extensions": they compile
  downward and never extend runtime semantics.
- **Adapter**: connects Behavior to external infrastructure (a database backend, a message
  queue, a command executor). It never redefines semantics.

## Clarifications

### Session 2026-10-03

- Q: Should this feature physically move the core into `behavior-ir-core` now, or first enforce
  the boundary inside one repository? → A: Split now, as a pure **extraction**: no semantics or
  public contracts are redesigned as part of the move. The core's Git history is preserved. A
  short **preflight**, not a separate boundary phase, comes first: the core has no dependency on
  bindings or models, the public contract the binding needs is identifiable, every semantic
  specification and fixture has an obvious owner, the core builds and tests on its own, and the
  ecosystem can consume the core through its intended public surface. Any failing criterion is
  fixed alone before the move. Versioning starts simple: the ecosystem pins one exact core
  release (or tag); compatibility ranges come only after the boundary has survived several
  releases. The ecosystem's continuous integration builds against a released core artifact
  outside any core checkout; local development may use a core source path, but never as the only
  working consumer path. Documents follow semantic ownership: *if changing a document could
  change what a Behavior program means, it belongs to the core.*
- Q: How should the ecosystem obtain a released core to build its Python extension against? →
  A: A **Core Release** is an immutable Git release: a version tag and the exact commit it
  identifies. The ecosystem records the human-facing core version and pins its build dependency
  to the exact core commit, which its lockfile records. The ecosystem depends only on the core's
  explicitly supported **public Rust crate(s)**, a deliberate public surface such as a single
  engine crate, never on arbitrary internal workspace crates. Core continuous integration defines
  and tests that surface before a release is tagged. Ecosystem release CI builds against the
  pinned release without a core checkout or path dependencies; local path overrides are a
  development convenience only. Publishing to a public crate registry is a later distribution
  improvement that does not change the boundary. Principle: *repository separation is not the
  boundary; the public Core API is.*
- Q: Should the public Rust surface be one new facade crate, or the existing crates the binding
  uses today? → A: One facade crate, `behavior-engine`, the only supported programmatic Rust API
  of the core. Bindings and the ecosystem depend directly on it alone; `behavior-core`,
  `behavior-store`, `behavior-verify` and every other workspace crate are implementation details
  that may be reorganized freely. For the extraction it uses **explicit** re-exports of the
  existing items bindings need (no API redesign); wildcard re-exports are forbidden, so every
  exported item is intentional. The command-line tool is not part of the facade: the CLI and the
  bindings are sibling consumers of `behavior-engine`, and engine functionality the binding
  reaches through the CLI today is exposed at the engine boundary, while CLI parsing, formatting,
  exit codes and process behavior stay in the CLI. An external-consumer build whose only core
  dependency is `behavior-engine` keeps the boundary machine-checked. Principle: *core may have
  many internal modules, but it has one public programmatic door.*
- Q (added by the author): How are validation and releases automated after the split? → A: Each
  repository gets its own GitHub Actions workflows (`core-ci`, `core-release`; `ecosystem-ci`,
  `ecosystem-release`). Actions only orchestrate: the release checks and artifact construction
  stay in repository scripts that run identically locally and in CI. The existing release
  strategy (immutable annotated `v<version>` tags on commits that passed `release-check`,
  `release-build`, `release`, the versioning policy) is migrated and adapted, not reinvented.
  Releases are explicit tags, never produced by ordinary merges; a tag is never moved, and a
  broken release is replaced by a new patch version. The ecosystem has its own version from the
  start (initially in step with the core) and states the core it requires by version and
  revision.
- Q (added by the author): Does this fit the GitHub Spec Kit workflow the project uses? → A: Yes,
  with explicit rules: each repository has its own Spec Kit installation and constitution; the
  feature sequence continues across both repositories without reusing a number; CI never depends
  on Spec Kit's machine-local state; the quality gates the constitution names are the same
  scripts in Spec Kit's implement phase and in CI.

### Session 2026-10-04

- Q: How should new feature numbers stay unique across the two repositories after the split? →
  A: Separate number ranges. The core continues from 012 (011 is this feature); the ecosystem's
  new features start at 500. Existing specifications keep their numbers. Spec Kit's sequential
  numbering then continues each range by itself; only the first new feature in each repository is
  created with an explicit number (`--number 012` in the core, `--number 500` in the ecosystem).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The core stands alone (Priority: P1)

A maintainer, an auditor or a team building a binding in another language works only with the
core repository. From it alone they can build the engine, read the specification of every
semantic form, admit and evaluate any Behavior IR document, verify it, run a store against the
persistence contract, replay histories, and run the complete semantic conformance suite.

**Why this priority**: The core invariant. If the ecosystem repository disappeared, the core must
still define and implement Behavior completely and unambiguously.

**Independent Test**: Clone only `behavior-ir-core` into an empty environment. It builds. Its
tests and conformance suite pass. Its command-line tool admits, evaluates, reads, verifies and
replays the conformance fixtures. No file or build step refers to the ecosystem repository.

**Acceptance Scenarios**:

1. **Given** a fresh clone of the core alone, **When** it is built and tested, **Then** every
   semantic test and every conformance fixture passes without the ecosystem repository.
2. **Given** any Behavior IR document the ecosystem produces, **When** the core's command-line tool
   admits and evaluates it, **Then** the result is identical to the result through a binding.
3. **Given** the core's dependency manifests and sources, **When** they are scanned, **Then**
   nothing refers to a binding, a model, an adapter or the ecosystem repository.

---

### User Story 2 - The ecosystem uses only the public core contract (Priority: P1)

A developer works in the ecosystem repository (`behavior-ir`): the Python binding and its
authoring DSL, examples, consumer skills and release packaging. It depends on a released,
exactly pinned version of the core and uses only that version's public contract: the published
library interfaces, the wire, record and store document formats, and the command-line tool.

**Why this priority**: The architectural test: *can everything in the ecosystem be implemented
purely through Behavior Core's public contract?* The required answer is yes.

**Independent Test**: In the ecosystem repository, the build resolves the core only as the pinned
release. Every test, example and skill check passes. A check fails if the ecosystem reaches into
anything the core does not export publicly.

**Acceptance Scenarios**:

1. **Given** the ecosystem repository, **When** it is built, **Then** the core comes from exactly
   the pinned release, never from a path into the core's sources.
2. **Given** a use of a core element that is not in the core's public contract, **When** the
   ecosystem's checks run, **Then** they fail and name it.
3. **Given** the pinned core version is changed to one the ecosystem release does not declare,
   **When** the ecosystem is built or imported, **Then** it refuses with both versions named.

---

### User Story 3 - One semantic truth across authoring paths (Priority: P2)

Behavior's meaning must not depend on how a definition was written. Equivalent definitions,
whether written through the Python binding, as hand-built wire IR in conformance fixtures, or
later through other bindings and models, resolve to identical admitted Behavior IR and therefore
identical semantic hashes.

**Why this priority**: It is what makes multiple bindings and models safe. Without it, each
authoring path could drift into its own dialect of Behavior.

**Independent Test**: Cross-authoring equivalence fixtures pair a definition in the Python DSL
with the same definition as wire IR. Both admit to the same behavior version and item hashes, in
the ecosystem's continuous integration.

**Acceptance Scenarios**:

1. **Given** an equivalence fixture, **When** its Python form and its wire form are admitted,
   **Then** the behavior versions and every item hash are identical.
2. **Given** a binding change that alters the IR it emits for an equivalence fixture, **When**
   continuous integration runs, **Then** it fails and names the fixture and the differing item.

---

### User Story 4 - Users install one thing (Priority: P2)

An application developer installs the released Behavior package as before. They get the Python
binding, the compatible core engine, the command-line tool and the guidance, without installing
or knowing about the core separately. The package states which core version it contains.

**Why this priority**: The repository separation must not become installation complexity for
users.

**Independent Test**: Install the ecosystem's released package in a clean environment. Run the
release smoke scenario and the skill examples. Ask the package for its versions: it reports its
own release and the exact core release it bundles.

**Acceptance Scenarios**:

1. **Given** a clean environment, **When** the released package is installed, **Then** the smoke
   scenario and every skill example run without any separate core installation.
2. **Given** the installed package, **When** its versions are listed, **Then** they name the
   ecosystem release and the exact core release, and the command-line tool reports the same core
   version.

---

### User Story 5 - Deciding where a new concept belongs (Priority: P3)

A maintainer or an agent proposes a new concept, such as a state machine, a workflow, a CRUD
convenience API or command intents. The architecture documents give one question that decides
where it belongs: *must the evaluator understand this construct for its semantics to be
correct?* If not, it is a model, library or tooling in the ecosystem and must lower completely to
existing Behavior IR. If so, it is a candidate core primitive, decided explicitly and never
hidden in a model.

**Why this priority**: It keeps the kernel small, explicit and conservative while the ecosystem
grows quickly. No model exists yet, so this story sets the rule before the first one arrives.

**Independent Test**: The architecture document states the criterion, the classification table
and the rules for models. The project principles and the engine and consumer guidance refer to
it. A worked example shows a state-machine transition lowered to ordinary preconditions,
effects and postconditions, admitted by the core with no knowledge of its origin.

**Acceptance Scenarios**:

1. **Given** the classification table, **When** a reviewer classifies state machines, workflows,
   approval flows, entity identity, option semantics, exact arithmetic, queries, migrations,
   persistence and replay, command intents and visualization, **Then** each has exactly one
   layer, and it matches the table.
2. **Given** a model construct that cannot be lowered faithfully to Behavior IR, **When** it is
   proposed, **Then** the documented procedure requires redesigning it on core semantics or
   proposing a core primitive; hidden model-specific runtime behavior is not an option.

### User Story 6 - Validation and releases run the same way locally and in CI (Priority: P2)

A maintainer opens a pull request in either repository and CI runs that repository's gates. When
the core is ready, the maintainer tags `v<version>` on a green commit; the release workflow
validates the tag, reruns every release gate, builds the artifacts, the manifest and the
checksums, and publishes a GitHub Release. The ecosystem release works the same way and records
the exact core it bundles. Everything CI does can be run locally with the repository's scripts,
and Spec Kit's implement phase uses the same gates.

**Why this priority**: The repository boundary is only real if something checks it on every
change, and a release is only trustworthy if it is reproducible outside the CI service.

**Independent Test**: Open a pull request that breaks a gate in each repository: CI fails on
it. Push a tag whose version differs from the declared release version: the release workflow
refuses before building anything. Run each repository's release check locally on a release
commit: it gives the same verdict and the same checksums as the release workflow.

**Acceptance Scenarios**:

1. **Given** a pull request to the core, **When** CI runs, **Then** it runs format, lint, unit
   and integration tests, the semantic conformance fixtures, the determinism check, the
   verification and persistence/replay tests, and the external `behavior-engine` consumer build.
2. **Given** a pull request to the ecosystem, **When** CI runs, **Then** it resolves the pinned
   core release (no core checkout or path dependency), builds the binding, runs model lowering
   and equivalence fixtures, builds the package, installs it in a clean environment and runs the
   smoke and conformance tests.
3. **Given** a merge to a protected branch, **When** it completes, **Then** no release is
   created; only an explicit tag starts a release.
4. **Given** a release tag whose version does not match the repository's declared release
   version, **When** the release workflow starts, **Then** it refuses and names both versions.

---

### Edge Cases

- **History.** The core keeps its history through a history-preserving extraction (FR-020); the
  ecosystem repository keeps its full history. Nothing is lost from either.
- **Shared fixtures.** Conformance fixtures belong to the core. The ecosystem consumes them from
  the pinned core release and never keeps a diverging copy.
- **Development across both repositories.** A change needing both (a new core form plus its DSL
  syntax) is made in the core first, released or tagged, then adopted by the ecosystem through
  the pin. A local core source path may be used while developing, but the ecosystem's continuous
  integration and release path always build against a released core artifact (FR-024).
- **A preflight criterion fails.** The one failing criterion is fixed and released on its own;
  the move does not start until all criteria hold (FR-022).
- **Version skew.** An ecosystem release with a core it does not declare refuses explicitly. It
  never runs with silently different semantics.
- **The engine guidance.** The skill for developing the engine moves with the core; the consumer
  skills stay with the ecosystem.
- **Release artifacts.** The core publishes its own artifacts (libraries, the command-line tool,
  the conformance fixtures, the format schemas). The ecosystem's package bundles a compatible
  engine and keeps its byte-identity, smoke and drift checks.
- **Existing documents.** No module, record, store document, hash vector or golden changes bytes
  or identity because of the move.
- **A broken release.** Its tag is never moved or deleted; a new patch release replaces it.
- **CI unavailable.** A release can still be checked and built locally with the repository's
  scripts and gives the same artifacts and checksums; CI adds orchestration, not authority.
- **Spec numbering across repositories.** A feature number is never reused in either
  repository: the core's range continues from 012 and the ecosystem's starts at 500. Existing
  features keep their numbers; a reference to a feature in the other repository names that
  repository (for example, "core 009").
- **A feature that needs both repositories.** It is specified and implemented as a core feature
  first (in the core's Spec Kit) and adopted by an ecosystem feature that pins the resulting core
  release, mirroring the release order.
- **Spec Kit's local state.** The pointer to the active feature is machine-local and ignored by
  Git; no CI workflow reads it or any other Spec Kit state.

## Requirements *(mandatory)*

### Functional Requirements

**Two repositories and their ownership**

- **FR-001**: Behavior MUST consist of two repositories with independent identities:
  `behavior-ir-core`, the deterministic semantic kernel, and `behavior-ir`, the ecosystem.
- **FR-002**: The core MUST own all of Behavior's semantics:
  - the Behavior IR, type system, admission, canonicalization and content hashing;
  - expression, query, read, transition, entity lifecycle and schema migration semantics, and
    external command semantics when they arrive;
  - evaluation, verification, the persistence contract, replay and governance/evidence;
  - the wire, record and store document formats and their schemas;
  - the semantic conformance tests and fixtures.
- **FR-003**: The core MUST be sufficient to determine the meaning of any admitted Behavior IR
  document without access to the ecosystem repository. Its tests, conformance suite and
  command-line tool MUST run from the core alone.
- **FR-004**: The ecosystem MUST contain the developer-facing layer: the language bindings (the
  Python binding and DSL first), higher-level models (none yet; an area reserved for them),
  examples, consumer skills, documentation of authoring and use, tooling and release packaging.

**Dependency direction and the public contract**

- **FR-005**: Dependencies MUST be one-way: models and bindings depend on the core; bindings may
  depend on models. The core MUST NOT depend on, refer to or know about any binding, model or
  adapter. This MUST be checked automatically in the core.
- **FR-006**: The core MUST define and publish its **public contract**: the document formats,
  schemas, the command-line interface, and one public Rust crate, **`behavior-engine`**, the
  only supported programmatic API. Every other core crate is internal and may be reorganized
  without affecting the ecosystem. It is versioned with the core, and core continuous integration
  checks it.
- **FR-006b**: `behavior-engine` MUST export each item explicitly (no wildcard re-exports), and
  for this extraction it MUST re-export the existing items that bindings need rather than
  redesigning them (FR-021). It covers what the ecosystem does: build and admit modules, evaluate
  and read, use stores, migrate, verify, authorize, and report versions.
- **FR-006c**: The command-line tool MUST NOT be part of the `behavior-engine` contract. The CLI
  and the bindings are sibling consumers of `behavior-engine`. Engine functionality that the
  binding reaches through the CLI today MUST be exposed at the engine boundary; command-line
  parsing, formatting, exit codes and process behavior stay internal to the CLI.
- **FR-006d**: Core continuous integration MUST build an external consumer whose only core
  dependency is `behavior-engine` (from the release's Git revision) and that exercises every
  capability of FR-006b.
- **FR-006a**: A **Core Release** MUST be an immutable Git release: a version tag on the exact
  commit it identifies. Before tagging, core continuous integration MUST pass its tests, the
  semantic conformance fixtures, the determinism check, the wire, verification, persistence and
  replay fixtures, and the public-surface check. A release states its version, commit, engine
  version, the wire and record format versions it supports, its verifier version and its public
  Rust surface.
- **FR-007**: The ecosystem MUST use only the core's public contract: its only direct core
  dependency is `behavior-engine`, never an internal core crate, and its code and documentation
  name items through `behavior_engine`. A check MUST fail, naming the element, when the ecosystem
  uses anything else.
- **FR-008**: Bindings MUST NOT reimplement typing, evaluation, hashing, verification or any other
  core semantics. They provide authoring syntax and access, and every semantic decision comes from
  the core.

**Models**

- **FR-009**: A model MUST lower deterministically and completely to ordinary Behavior IR, which
  the core admits without knowing the model. A model MUST NOT introduce runtime semantics unknown
  to the core.
- **FR-010**: Each model MUST have one authoritative normalization, validation and lowering
  definition, shared by every binding's syntax for it, never reimplemented per binding.
- **FR-011**: A model construct that cannot be lowered faithfully MUST be either redesigned on
  existing core semantics or proposed explicitly as a core primitive.
- **FR-012**: A model MAY have its own canonical representation and content identity. The Behavior
  module hash remains authoritative for runtime semantics, and an audit can keep both.
- **FR-013**: The architecture documentation MUST state the core admission criterion and the
  classification table, and MUST avoid "plugin" and "extension" for models.

**Equivalence and conformance**

- **FR-014**: The core MUST maintain conformance fixtures for admission, hashing, evaluation,
  reads, verification, records, replay, persistence and migrations, usable by any binding.
- **FR-015**: The ecosystem's continuous integration MUST check, at minimum:
  - every binding builds against the pinned core;
  - generated IR is admitted by the core;
  - cross-authoring equivalence fixtures give identical IR and hashes;
  - only the public core contract is used;
  - every example and skill works through released, public interfaces;
  - every model lowers successfully, once models exist.
- **FR-016**: Equivalent definitions through different authoring paths MUST give identical
  admitted Behavior IR and semantic hashes, checked by the equivalence fixtures.

**Releases and versions**

- **FR-017**: The core and the ecosystem MUST have independent release identities and MAY have
  independent version numbers. Each ecosystem release MUST declare exactly the core release it
  supports, by its version (for humans) and its commit (the build pin, recorded in the
  lockfile). Compatibility ranges are introduced only after the boundary has survived several
  releases; publishing to a public crate registry likewise comes later and does not change the
  boundary.
- **FR-018**: The ecosystem's user-facing package MUST bundle the compatible core engine, so users
  install one package. It MUST report both its own release and the core release it contains.
- **FR-019**: Moving code MUST NOT change the bytes or identity of any existing module, record,
  store document, hash vector, golden file or attestation format.
- **FR-020**: The core's Git history MUST be preserved by a history-preserving extraction: the
  moved sources, specifications and fixtures keep their commits as far as Git allows.

**The move**

- **FR-021**: The split MUST be an extraction only. It MUST NOT intentionally change Behavior
  semantics or any public contract; any change that turns out to be required is made and
  released separately, before or after the move.
- **FR-022**: A preflight MUST establish, before the move, that:
  - the core has no dependency on bindings or models;
  - the public contract the binding needs is identifiable;
  - every semantic specification and fixture has an obvious owner;
  - the core builds and tests on its own;
  - the ecosystem can consume the core through its intended public surface.
  A failing criterion is fixed on its own before the move.
- **FR-023**: Documents and specifications MUST be placed by semantic ownership: a document that
  could change what a Behavior program means belongs to the core (IR, types, evaluation,
  verification, persistence, lifecycle, queries, reads, schema evolution, governance, wire
  contracts); authoring APIs, models, application guides, agent skills, examples and integration
  guides belong to the ecosystem.
- **FR-024**: The ecosystem's continuous integration MUST build and test against the pinned core
  release, without a core checkout or path dependency: resolve the pinned core commit, build the
  native extension and the package, install it in a clean environment, and run the binding,
  conformance and example tests. A local core source path MAY be used as a development override,
  never as the only working way to consume the core.

**Continuous integration and releases**

- **FR-025**: Each repository MUST run CI on every pull request and on every update of a
  protected branch (the main and integration branches).
- **FR-026**: Core CI MUST run at least: format and lint checks, unit and integration tests, the
  semantic conformance fixtures, the determinism check, the verification and persistence/replay
  tests, and the external `behavior-engine` consumer build (FR-006d).
- **FR-027**: Ecosystem CI MUST run at least: resolve the pinned core release without a core
  checkout or path dependency, build the bindings, run model lowering (once models exist) and
  the equivalence fixtures, build the distributable package, install it in a clean environment,
  and run the smoke, conformance, example and skill checks (FR-015, FR-024).
- **FR-028**: Releases MUST be explicit: an annotated `v<version>` tag on a commit with green
  CI. Ordinary merges MUST NOT create releases. A tag MUST never be moved or deleted; a broken
  release is replaced by a new patch version.
- **FR-029**: A release workflow MUST validate that the tag equals the repository's declared
  release version, rerun every release gate, build the artifacts, produce the release manifest
  and checksums, and publish a GitHub Release. A core release publishes at least the manifest,
  the checksums, the format schemas and the command-line tool; an ecosystem release publishes
  the user-facing package and states the exact core version and revision it bundles.
- **FR-030**: GitHub Actions MUST only orchestrate: the release checks and artifact construction
  MUST live in repository scripts that run identically locally, and a release checked locally
  MUST give the same verdict and checksums as the release workflow. The existing release
  scripts and versioning policy are migrated to the two repositories and adapted, not replaced.
- **FR-031**: Each repository MUST have separate workflows for validation and for release (core:
  `core-ci`, `core-release`; ecosystem: `ecosystem-ci`, `ecosystem-release`), not one combined
  workflow.

**Spec Kit in two repositories**

- **FR-032**: Each repository MUST have its own Spec Kit installation (scripts, templates,
  commands) and its own constitution. The core keeps the current constitution's principles, amended only to name its gate
  script (FR-034); the ecosystem's
  constitution is derived from it with the binding and packaging rules, amended through Spec
  Kit's constitution command.
- **FR-033**: Feature numbers MUST stay unique across both repositories through separate ranges:
  new core features continue from 012, new ecosystem features start at 500, and existing
  specifications keep their numbers wherever they move. Each repository documents its range, and
  references to the other repository name it (for example, "core 009").
- **FR-034**: The quality gates the constitution requires (tests, format and lint, the
  determinism check) MUST be the same repository scripts in Spec Kit's implement phase and in
  CI, so a feature that Spec Kit reports complete passes CI.
- **FR-035**: No CI or release workflow MAY depend on Spec Kit's machine-local state (the active
  feature pointer) or on any agent tooling; Spec Kit artifacts in `specs/` are documentation and
  are versioned with the repository that owns them. Tasks converted to GitHub issues go to the
  repository that owns the feature.

### Key Entities

- **Behavior Core** (`behavior-ir-core`): the semantic authority. It holds the engine, its
  specification, document formats, conformance fixtures, the command-line tool and the guidance
  for developing it.
- **Ecosystem** (`behavior-ir`): bindings, models, examples, consumer skills, docs, tooling and
  the user-facing package.
- **Public core contract**: the versioned set of formats, schemas, command-line commands and the
  public Rust crate `behavior-engine` that the ecosystem may use. The public Core API, not the
  repository separation, is the boundary.
- **`behavior-engine`**: the core's one public programmatic door: explicit re-exports of the
  supported items; the CLI and every binding consume it as siblings.
- **Core Release**: an immutable version tag on one exact commit, with its stated format,
  verifier and engine versions and public surface; the ecosystem pins the commit.
- **Binding**: authoring syntax and access in one host language.
- **Model**: a higher-level abstraction with one lowering to Behavior IR, and optionally its own
  content identity.
- **Adapter**: a connection to external infrastructure that never redefines semantics.
- **Conformance fixture**: a core-owned input with an expected result that any binding or model
  must reproduce.
- **Equivalence fixture**: the same definition in two authoring forms, with one expected IR and
  hash set.
- **Workflow**: a GitHub Actions definition that orchestrates a repository's scripts, for
  validation (CI) or for releasing; never the authority on what a release is.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The core repository, cloned alone into an empty environment, builds and passes 100%
  of its tests and conformance fixtures, with zero references to the ecosystem repository.
- **SC-002**: The ecosystem repository passes 100% of its tests, examples and skill checks
  against the pinned core release, and contains zero uses of core elements outside the public
  contract.
- **SC-003**: Every existing module, record, store document, hash vector, golden and attestation
  format keeps its bytes and identity across the move (100%).
- **SC-004**: All equivalence fixtures (at least one per wire IR form: entities and types,
  actions and lifecycle, queries and module invariants, exact arithmetic, migrations, reads)
  produce identical behavior versions and item hashes from the Python and wire forms.
- **SC-005**: A user installs the released package with one install command and runs the smoke
  scenario and every skill example with no separate core installation, as today.
- **SC-006**: The determinism check passes in both repositories.
- **SC-008**: The ecosystem's continuous integration passes with the core taken only from a
  released artifact, in an environment with no core checkout.
- **SC-009**: The move itself changes no semantics: every conformance fixture, golden record and
  hash vector gives identical results before and after it (100%).
- **SC-010**: In each repository, a pull request that breaks any required gate is reported as
  failing by CI (checked once per gate with a deliberately broken change).
- **SC-011**: A release built locally from the release commit and the published release have
  identical checksums for every artifact.
- **SC-012**: After the split, new features receive numbers in their repository's range (core
  012 onward, ecosystem 500 onward) and never one used in the other repository; after the first
  feature in each repository, Spec Kit assigns them without an explicit number.
- **SC-007**: A reviewer can classify each of the 12 concepts in the classification table in
  under one minute each using only the architecture document, and arrives at the documented
  layer.

## Assumptions

- **Repositories.** `behavior-ir-core` (empty but for a license and a README, cloned to
  `../behavior-ir-core`) becomes the core. This repository (`behavior-ir`) becomes the ecosystem.
  The architecture text's names `behavior-core` and `behavior` refer to these two.
- **Split now** (clarified): the physical split happens in this feature, as an extraction after a
  preflight (FR-021, FR-022). Schema evolution (009) and first-class reads (010) are done; the
  wire and runtime formats and schemas already exist; the public core contract (FR-006) is
  identified in the preflight, not redesigned.
- **What moves to the core** (by semantic ownership, FR-023): the engine crates (core, verifier,
  store, command-line tool), the format schemas, the conformance fixtures, the semantic
  specifications (001–007, 009, 010), the constitution, `PRINCIPLES.md`, the semantic documents
  (persistence, verification, versioning of formats) and the engine-development guidance. The
  release-packaging specification (008) concerns the user-facing package and stays with the
  ecosystem.
- **What stays in the ecosystem:** the Python binding and DSL, examples, consumer skills and
  their evaluation material, user documentation, release packaging and the public API manifest of
  the user-facing package. The ecosystem has its own copy of the constitution and refers to the
  core's principles.
- **The Python native extension** is part of the binding and stays in the ecosystem, building
  against the pinned core release.
- **The command-line tool** belongs to the core: it is how the core is used without any binding,
  built on `behavior-engine` like a binding. The user-facing package keeps shipping the
  `behavior` command (FR-018), as a bundled core tool rather than through the binding's API.
- **Versions.** The first core release keeps the current version line (0.10.x) and is tagged on
  its exact commit. The ecosystem pins that commit through a Git dependency; independent version
  numbers, compatibility ranges and a public crate registry are allowed later (FR-017).
- **Not a process boundary.** Bindings link the core's public Rust surface. A wire-only binding
  driving a core process is not adopted: it would cost the typed builder integration the Python
  binding relies on. The wire formats remain the interchange boundary.
- **No models yet.** The models area is created empty, apart from the documented rules and the
  worked state-machine lowering example (US5). The first real model is a later feature, driven by
  an application.
- **Roadmap.** General invocation (planned as 011) and command intents follow this feature, in
  the core repository.
- **Existing release machinery.** `docs/versioning.md` and the scripts `release-build`,
  `release-check`, `release` and `determinism-check` already define immutable tags, release
  checks and artifacts; they are split between the repositories and adapted. GitHub Actions is
  new: neither repository has workflows today.
- **Spec Kit numbering** (clarified): Spec Kit's sequential numbering scans only its own
  repository's `specs/` and takes the highest number plus one. With separate ranges, the first
  new feature in each repository is created with Spec Kit's existing `--number` option (012 in the
  core, 500 in the ecosystem); later ones follow automatically. Spec Kit itself is not changed.
  Both repositories use the same Spec Kit version and the Claude integration.
- **Out of scope:** new semantics, additional bindings (TypeScript, Java, .NET), adapters beyond
  the existing backends, renaming either GitHub repository, publishing to crate or package
  registries, and changes to Spec Kit itself.
