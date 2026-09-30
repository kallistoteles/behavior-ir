# Feature Specification: Agent-Ready Package and Skills Library

**Feature Branch**: `008-agent-ready-package`

**Created**: 2026-09-30

**Status**: Draft

**Input**: User description: "Next step is to try to build a real program using this, but for us to be able to do that we need to make this into a package together with agent skills for agents to be able to utilize this library"

## Context

Features 001–007 produced a working system. Its parts are:
- a Python authoring DSL;
- an engine that admits, hashes, evaluates and replays behavior;
- a verifier;
- a persistence contract with a reference store;
- a command-line tool.

Today all of it is only usable from inside this repository's development environment.

The next experiment is to build a real, non-trivial application in a **separate repository**, largely by coding agents. That experiment has two purposes: to find what the current language genuinely cannot express, and to find where its guidance fails. It needs two things this feature provides:

1. **A versioned, binding-neutral release.** A separate project installs the package for its language (a *binding*) at a pinned version, without the source tree, the engine toolchain or the development environment. The engine owns all semantics; bindings are how applications author and invoke behavior.
2. **An agent skills library.** It teaches agents to use what exists today (not hypothetical features) and to record a semantic gap instead of working around it.

This feature adds no new behavior semantics. The language, engine, verifier and persistence contract stay as released in 007.

## Clarifications

### Session 2026-09-30

- Q: How do the agent skills reach a consumer project? → A: The skills are added to this
  repository, versioned together with the code; there is no installer or install command. A
  consumer agent uses the skills of the same repository revision as the package it depends on.
- Q: How does a consumer project install the library? → A: Option A, generalized to
  binding-neutral releases.
  - A Behavior release is an immutable tagged release of the semantic engine. It provides
    prebuilt artifacts for each supported language binding and target platform.
  - Consumer applications install and pin the package for their language. On supported targets
    they need no Rust toolchain.
  - Bindings are authoring and invocation interfaces only. Semantic evaluation, canonical hashing,
    verification and persistence behavior stay owned by the common engine.
  - At first, the engine and the bindings are released at the same version and require exact
    compatibility.
  - Local path and source builds remain a contributor workflow, never the consumer acceptance path.
- Added with Q1: the same semantic module expressed through different bindings must resolve to the
  same typed IR and semantic hashes.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install and use the library in a fresh project (Priority: P1)

A developer, or an agent acting for them, starts an empty application repository. They add the Behavior package for their language (the Python binding in this feature) as a dependency at a pinned released version and install it. Then they author a small module, evaluate an action, verify the module, persist transitions in a store and replay the history. At no point do they clone or build this repository.

**Why this priority**: Without an installable release there is no separate application to build; everything else depends on it. Testing against a real pinned release, not a local checkout, is what exercises the boundary an external user gets.

**Independent Test**: Take a clean machine or container with only the documented prerequisites. Create a new project, install the pinned package, and run a short script covering authoring, evaluation, verification, persistence and replay. It succeeds with the same results as the same script inside this repository.

**Acceptance Scenarios**:

1. **Given** an empty project and the documented prerequisites, **When** the binding package is added at a pinned released version and installed, **Then** importing it succeeds without compiling the engine. It reports its binding version, the engine version it requires, and the wire IR and record versions it supports.
2. **Given** the installed package, **When** a module is authored, evaluated, persisted and replayed, **Then** decisions, records and state identities are byte-identical to those produced by the same code in this repository at the same version.
3. **Given** the installed package but no solver on the machine, **When** verification is requested, **Then** a clear error names the missing prerequisite and how to provide it. Verification never silently passes or silently skips, and evaluation, persistence and replay keep working.
4. **Given** the installed package, **When** the command-line tool is invoked (admit, eval, replay, verify), **Then** it is available in the project environment and behaves as documented.

---

### User Story 2 - An agent authors a correct behavior model from the skills alone (Priority: P1)

A coding agent working in the application repository gets a domain requirement ("customers may not place orders beyond their credit limit"). It turns this into a behavior model using only the authoring skill: entities, types, actions, lifecycle, references, queries, local and module-level invariants, and exact arithmetic. It uses only features that exist, and the result is admitted, evaluates correctly and verifies.

**Why this priority**: Building the application is done by agents. If they cannot author correct models from the guidance, the experiment measures the guidance, not the language.

**Independent Test**: Give an agent that has only the installed package and the skills a set of domain requirements, including ones taken from the existing example domains. Check that its models are admitted and meet the stated acceptance checks, and that they use no construct absent from the public API.

**Acceptance Scenarios**:

1. **Given** the authoring skill, **When** an agent models a requirement the language can express, **Then** the model uses the documented constructs and is admitted without error.
2. **Given** a requirement the language cannot express (for example a grouped invariant "at most 3 open orders per customer" or a cross-entity filter), **When** the agent reaches it, **Then** it records a semantic gap in the documented gap log. It does not approximate the requirement silently, reach into engine internals, or write unsupported IR by hand.
3. **Given** every example in the skills, **When** the examples are run against the installed package, **Then** each behaves exactly as the skill states.

---

### User Story 3 - An agent runs and interprets verification correctly (Priority: P2)

An agent verifies the model it wrote and reads the outcomes correctly:
- **proven:** holds for every state;
- **counterexample:** a runtime-confirmed failing case to fix in the model;
- **inconclusive:** a blocking finding that is never treated as success.

It fixes models rather than weakening checks. It can tell verifier precision debt (the property holds but cannot be proven yet) from a missing requirement in the model.

**Why this priority**: The value of the system rests on agents acting correctly on verification results. An agent that deletes an invariant to "fix" a counterexample defeats the purpose.

**Independent Test**: Present the agent with seeded verification outcomes: a real counterexample, an inconclusive precision case, and a missing guard. Check its response against the skill's rules.

**Acceptance Scenarios**:

1. **Given** a confirmed counterexample, **When** the agent responds, **Then** it changes the model (a guard, a constraint, or a type) or records the finding as accepted. It never removes or weakens the violated rule just to make verification pass.
2. **Given** an inconclusive result caused by known precision debt, **When** the agent responds, **Then** it reports it as blocking and inconclusive, explains the cause from the skill's catalogue, and does not claim the property holds.
3. **Given** an inconclusive result caused by an expectation the model does not state, **When** the agent responds, **Then** it adds the missing guarantee to the model and does not ask for a verifier assumption.

---

### User Story 4 - An agent builds an application around the library (Priority: P2)

An agent builds the application layer: host code, a storage backend or the reference one, and request handling. Business semantics stay in the behavior model; the host only supplies identities, time, inputs, storage and presentation. Every state change goes through evaluate → commit, and history is replayable.

**Why this priority**: The experiment's second question is whether real applications can keep business rules in Behavior. The application skill defines the boundary the agent must not cross.

**Independent Test**: Review an agent-built application against the application skill's rules:
- no state is written outside commits;
- no business rule lives only in host code;
- identities are host-supplied;
- the store's conformance suite passes for any custom backend;
- replay of its history succeeds.

**Acceptance Scenarios**:

1. **Given** the application skill, **When** an agent implements a feature, **Then** each business decision is a behavior action and the host only calls evaluate and commit.
2. **Given** a custom storage backend written by the agent, **When** the packaged conformance suite runs against it, **Then** it passes every case before the backend is used.
3. **Given** a state conflict at commit time, **When** the host handles it, **Then** it re-evaluates on the current state rather than forcing or retrying the stale bundle.

---

### User Story 5 - Keep the package and the skills in sync with the engine (Priority: P3)

The maintainer changes this repository in future features. Releasing a new version produces a package and a matching skills library. Automated checks fail if a skill describes a construct, command or result that the released package does not behave as described.

**Why this priority**: Skills that drift from the package teach agents non-existent features, which is exactly the failure the experiment must avoid. It matters over time more than on day one.

**Independent Test**: Deliberately change a skill example so it no longer matches the package, and check that the release checks fail and name the example.

**Acceptance Scenarios**:

1. **Given** a release, **When** the release checks run, **Then** every runnable example in every consumer skill is executed against the built package and must match its stated outcome.
2. **Given** a skill that names an API element, **When** that element is not part of the public API of the released version, **Then** the release checks fail.
3. **Given** a released package, **When** an agent reads the skills, **Then** the skills state which package version they describe, and the package reports a version matching them.

---

### User Story 6 - Engine development guidance, kept separate (Priority: P3)

An agent working on this repository itself (the core, verifier, wire formats or persistence) uses a separate engine-development skill. It carries the project's constitution rules, the Spec Kit workflow, the gates, and the compatibility rules for frozen identities and wire versions. It is kept apart from the consumer skills, so consumer projects are never pointed at it.

**Why this priority**: Consumer agents must be firmly steered away from engine internals. Engine agents need different, stricter guidance. Mixing the two blurs the boundary the experiment depends on.

**Independent Test**: Check that the consumer skills' location contains only the three consumer skills and that the engine-development skill lives elsewhere. In this repository, check that the engine skill is available and references the actual gates and workflow.

**Acceptance Scenarios**:

1. **Given** the consumer skills' location, **When** its skills are listed, **Then** only the consumer skills (authoring, verification, application) are present.
2. **Given** this repository, **When** an engine agent follows the engine skill, **Then** its instructions match the constitution and the gates actually used (formatting, lints, tests, determinism check, frozen identities).

### Edge Cases

- **A different platform or Python version than those supported:** installation fails with a clear message naming the supported ones. It never produces a half-working installation.
- **Solver missing or a different version:** verification reports the problem explicitly. A different version may change which checks are proven or inconclusive, so the attestation records the solver version, as it does today.
- **A package upgrade after records have been written:** records and stores written by an earlier version still replay. If a version cannot read them, it refuses with a clear message rather than producing different results.
- **Skills from a different revision than the installed package:** the skills carry their version, and a mismatch with the installed package is visible to the agent before it acts.
- **An agent asking for a feature the skills do not describe:** the skills direct it to the gap log, not to guessing.
- **Two application repositories pinning different versions:** each works independently; nothing is installed globally in a way that couples them.

## Requirements *(mandatory)*

### Functional Requirements

**Release and bindings**

- **FR-001 — Versioned distribution**: A Behavior release MUST be identified by an immutable repository tag. It MUST provide installable artifacts for each supported binding and target platform. A consumer project pins an exact released version of its chosen binding, and the binding MUST use the corresponding version of the Behavior semantic engine. For the experiment phase, artifacts attached to the tagged release are sufficient; publishing to public package indexes is out of scope.
- **FR-002**: A binding's installed package MUST include everything needed to author, admit, evaluate, verify, persist and replay: the binding's authoring layer, the engine, the verifier driver, and the reference store with the conformance suite. The release MUST also provide the command-line tool.
- **FR-003 — No engine toolchain required by consumers**: On supported prebuilt platforms, application consumers MUST NOT need a Rust compiler or the Behavior engine development environment. Language-specific tooling required by the selected binding remains allowed: a Python application needs Python, a Java application needs Java.
- **FR-003a — Bindings carry no semantics**: bindings are authoring and invocation interfaces, not semantic implementations. A binding MUST NOT decide on its own any of:
  - decimal and exact arithmetic;
  - query membership;
  - constraint, invariant or module-invariant outcomes;
  - lifecycle validity;
  - canonical hashing;
  - verification;
  - persistence semantics.

  All of these are determined by the common engine.
- **FR-003b — Cross-binding equivalence**: the same semantic module expressed through different bindings MUST resolve to the same typed IR and semantic hashes, and so to the same evaluation, verification results and persistence semantics. The canonical wire form is the binding-neutral reference. While only one binding exists, each binding's module MUST be checked against its canonical wire form: modules built through the binding must reproduce the identities of the equivalent wire fixtures.
- **FR-004 — Three versions**: A release MUST distinguish and report:
  - the **engine version** (the implementation release);
  - the **wire IR versions** it reads and writes (for example 0.1–0.6), along with the record and store document versions;
  - the **binding version** (the package version of each language binding).

  At first, the engine and all bindings are released together at the same version, and a binding requires exactly the matching engine version. Flexible version ranges are out of scope.
- **FR-005**: The one external prerequisite, the SMT solver used for verification, MUST be documented with its supported version. Its absence MUST produce an explicit error only when verification is requested.
- **FR-006**: Each binding MUST define its public API explicitly: its importable names. The release MUST define its binding-neutral public surface: the command-line commands, the wire, record and store document formats, and the backend contract. Anything not listed is internal and may change without notice.
- **FR-007**: The same release MUST produce byte-identical decisions, records, hashes and attestation contents (given the same solver version) whether it is used through an installed binding package or from this repository.
- **FR-008**: The release version MUST follow a documented versioning policy that distinguishes behavior-identity-breaking, format-breaking and additive changes.
- **FR-009**: Records and stores written by an earlier release MUST remain readable and replayable by later releases of the same major version, or be refused explicitly.
- **FR-009a — The consumer boundary**: a consumer application MUST use Behavior only through a released public binding. It MUST NOT depend on the engine's internal crates or on paths inside this repository. Local path or source builds of a binding are a contributor workflow for developing Behavior and an application together. They are never the consumer acceptance path, and the consumer project MUST also be tested against a pinned release.

**Skills library**

- **FR-010**: A consumer skills library MUST be provided with three skills: authoring, verification and application. Each covers what the released version implements, and nothing more.
- **FR-011**: The consumer skills MUST live in this repository, in one documented location, and be versioned with the code. No installer or install command is provided. A consumer agent uses the skills from the same repository revision as the package version it depends on. The consumer skills MUST be kept apart from the engine-development skill, so one can be handed over without the other.
- **FR-012**: The authoring skill MUST cover:
  - entities and field types, including optional values, identities and references;
  - fixed-scale and exact arithmetic with explicit rescaling;
  - actions with preconditions, effects, postconditions, creation and removal;
  - entity constraints and invariants;
  - relational queries with their operators;
  - module-level invariants;
  - derived values and rules;
  - the placement rules (for example, queries are not allowed in entity constraints).
- **FR-013**: The verification skill MUST explain each outcome and each check kind, with a decision procedure for responding to them. Its core rules:
  - fix the model, never weaken a check to pass;
  - inconclusive is blocking;
  - a counterexample is runtime-confirmed.
  It MUST include a catalogue of known precision debt that distinguishes guarantees the model states from expectations it does not state.
- **FR-014**: The application skill MUST state the host boundary:
  - business rules live in behavior;
  - every state change goes through evaluate and commit;
  - identities and time are supplied by the host;
  - conflicts lead to re-evaluation;
  - a custom backend must pass the conformance suite;
  - replay is the audit mechanism;
  - Behavior is used only through the released public binding, never through engine crates or
    this repository's paths (FR-009a).
- **FR-015**: Every consumer skill MUST contain the explicit rule: if the public API cannot express a requirement, record a semantic gap and do not work around it. Working around means reaching into internals, hand-writing IR, moving the rule into host code, or silently approximating it.
- **FR-016**: The skills MUST define a semantic gap log format for consumer projects. Each entry records the requirement, why the current language cannot express it, what was done instead (if anything), and its severity. The log is meant to be collected as input to future features.
- **FR-017**: The skills' examples MUST be drawn from, or checked against, models known to pass admission, evaluation, verification, persistence and replay in the released version. The existing example domains are the primary source.
- **FR-018**: Each skill MUST state the release version it describes (engine and binding versions, which match at first).
- **FR-019**: A separate engine-development skill MUST exist for agents changing this repository. It covers the constitution, the Spec Kit workflow, the quality gates and the compatibility rules. It MUST NOT be part of the consumer delivery.

**Release consistency**

- **FR-020**: A release check MUST run every runnable example in every consumer skill against the built package and fail on any deviation from the stated outcome.
- **FR-021**: A release check MUST fail if a consumer skill references an API element that is not in the declared public API of that version.
- **FR-022**: A release check MUST install each built binding package into a clean environment, outside this repository, and run a consumer smoke scenario (User Story 1).
- **FR-023**: A release check MUST verify cross-binding equivalence (FR-003b) for every shipped binding against the canonical wire fixtures of the example domains.

### Key Entities

- **Behavior release**: an immutable tagged release of the semantic engine with its versions (engine, wire IR, record and store documents). It carries prebuilt artifacts for each supported binding and platform, the command-line tool, and the solver prerequisite.
- **Binding package**: the installable package for one language. It carries a binding version, requires exactly one engine version, and declares its public API. It authors and invokes behavior; it never implements semantics.
- **Public API manifest**: the declared list of stable importable names, command-line commands and formats for a release. The skills are checked against it.
- **Consumer skill**: a versioned instruction set for application-building agents (authoring, verification, application), with runnable examples. It lives in this repository.
- **Engine skill**: an instruction set for agents modifying this repository, kept apart from the consumer skills.
- **Semantic gap log**: a per-application record of requirements the language could not express. It is the experiment's main output.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a clean environment with only the documented prerequisites, installing the pinned package and running the consumer smoke scenario takes under 10 minutes of wall-clock time and succeeds without touching this repository.
- **SC-002**: 100% of the runnable examples in the consumer skills pass against the released package, and a deliberately broken example is reported by name.
- **SC-003**: The smoke scenario's decisions, records and state identities are byte-identical between the installed package and this repository at the same version.
- **SC-004**: An agent given only the installed package and the consumer skills produces admitted models for all requirements in a reference set drawn from the existing example domains, using no construct outside the public API.
- **SC-005**: For a reference set of requirements that the language cannot express, the agent records a gap entry for every one and hand-writes IR or reaches into internals for none.
- **SC-006**: For the seeded verification outcomes (a real counterexample, a precision-debt inconclusive, a missing-guarantee inconclusive), the agent's responses follow the verification skill's decision procedure in all three cases; it never weakens a violated rule.
- **SC-007**: The consumer skills' location contains exactly the three consumer skills. The engine skill lives in a separate location in this repository.
- **SC-008**: Every existing behavior identity, record, golden file and persistence document stays byte-identical. This feature changes packaging and guidance, not semantics.
- **SC-009**: For every example domain, the module built through each shipped binding has the same behavior version and item hashes as its canonical wire form. Any difference fails the release.
- **SC-010**: In a clean environment with no Rust toolchain, installing the pinned binding package succeeds on every supported platform.

## Assumptions

- The first consumer is a private experiment run by this project's maintainer. Public discoverability, marketing documentation and third-party support are out of scope.
- Only the Python binding is shipped in this feature, but the release structure, versioning and requirements are binding-neutral. Other bindings (for example JavaScript/TypeScript, Java, .NET, Go) are future features and must meet FR-003a and FR-003b.
- Supported platforms at first: Linux x86-64 with the Python version the project already requires. Other platforms may follow and will be declared, not assumed.
- The SMT solver stays an external prerequisite rather than being bundled, as today. The version is pinned in documentation and recorded in attestations.
- Agents in the consumer project are coding agents that can read skill files and run commands. The skills follow the agent skill format this project already uses for its own tooling. Pointing a consumer agent at the skills (for example by referencing or copying this repository's revision) is the maintainer's manual step; no tooling is provided for it.
- The public API at first is what the Python package exports, the command-line tool, the wire, record and store document formats, and the backend contract as they stand after 007.
- Feature 007 is committed as a known-good baseline before this feature's release.
- Engine features (new semantics) are frozen during this feature. Semantic gaps found later become input to future features.
