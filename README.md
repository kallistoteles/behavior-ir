# deterministic_ai_system

An AI-native information system where behavior is an immutable, typed, content-addressed
description of valid state changes, verified and executed deterministically. See
[PRINCIPLES.md](PRINCIPLES.md) for the first principles and `specs/` for feature specs.

## Development

Requires Nix with flakes. The dev shell provides the pinned Rust toolchain, Python 3.13,
maturin, pytest, and mypy, and creates `.venv/`.

```bash
nix develop
cargo build --workspace
cargo clippy --all-targets -- -D warnings
maturin develop                     # builds behavior._engine into .venv
python -c "import behavior._engine"
```
