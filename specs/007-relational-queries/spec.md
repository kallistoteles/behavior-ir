# Feature Specification: Relational Queries and Set Semantics

**Feature Branch**: `007-relational-queries`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "Feature 007 — Relational Queries and Set Semantics: typed,
deterministic, read-only relational queries over state, so behavior can reason declaratively about
sets of entities" (full description in the conversation; summarized below).

## Context

After features 001–006, behavior can reason about the entities an action binds explicitly, and
about a few universe-level facts: whether an identity exists (`exists`), whether any surviving
reference points at it (`referenced`), and whether the store has ever used it (the history fact
behind `ENTITY_ID_ALREADY_USED`).

Real applications also reason about **sets** of entities:
- all open orders of a customer;
- whether any unpaid invoice exists;
- how many active contracts there are;
- the total value of matching transactions;
- whether two entities violate a uniqueness rule.

None of this can be expressed today without the host loading entities and passing them in, which
hides the dependency from verification, replay and conflict detection.

This feature introduces the smallest relational abstraction that covers these needs: a typed set
comprehension over one entity type, with quantifiers, counts, aggregates and set algebra. It is
not SQL, and it adds no bulk mutation.

```text
Query<T> = { id ∈ Universe<T>(S) | predicate(entity(id), captured values) }
```

A query result is a mathematical set of typed identities: no order, no duplicates.

**Acceptance invariant**:

> A relational query is a deterministic, typed and reproducible observation of an exact state
> version; its result may influence behavior, but neither storage layout nor evaluation order may
> influence its meaning.

**Guiding principles**:
- Queries describe facts about state; they do not define storage access.
- Sets are semantic; indexes are implementation.
- Queries are pure state reads.
- Query meaning is independent of iteration order.
- Every query result that influences evaluation is part of the reproducible state snapshot.
- Exactness and type semantics continue through aggregation.
- Unsupported proof is inconclusive, never guessed.
- Local invariants describe entities; global invariants describe relations. (Entity constraint:
  intrinsic validity; per-entity invariant: local state validity; module invariant: global state
  validity; pre/postcondition: invocation-relative validity.)
- Name behavior and domain meaning; hash expressions by semantics.

## Clarifications

### Session 2026-09-27

- Q: How are member values consumed by aggregates and quantifiers recorded? → A: The query fact
  records membership only; member values that are read are ordinary observed field reads (FR-009,
  FR-009a).
- Q: How are a query and its parameters identified? → A: A query definition hash (type, predicate,
  operations) is distinct from a query instance identity = hash(definition hash, canonical captured
  values). Captured fields of bound entities are also ordinary observed field reads (FR-008,
  FR-009).
- Q: In which state are captured values evaluated? → A: In the enclosing semantic state: S for
  incoming rules and preconditions, S' for postconditions, outgoing rules and invariants on the
  resulting state (FR-013a).
- Q: When must relational invariants hold? → A: At genesis and on every resulting state.
  Dependency analysis may skip provably unaffected invariants as an optimization; correctness is
  defined as if all invariants were checked (FR-024).
- Q: How do short-circuiting operations over unordered sets stay deterministic? → A: Sets have no
  semantic order, but `any`, `all` and any other operation that may stop early visit members in
  canonical entity-identity order, so backend iteration order never affects traces or observed
  reads (FR-020a).
- Q: Is an index required? → A: No. An index may be a stated precondition of a performance
  benchmark (SC-007), never a semantic requirement: a missing index affects only speed (FR-016).
- Q: Should a query's filter read only the candidate entity plus inputs, context and captured
  values, or also other entities and queries? → A: Candidate-local only. Nested queries,
  `exists`/`referenced` and reads of other entities are admission errors. For a fixed query
  instance, an unchanged candidate's membership is stable; a changed captured value makes a
  different query instance. Cross-entity filtering will come later through explicit relational
  operators (for example a semijoin), not hidden reads inside filters (FR-003, FR-013).
- Q: Where may relational invariants live? → A: Only at module level. Queries may appear in
  module-level invariants, preconditions, postconditions, rules and derived values, but not in
  entity constraints or per-entity invariants, which stay local to one entity. Grouped rules such
  as "every customer has at most 3 open orders" are out of scope: FR-003 is not weakened for
  them; an explicit grouping operator may come later (FR-004, FR-024).
- Q: What must the verifier prove about queries? → A: Symbolic evaluated state, exact transition
  effect: relational facts about a valid S are symbolic (constrained by runtime guarantees,
  preconditions and module invariants assumed on S); for a fixed query instance (same definition
  and captured values in S and S') S' membership and supported aggregates are derived exactly from
  S plus the action's changes; a changed capture makes a new symbolic instance. Precision is
  operator-specific (count/sum exact deltas; quantifiers and uniqueness by sound constraints;
  min/max may be inconclusive). Bounded model checking is not part of proof (FR-017).
- Q: Are queries inline expressions or named module items? → A: Inline expressions only; no new
  module item kind. Identity comes from the definition hash (plus captured values for an
  instance). Python variable names are authoring reuse with no semantic identity; reusable domain
  concepts use derived values (for example `open_order_count(customer)`). Source names shown in
  diagnostics are non-semantic metadata (FR-008, FR-025).

## User Scenarios & Testing *(mandatory)*

The actors:
- a **behavior author**, who describes what the application may do;
- a **host developer**, who runs the engine and its store;
- an **auditor**, who must reconstruct and verify decisions.

### User Story 1 - Decide over a set of entities (Priority: P1)

A behavior author writes `close_customer`, which may remove a customer only if the customer has no
open orders. The author declares the set of the customer's open orders as a query over `Order`
(filtered by `customer` and `status`) and requires that its count is zero. Evaluating the action
against a store answers the query as of the evaluated state; the decision records which orders
matched.

**Why this priority**: deciding over "all X such that …" is the most common relational need and
the one the engine cannot express at all today.

**Independent Test**: a store with a customer and orders in various states.
- `close_customer` for a customer with no open orders is allowed, and its record shows the query
  and its empty result.
- For a customer with one open order it is denied, and the record shows that order in the result.

**Acceptance Scenarios**:

1. **Given** customer C with orders O1 (closed) and O2 (closed), **When** `close_customer(C)` is
   evaluated, **Then** it is allowed and the record contains the query fact for "open orders of C"
   with an empty result.
2. **Given** customer C with an open order O3, **When** `close_customer(C)` is evaluated, **Then**
   it is denied by the precondition, and the recorded query result is exactly `{O3}`.
3. **Given** a removed order that was open before its removal, **When** the query is evaluated,
   **Then** the removed order is not a member.
4. **Given** a query and the same state, **When** it is evaluated by stores that iterate entities
   in different orders, **Then** the results and the decision records are byte-identical.

---

### User Story 2 - Quantifiers, counts and aggregates (Priority: P1)

A behavior author requires that every order in a set has a positive amount (`all`), that none is
blocked (`not any`), and that the total of a customer's unpaid invoices stays below a credit limit
(`sum`). The author also uses `min` and `max`. Arithmetic in aggregates is exact, as elsewhere.

**Why this priority**: counts and totals over sets drive most business rules (credit limits,
quotas, balances).

**Independent Test**: evaluate actions using each quantifier and aggregate over sets of 0, 1 and
many members, including the empty set.

**Acceptance Scenarios**:

1. **Given** an empty set, **When** `any`, `all` and `count` are evaluated, **Then** they give
   `false`, `true` and `0`.
2. **Given** invoices with amounts 10.00, 20.00 and 0.01, **When** their `sum` is evaluated,
   **Then** the result is exactly 30.01 of the same money type, with no rounding.
3. **Given** an empty set, **When** `min` or `max` is evaluated, **Then** the result is an absent
   optional value, never a sentinel.
4. **Given** two sets over the same entity type, **When** their union, intersection and difference
   are used, **Then** they follow ordinary set semantics; combining sets of different entity types
   is refused at admission.
5. **Given** a `unique` rule over employees by personnel number, **When** an action would create a
   second employee with an existing number, **Then** the rule fails on the resulting state and the
   transition is refused.

---

### User Story 3 - Queries on the resulting state (Priority: P2)

A behavior author writes `place_order`, which creates an order and ensures afterwards that the
customer's order count grew by exactly one and the total stays within a limit. Postconditions and
rules that run after the effects query the proposed resulting state, which exists only inside the
evaluation and is never written to storage first.

**Why this priority**: postconditions and invariants over sets are how authors state what an action
promises; without them relational rules can only be checked before the change.

**Independent Test**: create, update and remove entities that match and do not match a query, and
check that the query on the resulting state reflects exactly those changes.

**Acceptance Scenarios**:

1. **Given** a customer with 2 orders, **When** `place_order` creates a third, **Then** a
   postcondition `count(customer_orders) == 3` holds on the resulting state.
2. **Given** an action that changes a bound order's status from open to closed, **When** the
   "open orders" query is evaluated on the resulting state, **Then** that order is no longer a
   member, and every other member is unchanged.
3. **Given** an action that removes a matching entity, **When** the query is evaluated on the
   resulting state, **Then** the removed entity is not a member.
4. **Given** any action, **When** it is evaluated, **Then** no speculative state is written to the
   store.

---

### User Story 4 - Reproducible, verifiable and auditable queries (Priority: P2)

An auditor replays a committed decision that depended on a query, without access to the live store
state it was made on. A behavior author verifies relational rules and gets either a proof, a
runtime-confirmed counterexample, or an inconclusive result — never a guessed proof. A host
developer evaluates a module without a store by supplying query facts, like the existing evaluation
facts.

**Why this priority**: queries only belong in the engine if they keep its guarantees (replay,
verification, determinism, content addressing).

**Independent Test**: record decisions that use queries, then replay them from the records alone;
verify modules with seeded relational defects; tamper with recorded query results.

**Acceptance Scenarios**:

1. **Given** a committed decision that used a query, **When** it is replayed, **Then** the replay
   uses the recorded query result and reproduces the decision byte for byte, without querying the
   store.
2. **Given** a recorded query result that was altered, **When** the history is replayed against
   the store, **Then** the divergence is reported at its position.
3. **Given** a plain evaluation without a store that reaches a query whose result is not supplied,
   **When** it is evaluated, **Then** the result is the evaluation error `UNKNOWN_FACT`.
4. **Given** a module with a seeded relational defect (for example a missing guard allowing an
   order total above a limit), **When** it is verified, **Then** a counterexample is reported that
   the runtime reproduces with its supplied query facts.
5. **Given** a relational property beyond the verifier's supported reasoning or budget, **When**
   it is verified, **Then** the check is inconclusive (a blocking finding), not proven.

---

### Edge Cases

- **Empty sets:** `any` is false, `all` is true, `count` is 0, `sum` is zero of the aggregated type,
  `min`/`max` are absent.
- **Removed entities** are never members, including in queries on the resulting state of the
  transition that removes them.
- **Created entities** are members of queries on the resulting state if they match, and never of
  queries on the evaluated state.
- **A query whose captured values differ** (for example the same query for two different
  customers) is a different observation with its own result.
- **Iterating a query in Python** (`for order in orders`) is refused with a definition error; a
  query is a behavior expression, not a Python collection.
- **A predicate that reads another query or an unrelated entity** is refused at admission.
- **A query used as an effect target** (bulk update) is refused at admission.
- **A query inside an entity constraint or a per-entity invariant** is refused at admission.
- **A grouped rule** ("every customer has at most 3 open orders") cannot be expressed: it needs a
  query inside a quantifier's predicate, which FR-003 forbids. It is out of scope until an explicit
  grouping operator exists.
- **Aggregating incompatible types** (for example summing a string field, or `min` over an
  unordered type) is refused at admission.
- **Very large results:** meaning does not depend on result size; representation (for example
  storing large results as separate content-addressed objects) is a storage choice.
- **Supplied query facts that contradict other supplied facts** (for example a member that is
  marked absent, or a member whose supplied field values do not satisfy the predicate) are refused
  as `INCONSISTENT_FACTS`.

## Requirements *(mandatory)*

### Functional Requirements

**Queries**

- **FR-001 — Typed entity queries**: `select(T)` MUST denote the set of currently existing entities
  of type `T`, identified by `Id<T>`. It never includes removed entities. `where(predicate)`
  MUST narrow a query to the members for which the predicate holds.
- **FR-002 — Set semantics**: query results MUST have mathematical set semantics: each identity at
  most once, no semantic order. Canonical serialization MAY sort identities, but order MUST NOT
  affect meaning or hashes. Positional operations (`first`, `last`, `nth`) are not provided.
- **FR-003 — Candidate-local predicates**: a predicate is a pure function
  `membership(candidate, captured values, input, context) → Bool`. It MAY depend only on the
  candidate entity's fields, derived values over the candidate alone, literals, inputs, context,
  and captured values from entities the action binds. A predicate MUST NOT contain another query,
  `exists`, `referenced`, a read of any other entity, recursion, or effects; each is an admission
  error. This guarantees that, **for a fixed query instance**, the membership of an unchanged
  candidate cannot change unless the candidate itself changes.
- **FR-004 — Queries are pure and relational**: query evaluation MUST NOT change state. Queries MAY
  be used in preconditions, postconditions, rules, derived values and **module-level invariants**,
  but MUST NOT produce effects. Entity constraints and per-entity invariants MUST NOT contain
  queries (an admission error): entity validity is intrinsic and checkable from the entity's own
  value (including input and context entities), while relational validity belongs to the state as
  a whole. Bulk effects over a query are out of scope.
- **FR-005 — Quantifiers**: `any(q, predicate)`, `all(q, predicate)` and `count(q)` MUST have
  mathematical semantics, including `any(∅) = false`, `all(∅) = true`, `count(∅) = 0`. Their
  predicates follow the rules of FR-003.
- **FR-006 — Aggregates**: `sum`, `min` and `max` over an expression of the candidate MUST be
  available for compatible types. `sum` MUST require an additive numeric type with a canonical
  zero (integers, decimals, and numeric nominals declaring addition; never strings, booleans,
  enums or identities). Its result type is the type produced by repeated exact addition under
  feature 004 (for example `Money` sums to range-checked `Money`), it never rounds implicitly, and
  the empty sum is that type's exact zero. `min` and `max` require an ordered type and return an
  optional value, absent for the empty set.
- **FR-007 — Set algebra**: union, intersection and difference of queries over the same entity type
  MUST follow ordinary set semantics. Combining queries over different entity types MUST be
  refused at admission.
- **FR-008 — Query identity**: every normalized query MUST have a **query definition hash**
  covering the entity type identity, the normalized predicate, the captured expressions (as
  expressions, not values) and the query operations, and excluding source locations and
  presentation. Queries that are equal after the existing canonicalization rules MUST hash
  equally. A **query instance identity** is the hash of the definition hash and the canonical
  captured values: "open orders of C1" and "open orders of C2" share a definition and are
  different instances. The same instance at the same state version always has the same result.
- **FR-025 — Queries are expressions**: a query MUST be an inline behavior expression, not a new
  kind of named module item. Its identity is its definition hash (FR-008) wherever it appears; two
  occurrences that normalize equally, under any Python variable names, share a definition. Reusable
  domain concepts MUST use the existing derived values, whose parameters supply captured values
  (for example a derived `open_order_count(customer)` counting a query). Derived values return
  values (Bool, counts, sums, optional values), not queries, in this feature. Source names or labels
  of queries used in diagnostics are non-semantic metadata and never affect hashes.
- **FR-021 — Uniqueness**: `unique(q, by=key)` MUST mean that no two distinct members of `q` have
  equal keys. It MAY be used in module-level invariants and conditions.
- **FR-022 — No hidden entity loading**: behavior MUST discover entity sets only through query
  semantics. No expression may obtain entities through host callbacks or any other channel.

**Observation, replay and evaluation**

- **FR-009 — Query facts**: evaluating a query MUST be a state observation, recorded as a query fact
  identifying the query instance (definition hash plus canonical captured values) and its
  canonical result, the set of matching identities. The query fact records **membership only**.
  Only queries actually evaluated are recorded; short-circuited ones are not. A captured value
  read from a bound entity's field is also an ordinary observed field read: query instances do not
  replace dependency tracking.
- **FR-009a — Member values are observed facts**: a quantifier, count or aggregate that reads fields
  of members MUST record those field values as observed field reads, so the decision can be
  recomputed from the record alone.
- **FR-010 — Replay without the live store**: replay MUST use the recorded query facts and member
  values and MUST NOT query the store. Replaying against the store MUST also check that the
  recorded results equal what the store's history gives at that position.
- **FR-011 — Plain evaluation**: evaluation without a store MUST accept query facts (and member
  values) in the request, like the existing evaluation facts. A needed but missing query fact
  MUST give `UNKNOWN_FACT`; the engine never guesses a result. Supplied query facts MUST be
  consistent with the other supplied facts and with the predicate, or the result is
  `INCONSISTENT_FACTS`.
- **FR-012 — One snapshot**: in a store evaluation, every query result MUST describe the same
  state and position as every other read of that evaluation (`data_version`: store, state,
  position), even while commits land concurrently.
- **FR-013 — Queries on the resulting state**: postconditions, rules and invariants evaluated after
  the effects MUST query the proposed resulting state S'. The engine MUST derive S' results from
  the S result plus the changes, reconsidering only created entities, removed entities, and bound
  entities whose fields changed; every other candidate keeps its membership (guaranteed by FR-003
  for the same query instance). If a captured value differs between S and S' (for example the
  action changes `customer.credit_limit` that the filter compares with), the resulting-state query
  is a **different query instance**. In every case one rule applies:
  `Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS`. The baseline
  `Q(captures_S', S)` is an observed query fact read against the original snapshot S, even though
  its captured values were evaluated in S'. Nothing is written to storage before the decision is
  final.
- **FR-013a — Captures follow the enclosing state**: captured values MUST be evaluated in the
  enclosing semantic state: S for incoming rules and preconditions, S' for postconditions,
  outgoing rules and invariants on the resulting state. For example, after
  `set_(customer.region, NORTH)`, a postcondition query filtering on `o.region == customer.region`
  uses `NORTH`.
- **FR-014 — Observed query dependencies**: every query that influenced a decision MUST be part of
  the observed read set, with at least the query instance and a hash of its result, next to field,
  existence, identity and reference reads.
- **FR-015 — Future conflict semantics**: concurrency stays whole-state
  (`evaluated_against == committed_on`). The recorded query dependencies MUST be sufficient for a
  later fine-grained rule to treat a changed query result as a changed dependency, even when every
  previous member is unchanged.
- **FR-016 — Indexes do not define semantics**: a store MAY accelerate queries with derived
  structures (field, type, reference or materialized indexes). They MUST be reconstructible from
  canonical state and MUST NOT contribute to the state identity. A missing or rebuilt index MUST
  NOT change any result or make a query unavailable: without an index a query is slower, never
  different.
- **FR-020 — Order independence**: no query result or aggregate may depend on physical iteration
  order. An operation whose result would depend on order is not admitted unless it defines a
  canonical order-independent meaning.
- **FR-020a — Canonical evaluation order**: query sets have no semantic order, but every operation
  whose evaluation may stop early (`any`, `all`, and any later operation of that kind) MUST visit
  members in canonical entity-identity order. Backend iteration order MUST NOT affect results,
  traces or observed reads. This defines a deterministic evaluation strategy; it does not change
  what `any` and `all` mean.
- **FR-024 — Module-level invariants**: a module-level invariant has no entity parameter and
  is a **closed state expression**: it may contain queries over state, literals and state-derived
  values, but MUST NOT depend on inputs, context, action parameters or invocation-specific captured
  values (an admission error), so global validity is meaningful at genesis and independent of any
  invocation. It describes the state as a whole (for example `unique(select(Employee), by=personnel_number)` or
  `sum(select(Account), balance) >= 0`). It MUST hold on the genesis seed and on every resulting
  state (`Valid(S0)`, and `Valid(S) ∧ transition → Valid(S')`). The engine
  MAY skip checking an invariant on a transition only when a sound dependency analysis shows the
  transition cannot affect it; when in doubt it MUST check. Correctness is defined as if every
  invariant were checked on every resulting state: skipping requires proof of non-interference
  (for example, a uniqueness invariant over `Employee.personnel_number` is unaffected by a change of
  `Account.balance`, but not by creating or removing an employee or changing a personnel number).

**Verification**

- **FR-017 — Sound verification: symbolic S, exact change**: the verifier MUST model query
  semantics soundly without enumerating the universe.
  - **Evaluated state S:** relational facts about a valid S are symbolic, constrained only by what
    is guaranteed: runtime guarantees, admitted facts, preconditions, and module-level invariants,
    which hold on every valid S (so the task is to prove they are preserved into S').
  - **Fixed query instance** (same definition and same canonical captured values in S and S'): its
    S' membership and supported summaries are derived exactly from the symbolic S value plus every
    creation, removal and change of a candidate the action performs. Each touched candidate is
    classified under the path condition (member before? after? contribution before? after?); for
    example an unknown count `n` becomes exactly `n + 1` when one matching entity is created, and a
    sum `x` becomes `x + 20` when a matching amount of 20 is created.
  - **Changed query instance** (a captured value differs between S and S'): its S' result MUST NOT
    be derived from the old instance; it is a new symbolic value unless otherwise constrained.
  - **Operator-specific precision:** `count` and `sum` support exact deltas; `any`, `all` and
    `unique` use sound symbolic constraints (for example counts of satisfying members); `min` and
    `max` may be inconclusive, for instance after the extremum is removed or changed.
  - Any property not implied by these constraints MUST be inconclusive (a blocking finding), never
    assumed. A bounded search for counterexamples MAY be added later as a search strategy, but the
    absence of a bounded counterexample MUST never produce a proof. Explicit unknown-member slots
    used to make a counterexample concrete are witnesses only; they never impose a finite-universe
    bound on a proof.
- **FR-018 — Confirmed counterexamples**: a relational counterexample MUST be confirmed by the
  normal evaluator with all the query facts and member values it needs; one that does not
  reproduce is inconclusive.

**Compatibility**

- **FR-019 — Lifecycle forms unchanged**: `exists`, `referenced` and `Ref<T>` keep their meaning,
  hashes and records. They MAY share query infrastructure internally.
- **FR-023 — Wire compatibility**: any semantic form introduced by this feature MUST require wire IR
  0.6. Documents of 0.4 and 0.5 that use none of them stay valid and keep their bytes and identities.
  Records gain query sections only when a query was observed; all others keep their bytes.

### Key Entities

- **Query**: a typed set comprehension over one entity type: the type, a candidate-local
  predicate, and captured values. Its identity is a semantic content hash.
- **Query result**: the canonical set of identities of the matching entities at one state.
- **Query definition / query instance**: the definition hash identifies what is asked; the instance
  identity adds the canonical captured values. Queries are expressions, never named module items.
- **Query fact / query read**: the observation "this query instance had these members at this
  state", recorded in the decision record and the read set. It records membership only; member
  field values read by quantifiers and aggregates are ordinary field reads.
- **Module-level invariant**: a rule over the whole state, without an entity parameter, that may
  use queries; checked at genesis and on every resulting state.
- **Relational expression**: a quantifier (`any`, `all`), `count`, an aggregate (`sum`, `min`,
  `max`), `unique`, or set algebra over queries.
- **Query index**: an optional, derived store structure that accelerates queries; never part of the
  state identity.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A behavior author can express "no open orders", "all amounts positive", "total of
  unpaid invoices below a limit" and "personnel numbers unique" in one behavior module, without
  host code that loads or passes entity sets.
- **SC-002**: For generated states and queries, the result and the decision record are identical
  across 1,000 random permutations of the store's iteration order and across rebuilt indexes.
- **SC-003**: In generated histories of 10,000 transitions using queries, data and behavior replay
  reproduce every decision from the records alone; every single tamper of a recorded query result
  or member value is found at its position.
- **SC-004**: For generated transitions (creations, removals, field changes of bound entities), the
  resulting-state query result derived by the engine equals a full re-evaluation of the query on
  the resulting state in 100% of 10,000 cases.
- **SC-005**: Verification fixtures cover all three outcomes of the proof boundary:
  - **proven:** a created matching entity makes a count exactly `n + 1`; `count(q) == 0` required
    and a non-matching entity created keeps it 0; uniqueness preserved when no existing member has
    the new key; a sum grows by exactly the created amount;
  - **counterexample** (runtime-confirmed): a transition that introduces a duplicate unique key or
    breaks a count or sum invariant;
  - **inconclusive** (never proven): a property depending on a changed capture (untouched entities
    may change membership), and a `min` after removing an entity that may be the current minimum.
- **SC-006**: Every existing module, record, golden file and persistence document (features
  001–006) keeps its bytes and identity; all existing tests pass unchanged.
- **SC-007**: Evaluating a selective query (matching at most 10 entities) in a store of 100,000
  entities completes in under 50 ms on the reference machine, and its cost does not grow with the
  number of non-matching entities. **Benchmark precondition:** an index exists for the
  predicate's fields. The same query without that index returns the same result, only slower.
- **SC-008**: For generated sets and store iteration orders, `any` and `all` produce identical
  traces and observed reads in 100% of 1,000 permutations.

## Assumptions

- **Member values are ordinary field reads** (FR-009a), not a new fact kind.
- **Record size**: large query results make large records, because a query fact lists its complete
  membership (even for `count`); aggregates and quantifiers that read member fields add those field
  reads on top. This is accepted for this feature; content-addressed result objects are a later
  storage choice that changes no semantics.
- **Concurrency stays whole-state** (feature 005); this feature only records the dependencies a
  finer rule will need.
- **Out of scope**: bulk mutation and `for_each` effects, general joins, cross-entity filters
  (to come later as explicit relational operators such as a semijoin, never as hidden reads in a
  filter), grouped or keyed cardinality rules (to come later as an explicit grouping operator), recursive queries and graph traversal, ordering, sorting, pagination, window functions, group-by, user-defined aggregates,
  cross-store and external database queries, derived values that return queries (a possible later
  extension, preferable to named query items), materialized-view semantics, and query-driven
  creation.
- **Formats change deliberately**: queries require wire IR 0.6 (FR-023); records with query facts
  get a new record version, all others keep theirs.
