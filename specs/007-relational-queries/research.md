# Research: Relational Queries and Set Semantics

Decisions for feature 007. Inputs: `spec.md` (FR-001–FR-025, FR-009a, FR-013a, FR-020a; the
clarifications of 2026-09-27) and the architecture of features 001–006 (evaluation facts,
`EvaluationFacts`, the store's as-of reads, the verifier's uninterpreted fact functions).

## R1. Language forms

- **Decision**: add these expression forms (wire 0.6), and no new module item kind except
  module-level invariants (R2):
  - `select(T)` — `Query<T>`, all existing entities of `T`;
  - `where(q, o → predicate)` — `Query<T>`, a narrower query; `o` is the candidate;
  - `union(q1, q2)`, `intersection(q1, q2)`, `difference(q1, q2)` — `Query<T>`, same `T` only;
  - `count(q)` — `Int`;
  - `any(q, o → predicate)`, `all(q, o → predicate)` — `Bool`;
  - `sum(q, o → expr)` — only for an additive numeric type with a canonical zero: `Int`, integer
    nominals with `add`, `Decimal`, decimal nominals with `add`, and exact types (never strings,
    booleans, enums or identities). The result type is the type of repeated exact addition under
    feature 004 (`Money` sums to range-checked `Money`, `Decimal` to `Exact<Decimal>`, `Int` to
    `Int`); `sum(∅)` is that type's exact zero;
  - `min(q, o → expr)`, `max(q, o → expr)` — `Option<E>` for an ordered type `E`, absent for ∅;
  - `unique(q, o → key)` — `Bool`.

  `Query<T>` is a new semantic type that only relational operators accept: it cannot be stored,
  compared, returned by a derived value, bound as a parameter, or used as a literal.
- **Rationale**: the smallest set covering the spec's needs. Typing `sum` as "the type of `+`"
  reuses feature 004's exactness and range rules instead of inventing aggregate typing.
- **Alternatives**: a generic `fold` (order-dependent in general, violates FR-020); SQL-like
  `group_by` (out of scope).

## R2. Candidate-local predicates and module-level invariants

- **Decision**:
  - **Predicate scope.** A lambda body (in `where`, `any`, `all`, `sum`, `min`, `max`, `unique`)
    is type-checked in a scope with the candidate (an entity of type `T`) plus the enclosing
    scope's parameters. Admission refuses, in a body: another relational form, `exists`,
    `referenced`, a derived value applied to anything but the candidate alone, and any reference
    to an enclosing *entity* parameter other than reading its fields as captured values. There is
    no nesting, so the candidate is always the only bound variable.
  - **Captured values** are the reads in the body that do not involve the candidate: enclosing
    inputs, context values and fields of enclosing entity parameters (bound state entities, or the
    parameters of a derived value). They are evaluated in the enclosing phase (S or S', FR-013a),
    and each capture of a bound entity's field is also an ordinary observed field read.
  - **Where queries may appear.** Preconditions, postconditions, rules, derived values and
    module-level invariants. Entity constraints and per-entity invariants containing a relational
    form are refused (`QUERY_NOT_ALLOWED`).
  - **Module-level invariants** are a new variant of invariant with no entity parameter (wire:
    `entity` and `param` omitted, 0.6 only), hashed under their own tag, named in the name table
    as invariants. They are **closed state expressions**: their scope has no parameters, so they
    cannot read inputs, context, action parameters or invocation-specific captures (their lambda
    bodies can capture only literals). Admission refuses anything else.
- **Rationale**: FR-003 and FR-004 as clarified. For a fixed instance, an unchanged candidate's
  membership is a pure function of its own value, which makes R4, R6 and R10 possible.
- **Alternatives**: allowing `exists` in filters (clarification Q1: rejected); per-entity
  relational invariants (Q2: rejected).

## R3. Query identity

- **Decision**:
  - **Definition hash**: the expression hash of the query node, with the candidate normalized: the
    lambda parameter's name is not hashed (the candidate is encoded as a fixed marker), so
    `lambda o:` and `lambda order:` hash equally. Captured expressions are hashed as expressions.
    The source name of the lambda parameter is kept outside the hash for display and
    serialization.
  - **Captured values**: the canonical encodings of the capture reads, in canonical order (sorted
    by the reads' `(param, field)` names as they appear in the query).
  - **Instance identity**: `sha256("behavior.query_instance.v1" ‖ definition hash ‖ canonical
    captured values)`.
- **Rationale**: FR-008, FR-025. Python variable names and lambda parameter names never affect
  identity.
- **Alternatives**: hashing captured *values* into the definition (then the definition would not be
  shared between instances).

## R4. Evaluation: query facts and member values

- **Decision**:
  - **Facts interface.** `EvaluationFacts` gains two questions, answered as of the evaluated state S:
    - `query(entity, definition, captures) → [id]` — the canonical member list of an instance;
    - `field(entity, id, field) → value` — a field of an entity that is not bound.
  - **On S** (preconditions, incoming rules): the evaluator computes the instance's captures in S,
    asks `query`, and records a query fact. Quantifiers and aggregates visit members in canonical
    identity order (FR-020a). A bound member's fields come from the bound value; any other
    member's fields come from `field` and are recorded as field facts. `any`/`all` stop at the
    first decisive member, so later members' fields are neither read nor recorded; `count` reads
    no fields.
  - **On S'** (postconditions, outgoing rules, module invariants): captures are computed in S'.
    The S' result of that instance is **derived**: `base = query(...)` at S *for the S' captures*
    (the same instance as on S if the captures did not change, otherwise a new observation of S);
    then removed entities leave; every bound entity is re-classified by the predicate on its S'
    value; every created entity is classified on its value; every other base member keeps its
    membership (FR-003 for a fixed instance). Unbound members' field values are unchanged by the
    transition, so their S facts serve S'.
  - **One rule for both cases.** `Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS`.
    The baseline `Q(captures_S', S)` is an observed query fact against the original snapshot,
    even though its captures were evaluated in S'; a changed capture therefore needs no special
    case (clarification Q1).
- **Rationale**: FR-009, FR-009a, FR-012, FR-013, FR-013a, FR-020a. Records stay self-contained:
  membership plus the member values actually read.
- **Alternatives**: recording aggregate results as opaque facts (rejected in the review: the
  computation must be reproducible and explainable).

## R5. Supplied query facts (plain evaluation)

- **Decision**: the request `facts` section gains `queries` and `fields`, with the record's shape.
  Missing facts give `UNKNOWN_FACT`. The snapshot check (FR-011) adds, closed under subsets:
  - every listed member is not marked absent or unused;
  - a bound entity of the queried type is listed iff the predicate holds for its bound value
    (evaluated with the instance's captures);
  - an entity with supplied field facts sufficient to decide the predicate is listed iff the
    predicate holds (a listed member that fails it, or an existing unlisted entity that satisfies
    it, is `INCONSISTENT_FACTS`);
  - one instance is given at most once, and field facts for bound entities must equal the bound
    values.
- **Rationale**: recorded (observed) facts always pass again on replay; supplied facts cannot
  describe an impossible state.

## R6. Store evaluation and indexes

- **Decision**:
  - **Backend primitives** (additions):
    - `keys_at(entity_type, position) → [EntityKey]`: every entity of the type existing at the
      position (a type index);
    - `keys_by_field_at(entity_type, field, value, position) → Option<[EntityKey]>`: an optional
      field index (default: `None`, meaning "not indexed").
  - **Query answering.** The store evaluates the predicate itself over candidates as of the
    evaluated position, reading each candidate with `version_at`. If a top-level conjunct of the
    predicate is `o.f == <captured or literal>` and the backend answers `keys_by_field_at` for it,
    only those candidates are read; otherwise all `keys_at`. Either way the predicate is
    re-evaluated on every candidate, so an index can only save work, never change a result
    (FR-016).
  - **The in-memory backend** maintains both indexes as derived edge events (like the reference
    index of feature 006), updated from the versions and removals of each commit. They are not
    state content; conformance checks them against a scan.
  - **Commit** re-derives the facts at the parent (as in 006), so a bundle whose query facts differ
    is refused (`BUNDLE_INVALID`).
- **Rationale**: FR-012, FR-016, SC-007 (the benchmark precondition is an index for the
  predicate's field).
- **Alternatives**: host-side query languages (breaks FR-022 and determinism); engine-side scans
  only (misses SC-007).

## R7. Module-level invariants at runtime

- **Decision**:
  - **Genesis** evaluates every module invariant over the seed (facts answered from the seed).
  - **Transitions**: every module invariant is checked on S' (phase `invariant_global`), unless a
    **sound dependency analysis** shows the transition cannot affect it. An invariant's signature
    is the set of entity types it queries plus, per type, the fields its predicates, keys and
    aggregate bodies read. A transition affects it if it creates or removes an entity of such a
    type, or its field effects change such a field. On S the invariant is assumed (valid state),
    as reference constraints are in 006.
  - **Uniqueness by delta.** On S', `unique(q, o → key)` is decided without reading every member:
    for each touched candidate `c` (created, or bound with a changed key or membership) that is a
    member of `q'`, the engine asks for the S members of the derived instance
    `where(q, o → key(o) == key(c))` (indexable), applies the transition's changes, and requires no
    member other than `c`. Untouched pairs are distinct because `unique` held on S. The derived
    instances are recorded as ordinary query facts.
  - **Result**: a failing module invariant is `DENY` with `INVARIANT_VIOLATED`, naming the
    invariant.
- **Rationale**: FR-024 (correctness as if every invariant were checked; skipping requires proof of
  non-interference) with cost proportional to the change for the central `unique` case.
- **Alternatives**: checking every invariant on every transition (simple, but a full scan of
  every uniqueness set per commit); maintaining aggregate indexes (deferred).

## R8. Records, bundles and versions

- **Decision**:
  - **Record `facts`** gains `queries: [{instance, definition, entity, captures, members}]` (sorted
    by instance id, members sorted by id) and `fields: [{entity, id, field, value}]` (sorted).
  - **Record version.** A record with query or field facts is `record_version "0.6"`; 006-only
    records stay `0.5`, and all others `0.4`.
  - **Bundle.** `read_facts` carries the same sections, so FR-014 holds with full results; a
    result hash per instance is also included for future fine-grained conflict detection (FR-015).
  - **Wire.** Any 007 form needs `ir_version "0.6"`; 0.4 and 0.5 documents without them stay
    valid and serialize as before.
- **Rationale**: FR-014, FR-015, FR-023, SC-006.

## R9. Hashing

- **Decision**: new expression codes for `select`, `where`, the three set operations, `count`,
  `any`, `all`, `sum`, `min`, `max`, `unique`, and the candidate marker; a new type code for
  `Query<T>`; a new tag for module invariants. Nothing existing changes code, so every earlier
  module keeps its identity (checked against `frozen_versions_006.json` and a new snapshot).
- **Rationale**: SC-006.

## R10. Verification: symbolic S, exact change

- **Decision**:
  - **Per query instance and phase**, symbolic summaries over S: `count_Q ≥ 0`; for each `sum` body
    `sum_Q`; for each `any`/`all` predicate `P` a satisfying count `sat_Q,P` with
    `0 ≤ sat ≤ count`; for `min`/`max` an optional value `m_Q`. **Known candidates** (the
    action's bound entities of type `T`) are classified by the predicate on their S values, and
    the summaries are constrained to include them (`count_Q ≥` known members, `sum_Q` = known
    contributions + an unknown remainder, and so on).
  - **S' for the same instance** (captures equal in S and S', as a path condition): exact deltas
    for every touched candidate: removed (leaves if it was a member), bound (re-classified on its
    S' value), created (classified on its value). `count` and `sum` compose exactly; `any`/`all`
    through the satisfying counts; `min`/`max` compose for creations (`min(m, new)`) and otherwise
    become a fresh symbol.
  - **S' for a changed instance**: fresh summaries for the new instance over S, constrained only
    by the known candidates, then the same deltas.
  - **Normalization**: quantifier satisfying counts are the counts of the narrowed instances
    (`any(q, p) ≡ count(where(q, p)) > 0`, `all(q, p) ≡ count(where(q, not p)) == 0`), and the
    `unique` delta rule's "other members with key k" is the count of `where(q, key == k)`, so
    equal questions share one summary.
  - **Module invariants** are assumed on S (`unique` as a flag on its instance; comparisons such as
    `sum_Q ≥ 0` as constraints) and checked on S'. `unique` on S' is encoded by the R7 delta rule
    over symbolic "other members with this key" counts.
  - **Soundness for free where precision is lost**: a fresh symbol only weakens what can be proven,
    and a spurious counterexample is refused by runtime confirmation (inconclusive).
  - **Counterexamples**: a second, concretizing query adds a small number of explicit unknown-member
    slots (each a symbolic entity satisfying the predicate) so that `count`, `sum` and the
    satisfying counts equal the known plus slot contributions; the model then becomes concrete
    query and field facts. If no concrete model is found within the slot bound, the check is
    inconclusive. The slots are witnesses only: they never impose a finite-universe bound on a
    proof, and "proven" comes solely from the unbounded symbolic summaries.
  - **Check kinds**: module invariants are preservation checks (subject kind `invariant_global`);
    postconditions and evaluation errors work as before.
- **Rationale**: FR-017, FR-018, SC-005. The verifier need not know the whole state; it must know
  exactly how the transition changes what it knows.
- **Alternatives**: quantified SMT over uninterpreted sorts (undecidable in general, slow);
  bounded model checking as proof (unsound; clarification Q3).

## R11. Replay

- **Decision**: plain replay uses the recorded query and field facts. Store behavior replay
  re-derives facts at the parent and compares. Data replay is unchanged (queries never change
  state). A new conformance case checks the field and type indexes against a scan at every
  position.
- **Rationale**: FR-010, SC-003.

## R12. Python DSL

- **Decision**: `select(T)` returns a `Query` object with `.where(fn)`, `.union(q)`,
  `.intersection(q)`, `.difference(q)`; functions `count(q)`, `any_(q, fn)`, `all_(q, fn)`,
  `sum_(q, fn)`, `min_(q, fn)`, `max_(q, fn)`, `unique(q, by=fn)`. Lambdas are traced once with a
  symbolic candidate. `Query.__iter__`, `__len__` and `__bool__` raise `BehaviorDefinitionError`.
  `@invariant` on a function with no parameters declares a module-level invariant.
- **Rationale**: SC-001; queries are behavior expressions, not Python collections.

## R13. Scale

- **Decision**: records carry the members and the member field values a decision read. A large
  query result makes a large record even for `count` (a query fact lists its complete membership);
  aggregates and quantifiers that read member fields add those field reads on top. This is
  accepted in this feature. Content-addressed result objects are a later storage choice (FR-010)
  and change no semantics.
- **Rationale**: correctness first; SC-007 covers selective queries.

## R14. Compatibility

- **Decision**: `exists`, `referenced` and `Ref<T>` keep their forms, hashes and records (FR-019).
  Records without query or field facts keep their bytes. Store documents gain only optional
  fields.
- **Rationale**: SC-006.
