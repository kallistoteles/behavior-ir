# Feature Specification: Verifiable Behavior IR Core

**Feature Branch**: `001-verifiable-behavior-ir`

**Created**: 2026-09-23

**Status**: Draft (revised 2026-09-23 during planning: Python authoring layer in scope,
formal verification moved to the next feature)

**Input**: User description: "AI-native system med verifierbar Behavior IR" — an information system
where AI is the primary human interface, but all system behavior (rules, constraints, actions,
pre/postconditions, state transitions, processes, permissions, invariants) is expressed in a
declarative, restricted Behavior IR that is verified mathematically before deployment, versioned
in Git, and executed deterministically. AI works only through defined capabilities and never
becomes the source of truth. (Full description in the `/speckit-specify` invocation; scope
revision in the `/speckit-plan` invocation.)

## Scope of This Feature (v0.1)

The description covers a whole product vision. This first feature delivers the **executable,
analyzable core** that everything else depends on:

```text
Python authoring layer → typed expression tree → Behavior IR → type validation
  → dependency graph → cycle detection → deterministic evaluation → decision trace
```

- **In scope**: a Python authoring layer in which behavior is written as declarative statements
  and expressions that build IR (never executed as ordinary Python logic); the Behavior IR and a
  stable, canonical serialized form of it; validation (types, dependencies, cycles, evaluation
  order); deterministic execution of actions with full decision records; and a capability
  boundary through which an AI interface submits structured intents (tested with recorded
  intents only).
- **Out of scope (later features)**: formal verification (rule conflicts, overlaps, gaps,
  impossible preconditions via a solver; process state machines and model checking), other
  frontends (AI-proposed IR, DMN), compilation to other targets (Python, WASM, SQL), a
  persistent data layer, a live AI model connection, and end-user conversational UI.

## Clarifications

### Session 2026-09-23

- Q: Should the core model separate a transition's state, input arguments, and context (e.g. the
  acting user) instead of treating every action parameter as an entity? → A: Yes. v0.1 separates
  state, input, and context; effects may only target state; input and context are read-only; no
  separate declared outputs yet (the change set and decision record are the output).
- Q: Should v0.1 store the IR as content-addressed objects (per-node hashes, Merkle structure) or
  as one document with a single hash? → A: Content-addressed identity, simple storage. Every
  semantic node gets a recursive content hash computed from its canonical semantic form (not
  from serialized bytes), with a domain-separation tag per node kind and format version. Names
  meant for humans and source metadata (locations, docs, comments) are outside the hash. The
  module is still stored as one document; a per-object store is a later storage change, not an
  IR migration.
- Q: Should type errors fail when an expression is built (DSL and engine) rather than in a
  later validation pass? → A: Type-check at both layers. The Python DSL rejects ill-typed
  expressions at construction for immediate author feedback. Serialized IR remains untrusted;
  the engine parses, resolves, and type-checks it into a Typed Behavior IR. Only typed semantic
  IR may be hashed, verified, or evaluated. A raw/wire representation may exist at the
  boundary, but invalid IR never enters the semantic engine.
- Q: Which types beyond Bool, Int, Decimal, String, and enumerations should v0.1 include? → A:
  `Option<T>` and typed identity `Id<T>` as core semantic types, plus a generic nominal
  (newtype) mechanism over primitives. Money is defined in the domain as a nominal Decimal, not
  as a privileged core type. Operator validity belongs to the nominal type definition, so
  Money + Money can be valid while Money + Decimal is rejected. Currency parameters are later.
- Q: Should invariants also be checked on the starting state of an action? → A: Yes.
  Invariants define the valid-state boundary and must hold both before and after every ordinary
  transition. An invalid starting state is rejected as `INVALID_STATE` before preconditions are
  evaluated, so the verifier may assume I(S) when proving a transition preserves I. Recovery
  from invalid states, if needed later, uses an explicit repair/migration mechanism rather than
  weakening normal transition semantics. Principle: transitions preserve validity; they do not
  establish validity from arbitrary state.

## User Scenarios & Testing *(mandatory)*

Actors:

- **Behavior author**: a developer who writes and changes the system's rules.
- **Reviewer**: a person who approves behavior changes before they are used.
- **Capability caller**: any client, including an AI interface, that asks the system to do
  something on behalf of a human.
- **Auditor**: a person who needs to know why a decision was made.

### User Story 1 - Author behavior in Python and get it validated (Priority: P1)

A behavior author defines entities (e.g. `Invoice`, `User`), derived values (e.g. `margin`),
rules (e.g. `high_risk`), invariants, and actions (e.g. `approve_invoice` with `requires`,
`set_`, `ensures`) in Python. Comparisons such as `invoice.amount <= user.approval_limit` build
typed expression trees instead of being computed; ill-typed expressions and ordinary Python
control flow on symbolic values fail immediately at the offending line. The author collects
everything in a behavior module, compiles it to IR, and validates it. They get either "valid"
plus an evaluation order, or a precise list of errors such as unknown references or circular
dependencies between derived values/rules.

**Why this priority**: nothing else works without a well-defined, validated behavior description.
It is the minimum usable product: behavior that is data, reviewable and analyzable.

**Independent Test**: write the invoice example and the `margin → high_risk` example, compile and
validate them, and confirm they pass with the expected evaluation order; introduce a type error
and a derived-value cycle and confirm each is reported with its source location.

**Acceptance Scenarios**:

1. **Given** the invoice example, **When** compiled, **Then** the precondition
   `invoice.amount <= user.approval_limit` appears in the IR as a less-or-equal node over two
   field references, not as a computed value.
2. **Given** `margin` derived from `revenue` and `cost`, and `high_risk` using `margin`,
   **When** admitted, **Then** admission succeeds and reports the order
   `margin` before `high_risk`.
3. **Given** derived values `a` using `b`, `b` using `c`, and `c` using `a`, **When** admitted,
   **Then** admission fails and names the full cycle `a → b → c → a`.
4. **Given** a precondition comparing a decimal field to a text value, **When** the author
   writes it in Python, **Then** the DSL raises at that line naming both types; **and given**
   the same ill-typed comparison in hand-written serialized IR, **When** submitted to the
   engine, **Then** it is rejected on admission with the location and the conflicting types.
5. **Given** an action body that uses `if invoice.amount > 100:`, **When** the action is defined,
   **Then** the author gets an immediate error explaining that symbolic values cannot drive
   Python control flow.
6. **Given** the same behavior compiled twice on different machines, **When** the serialized IR
   is compared, **Then** it is byte-identical and has the same behavior version identifier.

---

### User Story 2 - Execute actions deterministically with a decision record (Priority: P2)

A capability caller asks to perform an action (e.g. approve invoice 1042 as user Anna) with the
current state (the invoice), the call's input arguments, and context (the acting user, Anna).
The engine checks preconditions, applies the declared effects, checks postconditions and
invariants, and returns either the resulting changes or a refusal. Every call yields a
decision record: behavior version, data version, conditions evaluated with their values, and
result (ALLOW, DENY, or INVALID_STATE, with reasons).

**Why this priority**: this turns validated behavior into a working system core. The decision
record delivers the auditability promised by the concept.

**Independent Test**: execute `approve_invoice` for each branch (allowed, wrong status, wrong
role, above limit) and confirm results and records; re-run the same calls and confirm
byte-identical records.

**Acceptance Scenarios**:

1. **Given** a pending invoice of 43,200 and a manager with limit 50,000, **When**
   `approve_invoice` is executed, **Then** the result is ALLOW, the change set sets the invoice
   status to `approved`, and the record lists each condition with its input values
   (`amount = 43200`, `approval_limit = 50000`), behavior version, and data version.
2. **Given** the same invoice but amount 60,000, **When** executed, **Then** the result is DENY,
   the change set is empty, and the record names the failed precondition.
3. **Given** an action whose effects would break an invariant, **When** executed, **Then** the
   whole action is rejected, no change is returned, and the record names the violated invariant.
4. **Given** an invoice whose incoming data already breaks an invariant (e.g. a negative
   amount), **When** any action on it is executed, **Then** the result is `INVALID_STATE`,
   preconditions are not evaluated, no change is returned, and the record names the invariant
   and the values read.
5. **Given** a decision record from an earlier run, **When** it is replayed with the same
   behavior and input data, **Then** the same result and record are produced.
6. **Given** an amount exactly equal to the limit (50,000 vs 50,000.00), **When** executed,
   **Then** the comparison is exact and the result is ALLOW.

---

### User Story 3 - AI acts only through capabilities (Priority: P3)

An AI interface (in a later feature) translates a human request ("Godkänn faktura 1042") into a
structured intent. In this feature, intents are supplied as recorded fixtures. An intent names
only the capability, the ids of the state entities it targets, and its input arguments. The
trusted host that runs the engine supplies the current state, the context (such as the
authenticated acting user), and the data version; the AI can never supply these. The system
checks the intent, executes it through the engine (User Story 2), and returns a structured
result plus the decision record. The AI has no other way to read or change data.

**Why this priority**: this completes the runtime boundary of the concept but depends on P1 and
P2 and can be demonstrated with recorded intents without a live AI.

**Independent Test**: submit recorded intents — valid, naming an unknown capability, with
malformed arguments, and valid but denied by rules — and confirm each gets a structured result
and only the allowed one produces changes.

**Acceptance Scenarios**:

1. **Given** a structured intent for `approve_invoice` targeting invoice 1042, and a host that
   supplies invoice 1042 and the acting user, **When** submitted, **Then** it is executed exactly
   as the equivalent direct request and the decision record is identical.
2. **Given** an intent naming an undeclared capability, **When** submitted, **Then** it is
   rejected before evaluation, with no changes.
3. **Given** an intent with wrong types, missing arguments, or extra fields, **When** submitted,
   **Then** it is rejected with a structured error listing every problem.
4. **Given** an intent targeting invoice 9999 while the host supplies invoice 1042, **When**
   submitted, **Then** it is rejected (`TARGET_MISMATCH`) with no evaluation.
5. **Given** an intent that includes a `context` or `state` section, **When** submitted,
   **Then** it is rejected; context and state come only from the host.

---

### Edge Cases

- Input data is missing a field used by the behavior: rejected as invalid input with a
  structured error; the engine never guesses a value.
- Division by zero in a derived value (e.g. `margin` with `revenue = 0`): evaluation stops with
  result `ERROR` naming the expression; no change is returned.
- Two effects in one action assign the same field: rejected at validation.
- An effect targets a context or input value (e.g. `set_(actor.approval_limit, …)`): rejected at
  validation.
- A `float` value or field type is used: rejected with guidance to use exact decimals.
- `invoice.amount + Decimal("10")` where `amount: Money`: type error; the author must write an
  explicit conversion or a `Money` literal.
- Reading `invoice.approved_by` as a user id without testing it: type error.
- Ids arrive in input data as plain strings: each takes its type from the field it fills, so
  inside behavior an `Id<User>` can never be mixed with an `Id<Project>` or a `String`; the
  engine does not inspect id contents.
- Chained or boolean Python operators (`a < b < c`, `and`, `or`, `not`) on symbolic values:
  rejected with guidance to use the explicit combinators.
- Empty behavior module: admission succeeds with `ok: true`, an empty `items` table, an empty
  `evaluation_order`, and a behavior version (the hash of the empty module).
- Only a comment, docstring, or source line changes: behavior version is unchanged.
- A rule is renamed without changing its body: the rule's hash is unchanged, the module hash
  changes (the name table changed), and decision records show the new name with the same rule
  hash.
- Two structurally identical expressions in different rules: they have the same hash.
- Derived values form a cycle: because references are by hash, a cycle has no content hash; it
  is detected on the name-resolved graph before hashing, and the module gets no behavior
  version.
- IR produced by an older or newer IR format version: rejected with a clear version error.

## Requirements *(mandatory)*

### Functional Requirements

**Authoring layer**

- **FR-001**: Authors MUST be able to define entities with typed fields, enumerations, derived
  values, rules (boolean derived values), invariants, and actions in Python, and collect them in
  a behavior module. An action declares three kinds of parameters: **state** (entity instances
  the action may change), **input** (typed arguments of the call, e.g. a rejection reason), and
  **context** (read-only facts about the call, e.g. the acting user), followed by
  preconditions, effects, and postconditions.
- **FR-002**: Field access and operators on symbolic values MUST build typed expression trees;
  the authoring layer MUST NOT evaluate behavior. Building an ill-typed expression (e.g.
  `invoice.amount + invoice.status`, comparing a decimal to text) MUST raise an error at the
  line where it is built. Using symbolic values in Python truth tests
  (`if`, `and`, `or`, `not`, chained comparisons) MUST raise an error at definition time.
- **FR-003**: Declarative statements MUST be explicit (`requires`, `ensures`, `set_`, and
  decorators for entities, actions, derived values, and rules); arbitrary Python in an action
  body MUST NOT become behavior.
- **FR-004**: Each IR element MUST carry the source location (file, line) it was defined at.

**Behavior IR and validation**

- **FR-005**: The Behavior IR MUST be a typed representation restricted to: literals, field
  references, derived-value references, comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`), boolean
  `and`/`or`/`not`, set membership, arithmetic (`+`, `-`, `*`, `/`), option tests, and explicit
  nominal conversions. No loops, recursion, or side effects other than declared assignments.
- **FR-005a**: The type system MUST consist of: primitives (`Bool`, `Int`, `Decimal`,
  `String`); the structural type `Option<T>`; and nominal types: enumerations, `Id<E>` (the
  identity of entity `E`), and user-declared nominal types over a primitive (e.g.
  `Money = nominal Decimal`). The engine MUST NOT contain domain-specific types such as money,
  dates, or percentages.
- **FR-005b**: Every entity MUST have an identity field `id` of type `Id<Self>`. `Id<A>` MUST be
  distinct from `Id<B>` and from `String`, even though all may be encoded as strings; e.g.
  assigning a project's id to `invoice.approved_by: Option<Id<User>>` MUST be a type error.
- **FR-005c**: A nominal type MUST declare which operations it supports, chosen from a fixed
  set: equality (always), ordering, addition/subtraction within the type, scaling by
  `Int`/`Decimal` (result keeps the nominal type), and ratio (dividing two values of the type
  gives `Decimal`). Any operation not declared, and any mix of a nominal type with its
  underlying primitive or another nominal type, MUST be a type error. Converting between a
  nominal type and its underlying primitive MUST be an explicit expression.
- **FR-005d**: `Option<T>` values MUST only be used through explicit tests (`is_none`,
  `is_some`), equality with another `Option<T>` or with "none", or an explicit
  default (`value_or`). A value of `T` MAY be assigned to or compared with `Option<T>` (it
  is wrapped as present). Using an `Option<T>` where `T` is required MUST be a type error.
- **FR-006**: The IR MUST have a canonical serialized form: the same behavior always produces
  byte-identical output. Serialization is separate from identity (FR-006a).
- **FR-006a**: Every semantic node (literal, field reference, operator, predicate, effect,
  derived value, rule, invariant, action, entity, enumeration, module) MUST have a content hash
  computed recursively from its canonical semantic form and its children's hashes, prefixed by a
  domain-separation tag naming the node kind and hash format version (e.g.
  `behavior.expr.v1`). Changing the serialization format MUST NOT change any hash.
- **FR-006b**: Hashes MUST cover only what affects behavior. Included: node kinds, types,
  literal values, operator structure, entity, field, enumeration value, and parameter names
  (they define the shape of state, input, and context). Excluded: source locations,
  documentation, and comments. Derived values, rules, invariants, and actions are referenced
  inside other nodes by their content hash, not their name; the module binds each symbolic
  name to an item hash (like a Git tree binds file names to blobs). Renaming a rule therefore
  changes the module hash but not the rule's hash, and editing a comment changes neither.
- **FR-006c**: The behavior version identifier MUST be the module hash. Identical behavior MUST
  have identical identity regardless of declaration order in the Python source where order has
  no semantic meaning (e.g. the set of entities); where order is semantic (preconditions are
  evaluated in order), it is part of the hash.
- **FR-007**: Serialized IR MUST be treated as untrusted. The engine MUST parse, resolve names,
  and type-check it into a Typed Behavior IR in one admission step, reporting every error with
  source location and a human-readable explanation, in a deterministic order. Only typed
  semantic IR MAY be hashed, validated further, or evaluated; invalid IR MUST NOT enter the
  semantic engine. This step is called **admission**; "validation" in this spec means
  admission.
- **FR-008**: The system MUST derive the reference graph between derived values and rules from
  the IR, reject cycles (naming the full cycle), and derive a deterministic evaluation order.
  Other views of the graph (fields, actions, Datalog facts) are derived later from the same
  references.
- **FR-009**: (merged into FR-007)

**Execution**

- **FR-010**: The system MUST execute an action as a transition from current state, input, and
  context to a proposed new state, in this order: check input shape and types; check every
  invariant that applies to the state parameters on the current state (any violation →
  `INVALID_STATE`); check preconditions; compute effects against the current state; check
  postconditions and invariants on the proposed state. If any check fails, no change is
  returned (all-or-nothing). Transitions preserve validity; they never establish validity from
  an invalid starting state.
- **FR-010a**: Effects MUST only target fields of state parameters; an effect targeting input or
  context MUST be rejected at validation. Input and context values MAY be read by any
  expression.
- **FR-011**: Execution MUST be deterministic: the same behavior version, input data, and data
  version MUST produce byte-identical results and decision records.
- **FR-012**: Every execution MUST produce a decision record containing behavior version, data
  version, action (name and hash), state, input, and context used, each condition evaluated with
  its hash and the values it read, every derived value, rule, and invariant used (name and
  hash), result (`ALLOW`, `DENY`, `ERROR`, `INVALID_INPUT`, or `INVALID_STATE`), reasons, and
  the change set.
- **FR-013**: The system MUST replay a decision record and confirm it reproduces the result.
- **FR-014**: The engine MUST operate on state, input, context, and a data version supplied by
  the caller and return a change set over state only; storing data is outside this feature.
- **FR-015**: Numeric comparisons and arithmetic, including on nominal types over `Decimal`
  or `Int`, MUST be exact; floating-point values MUST be rejected.

**Capability boundary**

- **FR-016**: Every action MUST be exposed as a capability. Its input parameters and state
  targets are chosen by the caller of the intent; its state and context values are supplied by
  the trusted host.
- **FR-017**: The system MUST accept structured intents (capability name, state target ids,
  input arguments), reject unknown capabilities, wrong types, missing or extra arguments or
  targets, and targets that do not match the supplied state, before evaluation, and return
  structured errors listing every problem.
- **FR-018**: Intents from any caller, including an AI interface, MUST go through the same
  checks and produce the same decision records; there is no other access path to behavior.
- **FR-019**: AI integration in this feature is limited to the capability boundary, tested with
  recorded structured intents only; no live AI model is connected.

### Key Entities

- **Entity / Field / Enumeration**: the information model (e.g. Invoice with amount: Money,
  status: InvoiceStatus, approved_by: Option<Id<User>>; User with role and approval limit).
- **Type**: a primitive, `Option<T>`, an enumeration, `Id<E>`, or a nominal type over a
  primitive with its declared operations.
- **Expression**: a typed tree of literals, field references, derived references, and operators.
- **Wire IR / Typed Behavior IR**: the untrusted serialized form exchanged across boundaries,
  and the resolved, type-checked form that alone is hashed and evaluated.
- **Derived value / Rule**: a named expression over entity inputs; a rule is a boolean derived
  value. May depend on other derived values.
- **Invariant**: a predicate over an entity that defines valid state; it must hold before and
  after every ordinary transition.
- **Action (transition)**: a named operation over typed state parameters (changeable),
  input parameters and context (read-only), with preconditions, effects on state, and
  postconditions.
- **State / Input / Context**: the three parts of a transition request: entity instances that
  may change, the call's own arguments, and facts about the call such as the acting user.
- **Behavior Module**: the collection of all of the above plus a table binding symbolic names
  to item hashes; stored as one IR document; its content hash is the behavior version.
- **Content hash**: the identity of a semantic node, derived from its kind, content, and
  children's hashes; independent of names for humans, source metadata, and serialization.
- **Capability / Structured Intent**: the callable form of an action, and a request to call it
  (capability, state target ids, input arguments).
- **Host context**: state, context, and data version supplied by the trusted caller, never by
  the intent.
- **Decision Record**: the full trace of one execution (see FR-012).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An author can write the invoice approval example and get a validation result in
  under 5 minutes on first attempt using the quickstart.
- **SC-002**: 100% of seeded defects in the reference test suite (type errors, unknown fields,
  cycles, duplicate assignments, Python control flow on symbolic values, float use) are reported
  with source location before execution.
- **SC-003**: 100% of executions produce a decision record, and 100% of replayed records
  reproduce the original result.
- **SC-004**: Compiling, validating, and executing the same inputs twice yields byte-identical
  output in 100% of runs, across machines.
- **SC-005**: Validation of a module with 200 derived values/rules and 50 actions completes in
  under 2 seconds, and a single action executes in under 10 milliseconds, on a developer laptop.
- **SC-006**: An auditor can answer "why was this decided?" for any decision from its record
  alone, without reading source files, in under 2 minutes.
- **SC-007**: 100% of invalid intents in the test suite are rejected with no changes.

## Assumptions

- No live AI model is used in this feature; AI behavior is represented by recorded intents.
- Python is the only authoring frontend in this feature; the IR is designed so other frontends
  can be added later without engine changes.
- The engine does not store data; callers supply input data and a data version and persist the
  returned change set.
- Permissions are expressed as preconditions in v0.1 (e.g. `requires(user.role == "manager")`);
  dedicated permission constructs come later.
- The invoice approval and project margin domains are used as reference examples and test data.
- Behavior versions are the module's semantic content hash (FR-006c); the Git revision is
  recorded alongside when available.
- Money is a domain-declared nominal type over `Decimal` in a single currency; currency-
  parameterized types (`Money<SEK>`) are a later feature.
- The specification is written in English to match the other project documents; domain examples
  keep Swedish wording where the input used it.
