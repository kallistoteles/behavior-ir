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

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"})

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

- The Python DSL only **constructs** behavior. Ill-typed expressions (`Money + Decimal`,
  `Id[User] == Id[Project]`) and Python control flow on symbolic values fail at the author's line.
- The engine **admits** wire IR (untrusted JSON) into a typed semantic IR, detects cycles,
  and computes a content hash for every node. The module hash is the behavior version;
  names, comments, and source lines are not part of it.
- Evaluation checks invariants on the current state, preconditions, computes the change set,
  then checks postconditions and invariants on the proposed state. Every call produces a
  canonical decision record that can be replayed byte for byte.
- Structured intents (for an AI interface) name only a capability, target ids, and input; the
  trusted host supplies state and the acting user.

The `behavior` CLI exposes the same engine: `admit`, `version`, `hashes`, `eval`, `intent`,
`replay` (contracts/engine-api.md).

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
```

Layout: `crates/behavior-core` (engine), `crates/behavior-cli` (CLI), `crates/behavior-py`
(Python binding), `python/behavior` (DSL), `tests/fixtures` (shared fixtures; see its README),
`schema/` (wire IR JSON Schema), `examples/`.
