# First Principles

> **A behavior system is an immutable, typed, content-addressed description of valid state
> changes and the conditions under which they may occur.**

The data model describes the world. The Behavior IR describes how the world may change. The AI
understands what the human wants to do with it.

Everything else (execution, verification, dependency graphs, solver models, traces,
documentation, AI capabilities, version history) is derived from that one description.

This file is the test for new ideas: a feature that breaks this model is declined or
reshaped until it fits. The project constitution
(`.specify/memory/constitution.md`) holds the binding engineering rules. Feature specs under
`specs/` hold concrete requirements.

## The core primitives

| Primitive | Question it answers |
|-----------|---------------------|
| **Type** | What kinds of values exist? |
| **State** | What is currently true? |
| **Expression** | How are values derived from state, input and context? |
| **Predicate** | What must or may be true? |
| **Transition** | How may state change? |

There is one global constraint: an **invariant** is a predicate that must hold for every valid
state.

Rules, actions, permissions, validation, decisions, workflows, state machines and constraints
are all built from these primitives. Anything proposed for the core IR that cannot be reduced
to them should be treated with suspicion.

### The formal core

```text
Expression  : Environment → typed value         (environment = S, I, C as in scope)
Predicate   : Expression<Bool>
Transition  : S × I × C → ΔS
Invariant   : S → Bool

Valid transition:
      invariant(S)
    ∧ preconditions(S, I, C)
    ∧ transition(S, I, C) = ΔS
    ∧ S' = apply(S, ΔS)
    ∧ postconditions(S', I, C)
    ∧ invariant(S')
```

Dependency graphs, solver models, traces, permissions, workflows and AI capabilities are
derived from this core. None of them is built in as a special case.

## 1. A system is state

At any moment the system has a typed state `S`. Everything else is expressed in relation to it.

```text
Invoice {
    id:          Id<Invoice>
    amount:      Money
    status:      InvoiceStatus
    approved_by: Option<Id<User>>
}
```

## 2. Behavior is a transition between states

The fundamental primitive is not the rule. It is the transition:

```text
T  : S × I × C → Result<ΔS, O, Trace>
S' = apply(S, ΔS)
```

A transition takes the current **state** (which it may change), an **input** (the call's own
arguments) and a **context** (read-only facts such as the acting user). It returns a proposed
**change set** `ΔS`, outputs and a trace. The new state is the change set applied to the old
one. Because the engine produces `ΔS` before anything changes, every check on the result
happens **before** the change is applied.

**Identity is semantic; parameter names are only bindings.** A change set addresses real state
cells (entity type, id, field), not the parameter names an action happened to use. **Hidden
aliasing is forbidden**: two parameters of one transition may not silently refer to the same
entity; if shared identity is ever needed, it must be explicit in the behavior model.

**Determinism is the most important invariant of the whole system.** Given the same state,
input, context and behavior version, the result must be identical, byte for byte.

## 3. Conditions are typed predicates over explicit semantic inputs

A condition is a typed proposition over the read-only inputs it is given:
`Predicate : S × I × C → Bool`. Many concepts share this one foundation and differ only in
which inputs they see and where they are evaluated:

| Concept | Inputs | Evaluated |
|---------|--------|-----------|
| `requires` | `S`, `I`, `C` | before the transition |
| permission | `S`, `I`, `C` | before the transition |
| `ensures` | `S'`, `I`, `C` | on the proposed result |
| invariant | `S` only | on every state, before and after |

## 4. Effects describe change, not execution

An effect is a declared change, such as `Set(invoice.status, APPROVED)`. It is never "run this
code". The effects of a transition together form the change set `ΔS`, so the engine can
inspect a change before it happens:

```text
evaluate → proposed ΔS → check S' = apply(S, ΔS) → commit
```

The alternative, "execute arbitrary code and hope it was valid", is what this principle rules
out.

## 5. Behavior must be data

`requires(invoice.amount <= actor.limit)` must build an object:

```text
Le(Field(invoice.amount), Field(actor.limit))
```

It must not evaluate a Python boolean. Behavior can be inspected without being executed. That
lets it be hashed, compared, diffed, verified, compiled, visualized, explained and versioned.

Authoring frontends (the Python DSL now, AI proposals and DMN later) only construct behavior.
They never run it.

## 6. Behavior is immutable and content-addressed

A deployed behavior definition never changes; a change creates a new version. Every execution
can say exactly what it ran:

```text
state_version:     82391
behavior_version:  sha256:a74cd…
input:             …
result:            …
```

Identity is a content hash: identical behavior has identical identity. The hashing rules:

- **Identity is separate from storage.** Every semantic node (expression, predicate, effect,
  transition, invariant, module) has a recursive content hash. Together they form a Merkle
  DAG, even when the module is stored as one document. A per-object store can come later as a
  storage change without migrating the IR.
- **Hash the meaning, not the bytes.** Hashes are computed from the canonical semantic form,
  never from a serialization, so moving from JSON to another format changes no identity.
- **Names are for humans; hashes are for machines.** Symbolic names like
  `manager_can_approve` are not part of a rule's hash. The module binds names to hashes, as a
  Git tree binds file names to blobs. Renaming a rule changes the module, not the rule.
- **Metadata is outside the hash.** Source locations, documentation, comments, authors and
  timestamps do not change behavior identity. Fixing a typo in a comment is not a behavior
  change.
- **Hashes are domain-separated.** Each hash is tagged with its node kind and format version
  (`behavior.expr.v1`, `behavior.action.v1`, …). Identical bytes can never mean two different
  kinds of node.

Several properties follow for free: diffs between versions, cached and incremental
verification, incremental compilation, exact rule versions in audit records, deduplication of
identical expressions, and signed behavior packages.

## 7. Composition must preserve meaning

Behaviors defined separately must keep their meaning when combined. That requires pure
expressions, explicit references, explicit effects, no hidden global state, no invisible I/O
and no arbitrary side effects. The same properties are what make static verification possible.

## 8. References are explicit; dependencies are derived

Every reference (to a field, a derived value, a rule) is explicit in the IR. Dependencies,
graphs and analysis structures are derived from those references, never declared by hand:

```text
explicit references → dependency graph → evaluation order, cycle detection, …
```

If `risk` depends on `margin`, which depends on `revenue` and `cost`, the IR shows that through
its references alone.

```text
revenue ──┐
          ├── margin ── risk
cost ─────┘
```

The dependency graph is not the system; it is one view of it. Datalog facts, solver models,
state graphs, execution plans, documentation and AI tool schemas are all derived views of one
canonical semantic model. Keep the primitive model small and derive the useful structures.

## 9. Invalid behavior cannot enter the semantic model

There are two representations. The **wire IR** is whatever arrives at a boundary: JSON from the
Python DSL, from an AI or from a file. It may be malformed, and that is fine; it is untrusted.
The **semantic Behavior IR** is what the engine works with, and invalid semantic IR must never
exist. Untrusted representations are parsed, resolved and type-checked before they become
Behavior IR. `invoice.amount + invoice.status` fails to become Behavior IR at all; a warning
is not enough.

- **Checked at both layers.** The authoring DSL rejects ill-typed expressions where they are
  written, for immediate feedback. The engine still checks every wire IR it receives, in a
  single admission step. Only semantic Behavior IR is ever hashed, verified or evaluated.
- **The engine provides the means to build types; domains provide the types.** The core has
  primitives (`Bool`, `Int`, `Decimal`, `String`), the structural type `Option<T>`, and
  nominal types: enumerations, `Id<E>`, and user-declared types over a primitive. `Money`,
  dates, percentages and similar are declared by the domain (`Money = nominal Decimal`).
  None of them is built into the engine.
- **Operations belong to the type.** A nominal type declares which operations it supports, so
  `Money + Money` can be valid while `Money + Decimal` is rejected.
- **Identity is typed.** `Id<User>`, `Id<Project>` and `String` are distinct types, even when
  all are stored as strings.
- **Numbers are exact.** No floating point decides a rule.

## 10. Verification comes from semantics, not annotations

Nobody writes `@verify_no_conflicts`. The engine knows what expressions, predicates,
transitions and invariants mean, and it derives the verification problems itself:

```text
Rules A and B assign different values to x:  SAT(P_A ∧ P_B)?                            → possible conflict
Transition T and invariant I:  I(S) ∧ pre(S,I,C) ∧ S'=apply(S,T(S,I,C)) ∧ ¬I(S')?  → T can break I
```

No one writes the solver model; it follows from the semantics.

**Identity claims are data; authority requires evidence.** A statement such as "reviewer: anna"
proves nothing. Who stands behind an attestation is established by a signature that verifies
against a key the governing policy trusts, never by a name written in the attestation.

**The verifier may assume exactly what the runtime guarantees — no more, no less.** Whatever the
verifier takes for granted (types, validity of incoming entities, invariants on the starting
state) must be exactly what the runtime checks before it evaluates. Assuming more makes proofs
unsound; assuming less produces counterexamples that could never occur.

## 11. Ordinary transitions preserve validity; they do not establish it

There are two kinds of validity rules. **Entity constraints** say what a valid instance of a
type is (an employee's approval limit is not negative); they apply to every occurrence of the
type, whether it arrives as state, input, or context, and a violation is reported by role
(`INVALID_STATE`, `INVALID_INPUT`, `INVALID_CONTEXT`). **State invariants** are properties of the
system state and apply to S and S' only.

Invariants define the boundary of valid state and must hold both **before and after** every
ordinary transition. A starting state that already breaks an invariant is rejected
(`INVALID_STATE`) before any precondition is evaluated. The verifier may therefore assume
`I(S)` when proving that a transition preserves `I`.

The exception is explicit: repair and migration, if supported, use their own separate
semantics. They never weaken the semantics of ordinary transitions.

## 12. As expressive as necessary, as restricted as possible

Arbitrary code, unbounded loops, free side effects and dynamic behavior make formal analysis
hard or impossible. The core language is limited to:

- typed expressions and boolean logic
- constraints, finite sets and pure functions
- declarative effects and finite state transitions

Verifiability is a basic property of the language, not something added afterwards.

## 13. AI is the interface, never the source of truth

The AI has two roles, and it is the source of truth in neither:

```text
Runtime:      human → AI → structured intent → capability → engine
                    → deterministic result + trace → AI explanation → human

Development:  human → AI → proposed behavior → type check → verification
                    → tests → diff / human review → Git → deployment
```

The AI reaches the system only through declared **capabilities**. It never has direct access
to data or business logic, and every AI output is checked like any other untrusted input.
