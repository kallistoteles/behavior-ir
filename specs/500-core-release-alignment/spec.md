# Feature Specification: Current Core Release alignment

**Feature Branch**: `500-core-release-alignment`
**Created**: 2026-10-06
**Status**: Implemented and verified
**Input**: Bring behavior-ir up to date with the current Behavior Core.

## User Scenarios & Testing

### User Story 1 - Use the current released engine (Priority: P1)

An application installs one ecosystem package and receives the current published core,
its command line and conformance assets, with precise release identity.

**Why this priority**: A current dependency is the prerequisite for current capabilities.
**Independent Test**: Install the built package outside the checkout; check its release
identity, existing examples and byte-identical conformance outputs.

**Acceptance Scenarios**:
1. Given a validated published Core Release, when the ecosystem builds, then dependency
metadata, packaged executable and reported core revision agree exactly.
2. Given an existing authoring example, when it is built and replayed, then its historical
semantic identity and supported output remain unchanged.
3. Given an old required-governance history, when a new write is attempted, then the current
core's explicit upgrade refusal is surfaced and no history or state is changed.

### User Story 2 - Access current invocation and commands (Priority: P2)

An application imports an admitted behavior document, invokes either a read or transition,
inspects its evidence, commits a permitted candidate and streams durable command occurrences.

**Why this priority**: New semantics must be accessible without hidden host evaluation.
**Independent Test**: Use the release's frozen invocation and command fixtures through the
binding and packaged command line; compare canonical records and identities.

**Acceptance Scenarios**:
1. Given equivalent release fixture inputs, when either authoring path invokes them, then
admitted behavior, records and hashes equal the core's expected results.
2. Given malformed, inconsistent or unauthorized inputs, when invocation is attempted, then
the exact core refusal/error is returned without a commit or external effect.
3. Given duplicate command emissions and a committed history, when requests are paged, then
whole-event ordering, duplicate multiplicity and stable occurrence identities are preserved.
4. Given a replayable invocation record, when it is replayed, then the core determines the
match; altered records fail.

### Edge Cases

- Duplicate JSON keys remain available to core's checked decoder rather than disappearing
through a binding parse/re-serialize cycle.
- Current command records keep source diagnostics detached from canonical record bytes.
- Historical required-governance policy does not silently become authenticated policy.
- Wrong-store/future/invalid command cursors and incomplete snapshots fail explicitly.

## Requirements

### Functional Requirements

- **FR-001**: Official ecosystem builds MUST use one exact published Core Release for engine,
CLI, schemas and fixtures, with no active development override.
- **FR-002**: Existing authoring semantics, admitted identities and replay outputs MUST retain
their historical contract; new-profile behavior MUST require explicit document selection.
- **FR-003**: Package version reporting and the public capability manifest MUST describe the
actual bundled release, including current accepted formats and command-line capabilities.
- **FR-004**: Applications MUST be able to import current behavior documents through normal
core admission and invoke/replay reads and transitions with explicit host snapshots/context.
- **FR-005**: Applications MUST be able to inspect command candidates and stream committed
commands using checked history anchors, preserving core ordering and refusal behavior.
- **FR-006**: Authenticated governance MUST remain accessible through the bundled command line;
binding writes MUST preserve the core's independent-context requirement and explicit v2
policy/genesis selection. No binding may invent authorization or upgrade historical trust.
- **FR-007**: All implementation changes MUST have observed failing tests before implementation,
and collected binding/model conformance tests and canonical release checks MUST pass before
delivery. Verification evidence MUST identify the tested revision and any remaining limits.

### Key Entities

- Core Release: immutable revision/tag, version, artifact names and checksums.
- Invocation: declared operation, bindings/input/context and trusted snapshot, canonical
outcome record, optional uncommitted transition candidate.
- Durable command: typed candidate intent and committed occurrence within exact history.
- Governance context: independently supplied policy judgment context for a fresh write.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Every official build, release asset and installed package reports the same exact
core revision; unsupported overrides are refused by the existing gates.
- **SC-002**: All existing equivalence/replay examples pass, with no historical fixture copied
or redefined to disguise a semantic change.
- **SC-003**: Release invocation/command fixtures have byte-identical binding outcomes; at
least one altered-record and one invalid-cursor case fail as expected.
- **SC-004**: Core and ecosystem required gates and clean-install release checks pass for the
revisions being delivered; publication follows validation and consumer verification.

## Assumptions

- Core 012 and 013 are already implemented; the current core revision must become a published
Core Release before ecosystem support lands.
- The existing Python DSL retains its profile. New command syntax is authored as checked
current wire documents; a Python command DSL and new governance DSL are separate features.
- The package exposes current trusted governance through core's bundled command line;
Python helpers delegate document admission, invocation and history operations to core.
- The user's existing local Spec Kit edits remain untouched. Delivery uses an isolated
checkout and an explicit upgrade branch.
