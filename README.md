# deterministic_ai_system

An AI-native information system where behavior is an immutable, typed, content-addressed
description of valid state changes, admitted and executed deterministically by a Rust engine.
AI is the interface; it reaches the system only through declared capabilities.

- [PRINCIPLES.md](PRINCIPLES.md): the first principles every feature is checked against
- [specs/001-verifiable-behavior-ir](specs/001-verifiable-behavior-ir): the v0.1 spec, plan,
  data model, contracts, and quickstart

## What v0.1 does

```python
from decimal import Decimal
from behavior import BehaviorModule, Context, action, entity, field, nominal, requires, set_, evaluate

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)

@entity
class User:
    role = field(str)
    approval_limit = field(Money)

@entity
class Invoice:
    amount = field(Money)
    approved = field(bool)

@action
def approve(invoice: Invoice, *, actor: Context[User]):
    requires(actor.role == "manager")
    requires(invoice.amount <= actor.approval_limit)   # builds a tree; nothing is computed here
    set_(invoice.approved, True)

model = BehaviorModule(entities=[User, Invoice], actions=[approve])
decision = evaluate(
    model, "approve",
    state={"invoice": {"id": "1042", "amount": Decimal("43200"), "approved": False}},
    context={"actor": {"id": "anna", "role": "manager", "approval_limit": 50000}},
    data_version="18342",
)
decision.result      # "ALLOW"; decision.record_json is the replayable decision record
```

- The Python DSL is a **binding to the Rust engine**: every operator calls the engine's builder,
  which type-checks the node as it is built. Ill-typed expressions (`Money + Decimal`,
  `Id[User] == Id[Project]`) and Python control flow on symbolic values fail at the author's line;
  the typing rules exist only once, in Rust.
- The engine **admits** wire IR (untrusted JSON, e.g. from files or other frontends) through
  the same pipeline the builder finishes through, detects cycles, and computes a content hash
  for every node. The module hash is the behavior version; names, comments, and source lines
  are not part of it. Admitted modules serialize to canonical JSON (`model.to_wire_json()`).
- Evaluation checks invariants on the current state, preconditions, computes the change set,
  then checks postconditions and invariants on the proposed state. Every call produces a
  canonical decision record that can be replayed byte for byte.
- Structured intents (for an AI interface) name only a capability, target ids, and input; the
  trusted host supplies state and the acting user.

The `behavior` CLI exposes the same engine: `admit`, `version`, `hashes`, `eval`, `intent`,
`replay` (contracts/engine-api.md).

## Verification (feature 002)

`behavior verify` translates an admitted module to SMT and runs the pinned Z3 (from the dev
shell; `BEHAVIOR_Z3` points at it). No annotations: the checks follow from the semantics.

- **Blocking**: an action can break a state invariant or entity constraint (`preservation`),
  violate its `ensures` (`postcondition`), or divide by zero / overflow (`evaluation_error`).
  Every counterexample is replayed through the evaluator and ships with its decision record.
- **Warnings**: dead actions, redundant preconditions, rules that are always true or false.
- **Inconclusive** checks (solver budget) are blocking findings too: never reported as passed.
  Arithmetic is exact (feature 004), so the verifier and the evaluator share one numeric model.

```bash
python -m examples.tryout.dump > /tmp/purchase.json
behavior verify /tmp/purchase.json          # exit 1: approve can break within_budget
behavior verify tests/fixtures/verify/purchase_money2_remaining.json   # exit 0: verified
```

### Fixed-scale money (feature 003)

Principle: lossless operations may be implicit; lossy conversions must be explicit.

```python
from behavior import Exact, Rounding, nominal, rescale

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)

subtotal = a.amount + b.amount                     # Money: exact, no rounding needed
share = a.amount / 3                               # Exact[Money]: exact rational, not storable
set_(a.fee, rescale(a.amount * Decimal("0.25"), Money, Rounding.HALF_EVEN))  # explicit
requires(a.amount * Decimal("1.25") <= budget.limit)                        # exact comparison
```

Requests with more decimals than declared are rejected (`OFF_GRID`), records show exactly
`scale` digits (`"100.50"`), every rescale appears in the trace with its exact input
(`"40/3"`), and the verifier proves money properties exactly
(`behavior verify tests/fixtures/verify/purchase_money2_remaining.json` exits 0). Modes:
`HALF_EVEN`, `HALF_UP`, `DOWN`, `UP`, `FLOOR`, `CEILING`; there is no default. Details:
`specs/003-fixed-scale-decimals/`.

### Exact arithmetic closure (feature 004)

Numeric computation is exact by default; bounded representation and rounding are explicit.
Every decimal operation is exact: a ratio of two amounts is an exact dimensionless number
(`Exact[Decimal]`), general decimals compute exactly, and values leave the exact domain only
through `rescale` — or implicitly when admission proves, from types and literals alone, that the
stored value is representable (`money := amount * 2` is admitted, `money := amount / 3` is
`LOSSY_CONVERSION`).

```python
portion = a.amount / a.budget                                     # Exact[Decimal]: 1/3
set_(a.part, rescale(portion * a.total, Money, Rounding.HALF_EVEN))  # one rounding
requires(a.amount / a.budget <= Decimal("0.25"))                  # exact comparison
```

Admission also bounds every exact intermediate to the runtime's 512-bit representation
(`EXACT_BOUND_EXCEEDED` otherwise), so whatever the verifier assumes the evaluator can represent.
Wire IR 0.4 (and 0.5 with entity lifecycle) is accepted; records carry `record_version "0.4"`. Details:
`specs/004-exact-arithmetic-closure/`.

### Persistence contract (feature 005)

The engine defines what must be stored for behavior to stay reproducible; the host decides where.
A `Store` evaluates against one consistent snapshot and commits with whole-state optimistic
concurrency, and every transition becomes a hash-chained, replayable record:

```python
store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))
ev = store.evaluate(model, "transfer", bindings={"from_": "a1", "to": "a2"},
                    input={"amount": Decimal("20.00")}, commit_time=now)
store.commit(model, ev.bundle)          # StateConflict if the store moved on: re-evaluate
replay_data(store).ok, replay_behavior(store, [model]).ok
```

State identity is pure content, and history position and entity revisions are kept apart from it.
A host implements ten storage methods, including one atomic compare-and-set, and checks them
with `run_conformance`. See `docs/persistence.md` and `python -m examples.ledger.run`.

### Entity lifecycle (feature 006)

Actions create and remove entities; identities are host-supplied inputs and name one lifetime:

```python
@entity
class Account:
    owner = field(Ref[Customer])   # Id[Customer] + constraint exists(owner)
    balance = field(Money)

@action
def open_account(owner: Customer, *, account_id: Input[Id[Account]], initial: Input[Money]):
    create(Account, id=account_id, owner=owner.id, balance=initial)

@action
def remove_customer(customer: Customer):
    requires(not_(referenced(customer.id)))
    remove(customer)
```

Removal keeps history, a removed identity is never reused (`ENTITY_ID_ALREADY_USED`), and
referential integrity is checked on the resulting state (`DANGLING_REFERENCE`). Evaluation observes
existence, identity and reference *facts*: a store answers them as of the evaluated position,
plain `evaluate(..., facts={...})` takes them from the request, and records keep the observed ones
for replay. Any lifecycle form needs wire IR 0.5. See `docs/persistence.md`,
`python -m examples.accounts.run` and `specs/006-entity-lifecycle/`.

### Relational queries (feature 007)

Behavior can decide over *sets* of entities with typed, read-only set comprehensions:

```python
@derived
def open_order_count(customer: Customer):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    return count(orders.where(lambda o: o.status == OrderStatus.OPEN))

@invariant                      # no parameter: a module invariant over the whole state
def personnel_numbers_unique():
    return unique(select(Employee), by=lambda e: e.personnel_number)

@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(sum_(orders, lambda o: o.amount) + amount <= customer.credit_limit)
    create(Order, id=order_id, customer=customer.id, amount=amount, status=OrderStatus.OPEN, region=customer.region)
```

`select`, `where`, `union`/`intersection`/`difference`, `count`, `any_`, `all_`, `sum_` (exact),
`min_`/`max_` (optional) and `unique` are expressions, never Python collections. Filters are
candidate-local; relational invariants live at module level ("local invariants describe entities;
global invariants describe relations"). A query result is a *fact* about the evaluated state:
stores answer it as of the evaluated position (with an optional field index that never changes a
result), plain `evaluate(..., facts={...})` takes `queries`/`fields` (or whole `universe`
sections) from the request, records keep the observed memberships and member values, and
resulting-state queries are derived from the change, never stored. When a complete `universe`
and redundant query or field facts overlap, they must exactly agree; the universe determines the
query result and contradictions are `INCONSISTENT_FACTS`. Any query form needs wire IR
0.6. See `docs/persistence.md`, `docs/verification.md`, `python -m examples.orders.run` and
`specs/007-relational-queries/`.

The output is a canonical, content-addressed **verification attestation** bound to the
behavior version. Results are cached by check key (`--cache .behavior/verify-cache`), so
unchanged behavior is not re-verified.

Verification never changes whether a transition may be committed on its own: an **execution
policy** (a content-addressed document) decides, producing a **commit authorization**.
**Waivers** for blocking findings (by default only `inconclusive` ones) are governance evidence
bound to the behavior version and finding hash; they count only with an Ed25519 signature by a
key the policy trusts ("identity claims are data; authority requires evidence").

```bash
behavior authorize <wire> <record> --policy policy.json --attestation attestation.json \
    [--waiver waiver.json --signature signed.json]... --now 2026-09-25T12:00:00Z
behavior waiver-hash waiver.json
behavior sign-waiver waiver.json --seed key.hex
```

The same is available from Rust (`behavior_verify::{verify, governance::authorize}`) and Python
(`verify(model)`, `authorize(model, decision, policy=..., ...)`). Details:
`specs/002-smt-verification/` (quickstart, contracts); overview, guarantees, and known
limitations: `docs/verification.md`.

## Development

Requires Nix with flakes. The dev shell provides the pinned Rust toolchain, Python 3.13,
maturin, pytest, mypy, and jsonschema, and creates `.venv/`.

```bash
nix develop
cargo build --workspace
maturin develop                     # builds behavior._engine into .venv
```

Quality gates (all must pass before merging; see the constitution):

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
pytest python/tests
mypy
scripts/determinism-check.sh
cargo test --release -p behavior-core -- --ignored perf    # SC-005 performance check
cargo test --release -p behavior-verify --test perf -- --ignored   # SC-003, SC-004
```

Layout: `crates/behavior-core` (engine), `crates/behavior-verify` (SMT verification and
governance), `crates/behavior-cli` (CLI), `crates/behavior-py`
(Python binding), `python/behavior` (DSL), `tests/fixtures` (shared fixtures; see its README),
`schema/` (wire IR JSON Schema), `examples/`.
