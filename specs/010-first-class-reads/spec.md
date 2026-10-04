# Feature Specification: First-Class Reads

**Feature Branch**: `010-first-class-reads`

**Created**: 2026-10-03

**Status**: Draft

**Input**: User description: the next phase is a **Behavior runtime model** with clearly separated
operations: READ (observe state), TRANSITION (change state), MIGRATION (change schema) and, later,
COMMAND INTENT (affect the outside world). Migration (009) is done. This feature makes READ
first-class:

```text
Read: (StateVersion, Expression) → Value + Trace + ObservedReads
```

There is no ΔS, no commit, no transition record and no dummy bound entity. (Full description in
the conversation of 2026-10-03.)

## Context

The external application built on release 0.8.0 ("TCUP") answered questions through about 107
generated, effect-free actions, plus an artificial "lab" entity that every one of them bound. To
read, it pretended to change state. That is architecture around the engine, not use of it:

- **A read needs a bound entity:** an action requires at least one, so reads that are about the
  whole state, or about nothing in particular, invent one.
- **A read looks like a decision:** the result is ALLOW or DENY, and the answer is hidden in the
  trace.
- **Whole records cannot be read deliberately:** a question such as "show the whole record,
  including which fields are unknown" needed one query per field. Short-circuit evaluation (which
  correctly records only what was read) decided which fields were observed.

The engine already has everything a read needs semantically: typed expressions, derived values,
relational queries over sets, exact arithmetic, evaluation facts as of an exact history position,
traces and observed reads. What is missing is the operation itself.

The runtime model this feature starts to complete:

| Operation | Changes | Recorded as |
|---|---|---|
| **Read** | nothing (ΔS = ∅) | a read record, reproducible, never part of history |
| **Transition** | state, within one schema | a transition record |
| **Migration** | the schema under which state is valid | a migration record |
| Command intent (feature 012) | nothing inside; requests something outside | (later) |

Principle: **observing the world is not changing it.** A read can never write, commit, appear in
the store's history or alter any identity. Observation, decision and change are three different
things.

For agents the separation becomes a capability boundary: *asking a question* is a read capability;
*asking to change something* is a transition capability.

## Clarifications

### Session 2026-10-03

- Q: Can a projection list only stored fields, or also the entity type's derived values? → A: Stored
  fields and declared derived values of the projected entity type. A derived item is evaluated with
  its existing semantics against the same exact state, and its field and query dependencies are
  observed as usual. If any requested item fails for any member, the whole read is an evaluation
  error naming the member and the item; partial success is never implicit. Ad-hoc reference
  traversal (`order.customer.name`) is not allowed in projections; cross-entity navigation stays a
  separate relational capability. A projection exposes semantic values, not just the stored
  representation.
- Q: Can actions, rules, derived values or other declared reads call a declared read? → A: No.
  Declared reads are capability entry points only. Nothing in a module (actions, rules,
  invariants, derived values, other declared reads) may reference one. Reusable pure computation
  belongs in derived values, which reads and actions both use. A reference to a declared read is
  refused at admission with a specific diagnostic, `READ_CALL_NOT_ALLOWED`, that tells the author
  to move the shared computation into a derived value. Principle: *capabilities are entry points,
  not building blocks*; derived values compose semantics, and reads and actions expose
  capabilities.
- Q: Can a projection also apply to one entity given by identity and return a single record? → A:
  Yes. There are two cardinalities with the same projection semantics: a projection of a bound
  entity returns exactly one record, and a projection of a query returns a list of records. A
  single-entity projection works on an entity the read's parameters have already bound; it never
  looks up an identity itself. If the bound identity is absent at the read's exact state, the
  binding fails and the read is refused before evaluation. Stored fields, derived values,
  observation and failure work the same way in both forms. A query projection's output is in
  canonical identity order for deterministic encoding; the query itself stays an unordered set.
  Principle: *cardinality belongs to the read contract, not to a query workaround.*
- Q: When an agent calls a declared read through the capability boundary, what does it get back?
  → A: Only the declared result plus the identity of the read record that produced it. The trace
  and observations (which may include fields a derived value read internally) are not part of the
  capability response. The engine returns the capability response and the full read record as two
  distinct types, so a host cannot hand out the evidence by mistake. Trace exposure is not
  configurable per read; inspecting evidence by record identity would be a separate,
  policy-controlled capability. The record identity binds the declared read, behavior version,
  exact state, parameters and context, observations and result. Principle: *capabilities expose
  declared information; records preserve complete evidence.*

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Answer a question about the current state (Priority: P1)

An application needs a value from its store: how many cultures are active, the total of a
customer's open orders, whether an identity is in use. The author writes it as a read: an
expression in the behavior language, possibly using the module's derived values and queries. The
application evaluates it against the store's current state. It gets the typed value, the trace
and exactly what was observed, and the store is untouched.

**Why this priority**: This is the core gap. Every application reads more often than it writes,
and today every read pretends to be a transition.

**Independent Test**: Create a store, commit transitions, read a derived value and a count. The
values are correct. The store's position, state identity, history and head are byte-identical
before and after. No bound entity was invented.

**Acceptance Scenarios**:

1. **Given** a store with entities, **When** a read of a derived value over the whole state
   (for example, a count of active cultures) is evaluated, **Then** it returns that value with
   its trace and observed reads. The store is unchanged.
2. **Given** a read with parameters (an input value, or an entity bound by identity), **When** it
   is evaluated, **Then** the parameters are checked like action parameters. An unknown entity,
   a missing input or a wrong type is refused before anything is evaluated.
3. **Given** a read that fails to evaluate (for example, a division by zero), **When** it is
   evaluated, **Then** the result is an explicit evaluation error with its reason. Nothing is
   written.

---

### User Story 2 - Read whole records with an explicit projection (Priority: P1)

A user or an agent asks "show me these orders, with these fields", or "show me order 42". The
application issues a projection read: an explicit list of fields to return, over either a query
(zero or more records) or one bound entity (exactly one record). For a query, the result is
every member, in a canonical order, with exactly the requested fields, including fields whose
value is absent. Every projected field counts as deliberately read.

**Why this priority**: Without projections, reading a record means one expression per field, and
which fields are observed depends on evaluation order. This is the second gap TCUP reported.

**Independent Test**: Run a query projection with a filter over a store, and an entity projection
of one bound entity. The query's records are exactly the matching entities, in canonical order;
the entity projection returns exactly one record. Each record has exactly the projected fields;
absent optional values are reported as absent. The observed reads list every projected field of
every record.

**Acceptance Scenarios**:

1. **Given** a query and a projection of three fields, **When** it is read, **Then** every
   matching entity is returned with its identity and those three fields, nothing more, in
   canonical order.
2. **Given** a projection that names a field the entity type does not have, **When** the read is
   admitted, **Then** it is refused, naming the field.
3. **Given** an entity whose optional field is absent, **When** it is projected, **Then** the
   field is present in the result as absent, not omitted.
4. **Given** a projection that includes a derived value of the entity type (for example, an
   order's total), **When** it is read, **Then** each member's value is the one that derived value
   gives for that member at that state, and everything its evaluation read is observed.
5. **Given** a projected derived value that fails to evaluate for one member, **When** it is read,
   **Then** the whole read is an evaluation error naming that member and that derived value. No
   partial result is returned.
6. **Given** a read with an order bound by identity and a projection of three fields, **When** it
   is read, **Then** exactly one record is returned with the order's identity and those fields.
   **When** that identity does not exist at the read's state, **Then** the binding fails and the
   read is refused before evaluation; it does not return an empty result.
7. **Given** a projection item that traverses a reference (for example, `order.customer.name`),
   **When** the read is admitted, **Then** it is refused.

---

### User Story 3 - Read the past exactly (Priority: P2)

An auditor or an application asks what a value *was* at an earlier position, for example the
open order total just before a disputed transition. The read is evaluated against that exact
history position and gives the same answer it would have given then.

**Why this priority**: History is only useful if it can be queried with the same semantics as
the present. Combined with 009, a past position is read under its own schema.

**Independent Test**: Commit transitions, remember the result of a read at position p, commit
more, then read at position p again. The result, trace and observed reads are identical. A read
at a position under another schema is refused unless it uses that schema's module.

**Acceptance Scenarios**:

1. **Given** a store at position 10, **When** a read is evaluated at position 4, **Then** the
   result is the one that state determines. Later transitions have no influence.
2. **Given** a store that migrated at position 6, **When** a read at position 4 uses the
   pre-migration module, **Then** it is evaluated. **When** it uses the post-migration module,
   **Then** it is refused with `SCHEMA_MISMATCH`.

---

### User Story 4 - Reads are reproducible evidence (Priority: P2)

A read's answer may be used for a decision outside the engine: a report, an agent's next step, an
audit. Its result can be kept as a **read record**: what was asked, of which store, at which
exact state, under which behavior, with which result and which observations. Anyone can later
replay it from its recorded facts alone, or against the store at that position, and get the same
bytes.

**Why this priority**: Behavior's value is that results are explainable and reproducible. A read
that cannot be reproduced would be the one unauditable operation.

**Independent Test**: Evaluate a read, keep its record, replay it from the record alone and
against the store. Both reproduce it byte for byte. A tampered record (result, facts or state
reference) does not reproduce, and the difference is reported.

**Acceptance Scenarios**:

1. **Given** a read record, **When** it is replayed from its recorded facts, **Then** the same
   result is produced.
2. **Given** a read record and the store, **When** it is replayed against the store at the
   recorded position, **Then** the facts are re-derived, compared and found equal.
3. **Given** a read record with an altered result or altered facts, **When** it is replayed,
   **Then** the mismatch is reported.

---

### User Story 5 - Declared reads as capabilities (Priority: P3)

A module publishes declared reads, such as "open order total of a customer", "cultures needing
attention" or "the record of an order". Each is typed, with parameters, and content-addressed as
part of the behavior. An agent or an application calls a declared read by name with arguments,
through the same validated capability boundary as actions: it never sends an expression of its
own.

**Why this priority**: It makes the read/transition separation usable for agents ("asking is a
read capability; changing is a transition capability"). Ad-hoc expressions from untrusted callers
would not be governed.

**Independent Test**: Declare reads in a module, call one by name through the capability boundary
with valid and invalid arguments. Valid calls return the result and the record's identity, never
the trace or observations; invalid ones are rejected with every
problem listed, and nothing is evaluated. Adding a declared read changes the behavior version but
not the schema, so no migration is needed.

**Acceptance Scenarios**:

1. **Given** a module with a declared read, **When** a caller invokes it by name with valid
   arguments, **Then** it is evaluated like any read.
2. **Given** a call naming an unknown read, or arguments of the wrong type, **When** it is
   submitted, **Then** it is rejected with every problem named, before any evaluation.
3. **Given** a module that gains a declared read, **When** it is used with an existing store,
   **Then** no migration is needed.
4. **Given** an action, rule, invariant, derived value or declared read that references a declared
   read, **When** the module is admitted, **Then** it is refused with `READ_CALL_NOT_ALLOWED`,
   pointing the author to a derived value for the shared computation.
5. **Given** a declared read projecting a customer's name and status, where the status is a derived
   value that internally reads a credit limit, **When** an agent calls it, **Then** the capability
   response holds only the name, the status and the record identity. The credit limit appears only
   in the read record the host receives.

### Edge Cases

- **Reads never write:** no read can create, remove or change an entity. A read expression that
  contains an effect is not a read and is refused at admission.
- **Empty results:** a query projection with no matching members returns an empty list; an aggregate
  over nothing returns the aggregate's defined value for an empty set (for example, a count of 0,
  an absent minimum).
- **Large results:** a projection over many entities returns all of them in canonical order.
  Sorting by a field and pagination are not part of this feature (ordering is a known semantic
  gap); canonical order is by identity.
- **Concurrency:** a read is evaluated against one consistent snapshot: the position it names (or
  the current position when it starts). Commits that land during the read never affect it.
- **Unknown facts:** a read in plain mode (no store) needs its facts supplied. A missing fact is
  reported as `UNKNOWN_FACT`, never guessed.
- **Schema:** a read is bound to the schema of the position it reads. There is no implicit
  conversion between schemas.
- **A projected derived value fails for one member:** the whole read is an evaluation error naming
  the member and the item. Tolerant, per-member results would be a separate, explicit feature.
- **Evaluation errors** are results, not crashes. They are recorded and reproducible like any
  other result.
- **Reads inside transitions** are unchanged: actions keep using derived values and queries as
  today. This feature adds a separate operation; it does not change transitions.

## Requirements *(mandatory)*

### Functional Requirements

**The read operation**

- **FR-001**: The engine MUST offer a read operation, distinct from transitions and migrations. A
  read evaluates a typed, pure expression of the behavior language against one exact state and
  returns a value. It never produces a state change, a commit, a transition record or a change to
  any store document or identity.
- **FR-002**: A read MUST be evaluated against exactly one state: a named history position of a
  store, the store's current position, or (plain mode) a supplied set of facts. Every value it
  observes comes from that state.
- **FR-003**: A read MUST require no bound entity. It MAY have parameters: input values, context
  values, and entities bound by identity. They are checked like action parameters; a problem
  refuses the read before evaluation and names every problem.
- **FR-004**: A read MUST use the full read semantics of the behavior language: field reads,
  derived values, relational queries and aggregates, `exists`/`referenced`, and exact
  arithmetic. It MUST NOT contain effects (setting fields, creating, removing).
- **FR-005**: A read's result MUST be one of:
  - a **value** (any expression type, in canonical encoding);
  - an **evaluation error**, with its reason;
  - a **refusal before evaluation**: invalid parameters or supplied facts that contradict each
    other (`INVALID_INPUT`, reason `INCONSISTENT_FACTS` for facts), or a bound identity that does
    not exist at the read's state (`INVALID_BINDING`).

  A fact that is missing and discovered during evaluation is an evaluation error with reason
  `UNKNOWN_FACT`. A schema mismatch, or a position that is not a state of the store, is not a
  result: it is refused with an error and produces no record (FR-009).

**Projections**

- **FR-006**: The engine MUST offer a projection read: an explicit projection (the list of items
  to return) over either a query on one entity type or one bound entity. A projection item is
  either a stored field of the entity type or a declared derived value of that entity type.
  - A **query projection** returns every member of the query at that state, each as a record with
    its identity and exactly the projected items. Records are encoded in canonical identity order;
    the query itself remains an unordered set.
  - An **entity projection** returns exactly one record for an entity already bound by the read's
    parameters. It never resolves an identity itself. If the bound identity does not exist at the
    read's state, the binding fails and the read is refused before evaluation (FR-003).
  Both forms follow the same rules for items, observation and failure (FR-006a, FR-006b, FR-007).
- **FR-006a**: A derived projection item MUST be evaluated by binding the member to the derived
  value's entity parameter and evaluating the existing derived expression against the read's exact
  state. There is no separate read-only implementation. Everything that evaluation observes
  (field reads, query and other evaluation facts) is part of the read's observations and trace. A
  relational derived value is allowed if it is a valid derived value.
- **FR-006b**: If any projection item fails to evaluate for any member, the whole read MUST be an
  evaluation error that names the member and the item. Partial success is never implicit, and an
  absent value never stands for a failed computation.
- **FR-007**: Every projected stored field of every returned member MUST count as observed. A projection
  is independent of evaluation order and of short-circuiting. Absent optional values are returned
  as absent, never omitted.
- **FR-008**: A projection item that is neither a stored field nor a declared derived value of
  the entity type MUST be refused at admission, naming the item. Reference traversal (such as
  `order.customer.name`) is not a projection item; a module that needs it declares a derived value.

**Evidence and reproducibility**

- **FR-009**: Every read evaluated against a valid snapshot MUST produce a **read record**. A read
  against a store whose module schema differs from the schema at the position, or whose position
  is not a state of that store, is refused with an error and produces no record: there is no valid
  snapshot to record. The record holds the read's identity (its expression or
  declared read and the behavior version), the exact state it read (store, state identity,
  position), its parameters, its result, its trace and everything it observed (field reads and
  evaluation facts, as transitions record them). The record is canonical and content-addressed:
  its identity binds the read (expression or declared read), behavior version, exact state,
  parameters and context, observations and result, so a result paired with a record identity means
  exactly that record's result.
- **FR-010**: Reads MUST be deterministic: the same read of the same state with the same
  parameters gives a byte-identical record.
- **FR-011**: A read record MUST be replayable from its recorded facts alone (no store), and
  against the store at the recorded position. Both reproduce it byte for byte, and any difference
  is reported.
- **FR-012**: Read records MUST never become part of a store's history. A store with or without
  any number of reads has identical positions, state identities, records and head.

**Schema and history**

- **FR-013**: A read against a store MUST require the module's schema to equal the store's schema
  at the read position (009), and is refused with `SCHEMA_MISMATCH` otherwise.
- **FR-014**: A read of a past position MUST give the result that position's state determines,
  independent of later transitions or migrations.

**Declared reads and capabilities**

- **FR-015**: A module MAY declare **declared reads**: a name, typed parameters, and either a value
  expression or a projection (over a query or a bound parameter entity). They are content-addressed behavior items. Adding,
  changing or removing them changes the behavior version, never the schema.
- **FR-015a**: Declared reads MUST be entry points only. No module expression (in an action, rule,
  invariant, derived value or another declared read) may reference a declared read. Such a
  reference MUST be refused at admission with `READ_CALL_NOT_ALLOWED`, and the diagnostic MUST say
  to move the shared computation into a derived value. Dependencies therefore run one way: from
  capabilities (declared reads, actions) into derived values, never back.
- **FR-016**: Declared reads MUST be callable by name with arguments through the engine's
  capability boundary for untrusted callers (as structured intents are for actions). A call is
  validated completely (known read, argument types and values, bound identities) before
  evaluation, and every problem is reported. Untrusted callers can only call declared reads;
  ad-hoc read expressions are a trusted, host-side facility.
- **FR-016a**: A call through the capability boundary MUST return a **capability response**
  holding only the declared read's result (or its refusal or evaluation error) and the identity of
  its read record. The trace and observations MUST NOT be part of it. The engine MUST return the
  capability response and the full read record as distinct types, so that disclosure is fixed by
  the engine, not by host routing. Whether a read exposes its trace is not configurable per read.
- **FR-017**: The verifier MUST check declared reads for evaluation errors, as it does for actions'
  expressions: a read that can fail for some valid state is reported with a confirmed
  counterexample.

**Surfaces**

- **FR-018**: The Python binding MUST let authors write reads as expressions (including over
  derived values and queries) and as projection reads (over a query or a bound entity); declare reads in a
  module; and evaluate reads against a store (current or past position) or in plain mode.
- **FR-019**: The command-line tool MUST evaluate a read in plain mode (a module, a read and
  supplied facts) and replay a read record.
- **FR-020**: The consumer skills MUST describe reads: when to read instead of transitioning,
  projections, declared reads as capabilities, and replaying read records. The existing skill
  checks apply.
- **FR-020a**: The project principles MUST state the capability boundary: *capabilities are entry
  points, not building blocks*. Derived values compose semantics; declared reads and actions
  expose capabilities.

**Compatibility**

- **FR-021**: Every existing module, record, store document and golden file MUST keep its bytes
  and identity. Modules without declared reads keep their behavior versions. Actions and their
  records are unchanged.

### Key Entities

- **Read**: a pure, typed observation of one state: an expression, or a projection over a query or
  a bound entity, plus its parameters. It changes nothing.
- **Declared read**: a named, content-addressed read in a module, with typed parameters. It is a
  behavior item, part of the behavior version and not of the schema, and it is the unit that
  capabilities expose. It is an entry point only: nothing in the module references it. Shared
  computation lives in derived values.
- **Projection**: the explicit list of items a projection read returns for each record, over a
  query (zero or more records) or a bound entity (exactly one record): stored fields
  and declared derived values of the entity type. It exposes semantic values, not just the stored
  representation. Every listed stored field is observed, and so is everything a listed derived
  value reads.
- **Read record**: the canonical, content-addressed evidence of one read: its identity, the exact
  state, parameters, result, trace and observations. It is reproducible and never part of store
  history.
- **Read result**: a value, an evaluation error, or a refusal before evaluation.
- **Capability response**: what an untrusted caller receives from a declared read: the read result
  and the read record's identity, nothing else. It is a distinct type from the read record.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every question the external application answered through effect-free actions can be
  answered by a read with no invented entity. The application's generated read-only actions (about
  107) and its artificial bound entity become unnecessary.
- **SC-002**: In a test of at least 1,000 reads interleaved with transitions and a migration, the
  store's positions, state identities, records and head are byte-identical to the same history
  without the reads (100%).
- **SC-003**: Every read record of that test replays byte-identically from its recorded facts and
  against the store. Every tampered record is detected (100%).
- **SC-004**: A query projection of five items (three stored fields and two derived values) over
  10,000 entities completes in under 2 seconds on the reference machine, and records every
  projected stored field as observed (30,000 field facts).
- **SC-005**: A read of a past position gives the same record before and after any number of later
  transitions (100% of sampled positions).
- **SC-006**: An agent can answer questions through declared reads only. Every malformed call is
  rejected with all its problems before evaluation, and no call can change state.
- **SC-007**: Every existing module, record, store document and golden file keeps its bytes and
  identity.

## Assumptions

- **Scope.** This is feature 010 of the runtime model. General invocation, meaning actions with
  zero or optional state bindings (011), and external command intents (012) are separate
  features. After them, the "Behavior kernel v1" is conceptually complete. Joins, group-by,
  sorting, struct types and bulk updates stay out of scope, driven later by real applications.
- **No ordering.** Results are in canonical identity order. Sorting by value and pagination are a
  known semantic gap, not part of this feature.
- **Read records are evidence, not history.** The engine produces them; whether and where a host
  keeps them is the host's choice. A store never stores them.
- **Trust boundary.** Ad-hoc read expressions are for trusted host code. Untrusted callers,
  including agents, use declared reads through the capability boundary, the same split as
  structured intents for actions.
- **Access control** (who may read what) is the host's concern and out of scope; declared reads
  give it a unit to grant.
- **Facts.** Plain-mode reads take facts in the same forms as plain evaluation of actions
  (universes, query facts, field facts, existence and reference facts), and are subject to the
  same consistency rules.
- **Version.** New behavior items (declared reads) and a new record kind make this a minor release
  (0.10.0) under `docs/versioning.md`. Existing documents are unchanged.
