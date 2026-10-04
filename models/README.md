# Behavior Models

A **Behavior Model** is a higher-level modeling abstraction, such as a state machine, a workflow,
an approval process or a saga. It lowers completely to ordinary Behavior IR.

**Behavior IR is the semantic assembly language; models are languages above it.** The core
admits the lowered IR like any other module and never learns which model produced it. See the
core's [ARCHITECTURE.md](https://github.com/kallistoteles/behavior-ir-core/blob/v0.10.2/ARCHITECTURE.md)
for the layers and the dependency direction.

No model exists yet. This area holds the rules every model follows, and one worked example.

## Rules

1. **Complete, deterministic lowering (FR-009).**
   - A model lowers to ordinary Behavior IR, and the same model always gives the same IR, byte
     for byte.
   - It may restrict, compose or generate core semantics. It never introduces runtime semantics
     the core does not know: there is no model-specific evaluation, hook or side channel.
2. **One authoritative lowering (FR-010).**
   - Each model has exactly one definition of its normalization, validation and lowering.
   - Every binding's syntax for the model calls that definition; no binding reimplements it.
3. **What cannot be lowered faithfully (FR-011).** A construct that Behavior IR cannot represent
   faithfully is never implemented as hidden model behavior. Either:
   - it is redesigned on existing core semantics; or
   - the missing concept is proposed as a core primitive in a core feature (numbered 012+ in
     behavior-ir-core), and the model waits for the Core Release that adds it.
4. **Model identity (FR-012).**
   - A model may have its own canonical form and content hash. An audit can then keep both what
     was authored (the model's identity) and what was executed (the Behavior module hash).
   - The module hash is always authoritative for runtime semantics.
5. **Where a concept belongs.** Ask: *must the evaluator understand this construct for its
   semantics to be correct?*
   - If not, it is a model, a library or tooling here.
   - If so, it is a candidate core primitive.

## Worked example: a state machine

[`examples/state_machine/lower.py`](examples/state_machine/lower.py) lowers a transition
`submit: DRAFT -> SUBMITTED` over `Order.status` to one action:

```python
requires(order.status == OrderStatus.DRAFT)
set_(order.status, OrderStatus.SUBMITTED)
ensures(order.status == OrderStatus.SUBMITTED)
```

Its tests (`examples/state_machine/test_state_machine.py`) check that:
- the lowered IR holds exactly these three statements;
- the core admits the module;
- nothing in the IR names a state machine;
- lowering twice gives byte-identical IR.
