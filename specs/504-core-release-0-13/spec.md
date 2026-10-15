# Feature Specification: Core Release 0.13.0 alignment

**Feature Branch**: `504-core-release-0-13`
**Created**: 2026-10-15
**Status**: Implemented
**Input**: Align the ecosystem with Core Release 0.13.0 (tag `v0.13.0`, commit
`15094972191aa0ae79705df0397c551a18745399`), successor to feature 500. No Core changes.

## Context

Core 0.13.0 is a minor release. It collects the Core remediations since 0.12.1: complete
resulting-state validity with fresh decision record 0.8 and verifier 0.9.0 (corrected outcomes
for previously uncovered invalid child states), strict profile-preserving Store replay with
original request evidence, the Store creation contract, accurate fresh-admission metadata
(accepted Wire IR 0.4–0.8), strict query-bearing Bundle Evidence validation, faithful ordinary
Facts/witness schemas and the query evidence domain contracts.

One of these fixes unblocks feature 503: under the command profile with a v2 store genesis,
Core 0.12.0 refuses its own candidate for every action whose evaluation observes a query
(`INVALID_CANDIDATE: invalid observed facts`). Core 0.13.0 accepts it (503 research R13).

## Scope decision (user, 2026-10-15): no backward compatibility

Core 0.13.0 is the authoritative contract. Implementation found two contract changes beyond the
pin (research R9, R10): fresh action evaluation requires a complete parent snapshot (Core 017),
and `Backend.create` must recover an equivalent original creation idempotently (Core 019).
Backward compatibility is **not** a requirement before the first stable release: tests,
examples, backends and documentation migrate directly to the new semantics, with no
compatibility shims, deprecated APIs, legacy fallbacks or helpers kept only for old call
patterns; obsolete code is removed rather than maintained in parallel. What must not change by
accident: determinism, canonical content identity where semantically required, and evidence
integrity. Any change of semantic meaning is a deliberate, recorded decision.

## User Scenarios & Testing

### User Story 1 - Use the 0.13.0 engine (Priority: P1)

An application installs one ecosystem package and receives Core 0.13.0, its command line and
conformance assets, with exact release identity.

**Why this priority**: Every other story depends on the pinned release; 503 cannot run its
command-profile tests without it.

**Independent Test**: Build and install the package outside the checkout; check reported
versions, the bundled command line and the conformance assets against the published release
checksums.

**Acceptance Scenarios**:

1. **Given** the published Core Release 0.13.0, **When** the ecosystem builds, **Then**
   dependency metadata, the release pin, the packaged command line and the reported core
   version and revision all name 0.13.0 / `1509497…` exactly, and the pin gate passes.
2. **Given** a release asset whose checksum differs from the pin, **When** assets are fetched,
   **Then** the fetch is refused and nothing is installed.
3. **Given** the ecosystem version reporting, **When** a consumer queries it, **Then** it
   reports the new ecosystem minor version, verifier 0.9.0, decision records up to 0.8 and
   accepted Wire IR 0.4–0.8 (previously reported as 0.1–0.8), matching what the bundled core actually reports.

### User Story 2 - Command-profile actions that observe queries commit (Priority: P1)

An application with a current-profile module (command intents, v2 genesis) invokes an action
whose guard or postcondition reads a query, and commits the permitted candidate.

**Why this priority**: This is the defect that blocks 503 at runtime.

**Independent Test**: A minimal module with one command-profile action guarded by a query
over an entity type, invoked and committed through the public engine and through the Python
binding.

**Acceptance Scenarios**:

1. **Given** a command-profile module and a v2 genesis, **When** an action whose precondition
   observes a query is invoked, **Then** an allowed candidate is produced and commits, and the
   committed state and history verify.
2. **Given** the same action whose query makes the precondition false, **When** it is invoked,
   **Then** it is refused with its recorded outcome and nothing is committed.
3. **Given** a candidate whose query evidence has been altered, **When** it is committed,
   **Then** the core refusal is surfaced and no history or state changes.

### User Story 3 - Everything migrates to the 0.13.0 contract (Priority: P2)

Tests, examples, skills and backends use the 0.13.0 contract directly: direct evaluation
supplies a complete parent snapshot, and backends implement idempotent original creation.
Semantic guarantees (outcomes on equal complete inputs, module identities, replay of
histories) are preserved; API signatures are not.

**Why this priority**: The upgrade must not silently change identities or outcomes, but old
call patterns are not preserved.

**Independent Test**: Run the full existing binding, model and conformance suites, the model
compatibility check and the replay examples against 0.13.0.

**Acceptance Scenarios**:

1. **Given** an existing authoring example or pinned model case, **When** it is built, **Then**
   its admitted module bytes and semantic identities are unchanged.
2. **Given** a stored history written under 0.12.0, **When** it is replayed, **Then** it
   replays with its recorded profile and bytes; replay grants no fresh authority.
3. **Given** a fresh action, **When** it is evaluated, **Then** its record uses the version
   0.13.0 produces for fresh actions; any expected output that changes is updated only with a
   reference to the Core change that explains it.
4. **Given** a direct evaluation without a complete parent snapshot, **When** it is
   evaluated, **Then** it is refused `INVALID_INPUT / INVALID_STATE_SNAPSHOT`; the binding
   adds no implicit completion.
5. **Given** a Python backend, **When** the 0.13.0 backend conformance suite runs (including the
   Core 019 creation cases), **Then** every case passes; a retry of the same original creation
   is an idempotent no-op and a different creation is refused.
6. **Given** a previously uncovered invalid resulting state, **When** a fresh action would
   produce it, **Then** it is refused as verifier 0.9.0 specifies, and the test says so.

### Edge Cases

- A fixture that changes under 0.13.0 for a reason not documented in the Core release notes is
  a stop condition, reported, not regenerated.
- Fresh authenticated verification with a policy not compatible with verifier 0.9.0 is refused
  by the core; the binding does not upgrade policies or historical authorizations implicitly.
- Ordinary Facts still rejects query `result_hash`; only Bundle Evidence carries it. Tests must
  not conflate the two.
- Development overrides of the core (path or patch) remain refused by the pin gate.

## Requirements

### Functional Requirements

- **FR-001**: Official builds MUST use exactly Core Release 0.13.0 for engine, command line,
  schemas and conformance assets, with checksums matching the published release and no active
  development override.
- **FR-002**: Every place that names the pinned Core release (pin file, dependency revision and
  lockfile, model compatibility fixture, user documentation and links) MUST name 0.13.0
  consistently; the existing pin and compatibility gates MUST pass.
- **FR-003**: The ecosystem version MUST receive a minor bump per the versioning policy, and the
  versioning document MUST describe the bump and the Core changes that motivate it.
- **FR-004**: Version reporting and the public capability manifest MUST describe the bundled
  release as the core reports it (verifier, record, read record and accepted Wire IR versions).
- **FR-005**: A command-profile action that observes a query MUST be invokable and committable
  through the public engine and the Python binding; a regression test MUST cover the allowed,
  refused and altered-evidence cases.
- **FR-006**: Existing module identities, model compatibility cases and replay of histories
  written under 0.12.0 MUST be preserved; every changed expected output MUST cite the Core change
  that explains it, and no historical fixture may be redefined to disguise a semantic change.
  Backward compatibility of Python API signatures, call patterns or helpers is NOT required.
- **FR-010**: Direct action evaluation (`evaluate`, `evaluate_intent`) MUST take the complete
  parent snapshot from the host as required input; the binding MUST NOT complete it implicitly.
  Every test, example and skill that evaluates directly MUST supply one (or use a Store).
- **FR-011**: Python backends (test backends, skill examples) MUST implement Core 019's creation
  contract without a compatibility layer, and the full 0.13.0 conformance suite MUST pass.
- **FR-012**: Obsolete helpers and documentation made pointless by 0.13.0 MUST be removed.
- **FR-007**: Policy selection for fresh authenticated verification MUST remain explicit; the
  binding MUST NOT invent authorization or upgrade historical trust.
- **FR-008**: The Core is not changed by this feature. A defect found in 0.13.0 is reported to
  the Core repository and recorded, not worked around.
- **FR-009**: Implementation changes MUST have observed failing tests before implementation, and
  the full required gates MUST pass before delivery; verification evidence MUST identify the
  tested revisions and any remaining limits.

### Key Entities

- **Core Release pin**: version, tag, commit, repository, asset names and checksums.
- **Ecosystem version**: the package version and its reported component versions.
- **Compatibility fixture**: pinned model cases and the core revision they were checked against.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Every official build, release asset and installed package reports the same core
  version and revision (0.13.0, `1509497…`); the pin gate passes and an override is refused.
- **SC-002**: 100% of existing binding, model and conformance tests pass; every changed expected
  output (target: zero outside fresh record versions) is listed with its Core reason.
- **SC-003**: The query-observing command-profile regression passes in all three cases through
  both the engine and the binding.
- **SC-004**: After this feature merges, feature 503 can rebase onto it and run its
  command-profile runtime tests without any workaround.

## Assumptions

- Core Release 0.13.0 is published and immutable; its assets and checksums are taken from the
  GitHub release.
- The existing Python DSL keeps its legacy profile; no new authoring syntax is part of this
  feature.
- Feature 503 stays on its own branch and is rebased after this feature merges; this feature does
  not include Approval work.
- Work happens in an isolated worktree (`behaviour-ir-504`) from `main` `47b4231`; the user's
  local Spec Kit edits in the main checkout are untouched.
