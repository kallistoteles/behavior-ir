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
- **Inconclusive** checks (solver budget, or a decimal rounding interval that crosses a
  boundary) are blocking findings too: never reported as passed. Decimal `*` and `/` always
  round, `+` and `-` round for results that need more than 28 digits; the verifier models the
  exact value plus a bound.

```bash
python -m examples.tryout.dump > /tmp/purchase.json
behavior verify /tmp/purchase.json          # exit 1: approve can break within_budget
behavior verify tests/fixtures/verify/purchase_fixed.json   # exit 0: verified
```

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
