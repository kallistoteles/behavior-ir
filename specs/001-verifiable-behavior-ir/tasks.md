---

description: "Task list for Verifiable Behavior IR Core (v0.1)"
---

# Tasks: Verifiable Behavior IR Core (v0.1)

**Input**: Design documents from `/specs/001-verifiable-behavior-ir/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (ir-encoding.md,
hashing.md, python-api.md, engine-api.md), quickstart.md; also `PRINCIPLES.md` and
`.specify/memory/constitution.md` at the repository root.

**Tests**: Included and written first. The constitution makes test-first non-negotiable
(principle III): every test task below must be written, run, and seen to fail before the
implementation tasks that follow it in the same phase.

**Organization**: Tasks are grouped by user story. The Rust admission pipeline (wire IR →
semantic IR with hashes) is foundational because every story depends on it; the Python DSL is
User Story 1.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1, US2, US3 (from spec.md)
- Paths are relative to the repository root (layout in plan.md → Project Structure)

## Conventions for every task

- Rust: edition 2024, `#![forbid(unsafe_code)]` in `behavior-core` and `behavior-cli`, typed
  errors with `thiserror`, no `unwrap`/`expect` outside tests unless an invariant is documented
  at the call site, only ordered collections (`BTreeMap`, `Vec`) where order can reach output.
- Output JSON is always produced through `canonical.rs` (sorted keys, compact, no floats).
- Decimals are always encoded as normalized plain strings (`"50000"`, `"0.05"`, never exponent
  notation or trailing zeros, `"-0"` → `"0"`).
- Error codes, result names, and JSON keys must match data-model.md and contracts/ exactly.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Toolchain, workspace, and package skeletons

- [X] T001 Create `flake.nix` providing a dev shell with the Rust toolchain read from `rust-toolchain.toml` (via `rust-overlay` or `fenix`), `python313`, `maturin`, `python313Packages.pytest`, `python313Packages.mypy`, and a venv hook so `maturin develop` installs into `.venv`; add `flake.lock` by running `nix flake lock`
- [X] T002 Create `rust-toolchain.toml` pinning a specific stable Rust version with components `rustfmt` and `clippy`, and the workspace `Cargo.toml` with members `crates/behavior-core`, `crates/behavior-cli`, `crates/behavior-py`, `edition = "2024"`, `resolver = "3"`, and `[workspace.lints]` denying `clippy::unwrap_used` and `clippy::expect_used` (allowed in tests via `cfg_attr`); `unsafe_code` is **not** a workspace lint because PyO3's macros generate unsafe code (see T005)
- [X] T003 Create `crates/behavior-core/Cargo.toml` (deps: `serde` with `derive`, `serde_json` without `preserve_order`, `rust_decimal`, `sha2`, `thiserror`; dev-deps: `proptest`, `pretty_assertions`) and `crates/behavior-core/src/lib.rs` with `#![forbid(unsafe_code)]` and empty modules `wire`, `semantic`, `admit`, `decimal`, `canonical`, `eval`, `record`, `intent`, `pretty`
- [X] T004 [P] Create `crates/behavior-cli/Cargo.toml` (deps: `behavior-core`, `clap` with `derive`) and `crates/behavior-cli/src/main.rs` with `#![forbid(unsafe_code)]`, a `clap` skeleton for subcommands `admit`, `version`, `hashes`, `eval`, `intent`, `replay` that exit with code `64` and a "not implemented" message
- [X] T005 [P] Create `crates/behavior-py/Cargo.toml` (`crate-type = ["cdylib"]`, deps: `behavior-core`, `pyo3` with `extension-module`) and `crates/behavior-py/src/lib.rs` defining the `#[pymodule]` `_engine` with no functions yet; no `#![forbid(unsafe_code)]` in this crate because PyO3's macros generate unsafe code, but hand-written `unsafe` is not allowed (checked in the review task T075)
- [X] T006 [P] Create `pyproject.toml` for maturin (`[tool.maturin] python-source = "python"`, `module-name = "behavior._engine"`, `manifest-path = "crates/behavior-py/Cargo.toml"`), `requires-python = ">=3.13"`, pytest config (`testpaths = ["python/tests"]`), and mypy config (`strict = true`, `files = ["python/behavior"]`)
- [X] T007 [P] Create `python/behavior/__init__.py` (empty `__all__`), `python/behavior/errors.py` defining `BehaviorError`, `BehaviorTypeError`, `BehaviorDefinitionError`, `BehaviorInvalid`, `IntentRejected` (each storing `message`, `file`, `line` where applicable), and `python/tests/conftest.py`
- [X] T008 [P] Create `rustfmt.toml` (`max_width = 100`), `.gitignore` (`target/`, `.venv/`, `__pycache__/`, `*.so`, `result`), and `tests/fixtures/README.md` documenting each fixture directory from plan.md and that fixtures are shared by Rust, CLI, and Python tests
- [X] T009 Verify the skeleton: inside `nix develop`, `cargo build --workspace`, `cargo clippy --all-targets -- -D warnings`, `maturin develop`, and `python -c "import behavior._engine"` all succeed; commit `Cargo.lock`; record the exact commands in `README.md` under "Development"
- [X] T010 [P] Create `scripts/determinism-check.sh` (executable, `set -euo pipefail`): run `behavior admit` twice on every file in `tests/fixtures/wire/valid/` and compare the outputs byte for byte; exit non-zero on any difference or if the `behavior` binary is missing; later phases add their cases to this same script

**Checkpoint**: empty workspace builds; Python can import the extension

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Everything every story needs: exact decimals, canonical output, wire IR decoding,
the semantic IR, and the admission pipeline with content hashing (research R2–R5, R8, R9;
contracts/ir-encoding.md, contracts/hashing.md; data-model.md)

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests for Foundational (write first, must fail)

- [X] T011 [P] Write decimal tests in `crates/behavior-core/tests/decimal.rs`: parsing from strings and JSON integers; normalization (`"50000.00"`→`"50000"`, `"0.050"`→`"0.05"`, `"-0"`→`"0"`, `"1E+3"` rejected); rejection of JSON fractional numbers and exponent forms; equality of `50000` and `50000.00`; division rounding half-to-even at 28 significant digits; overflow and division by zero return typed errors
- [X] T012 [P] Write canonical JSON tests in `crates/behavior-core/tests/canonical.rs`: keys sorted by byte value at every depth, no whitespace, no trailing newline, UTF-8 without escaping non-ASCII, and a round trip of the same value yields identical bytes
- [X] T013 [P] Create the shared typing table `tests/fixtures/typing_cases.json`: a list of `{"op", "operands": [<wire type>…], "nominals": [...], "expect": <wire type> | {"error": "<code>"}}` covering every row of data-model.md → Typing rules, including: Int+Int→Int; Int+Decimal→Decimal; Money+Money→Money with `add`; Money+Decimal→`TYPE_MISMATCH`; Money<Money→Bool with `order` and `OP_NOT_ALLOWED` without it; Money*Int→Money with `scale`; Quantity (nominal over Int) * Decimal→`TYPE_MISMATCH`; Money/Money→Decimal with `ratio`; Money/Decimal→Money with `scale`; Id(User)==Id(Project)→`TYPE_MISMATCH`; Id(User)==String→`TYPE_MISMATCH`; Option(Id(User))==Id(User)→Bool (implicit `some`); Option(T) where T required→`TYPE_MISMATCH`; Option(Option(T))→`NESTED_OPTION`; `in` with empty list→`EMPTY_IN`; `value_or`; `wrap`/`unwrap`; Bool-only `and`/`or`/`not`
- [X] T014 [P] Write `crates/behavior-core/tests/typing_cases.rs` that loads `tests/fixtures/typing_cases.json` and asserts the engine's typing function returns the expected type or error code for every case
- [X] T015 [P] Create hand-written valid wire IR fixtures `tests/fixtures/wire/valid/invoice.json` (Money nominal with ops `add, order, ratio, scale`; enum InvoiceStatus `pending, approved`; entities User{role: string, approval_limit: Money} and Invoice{amount: Money, status: InvoiceStatus, approved_by: option of id User}; invariant `non_negative_amount`: `invoice.amount >= Money(0)`; action `approve_invoice(invoice: state Invoice, actor: context User)` with the three preconditions, effects on `status` and `approved_by`, postcondition `approved_by == actor.id`; action `apply_discount(invoice: state Invoice, discount: input Money)` with effect `amount := amount - discount`) `tests/fixtures/wire/valid/empty.json` (every list empty), and `tests/fixtures/wire/valid/project_margin.json` (Project{revenue: Money, cost: Money, flagged: bool}; derived `margin` = `(revenue - cost) / revenue`; rule `high_risk` = `margin(project) < 0.05`; action `flag_project(project: state Project)` with precondition `(project.revenue != Money(0)) and high_risk(project)` and effect `flagged := true`, plus an action `flag_project_unguarded` with precondition `high_risk(project)` only), exactly per contracts/ir-encoding.md
- [X] T016 [P] Create invalid wire IR fixtures in `tests/fixtures/wire/invalid/`, one `<case>.json` plus `<case>.expected.json` (`{"errors": [{"code", "loc"}…]}` in sorted order) per case: `unknown_key` (`DECODE_ERROR`), `ir_version_0_2` (`UNSUPPORTED_IR_VERSION`), `unknown_entity`, `unknown_type`, `unknown_field`, `unknown_derived`, `unknown_param`, `duplicate_entity_name` (`DUPLICATE_NAME`), `field_named_id` (`RESERVED_NAME`), `money_plus_decimal` (`TYPE_MISMATCH`), `id_user_vs_id_project` (`TYPE_MISMATCH`), `option_used_as_value` (`TYPE_MISMATCH`), `derived_arity` (`ARITY_MISMATCH`), `precondition_not_bool` (`NOT_BOOLEAN`), `order_without_op` (`OP_NOT_ALLOWED`), `cycle_a_b_c` (`CYCLE` with message `derived values form a cycle: a → b → c → a` and `related_locs` for all three), `duplicate_assignment` (`DUPLICATE_ASSIGNMENT`), `effect_on_context` (`EFFECT_ON_READONLY`), `effect_on_id` (`RESERVED_NAME`), `empty_in` (`EMPTY_IN`), `float_literal` (`INVALID_LITERAL`), `nested_option` (`NESTED_OPTION`), and `multiple_errors` (three independent errors in different files/lines, checking sort order by file, line, code)
- [X] T017 Write `crates/behavior-core/tests/admission.rs`: every `tests/fixtures/wire/valid/*.json` admits with `ok: true`, a `behavior_version`, for `project_margin.json` `evaluation_order == ["margin", "high_risk"]`; `empty.json` admits with empty `items` and empty `evaluation_order`; every invalid fixture returns `ok: false`, no `behavior_version`, and exactly the expected errors in order; admitting the same file twice yields byte-identical AdmissionResult JSON
- [X] T018 [P] Write `crates/behavior-core/tests/hash_properties.rs` using `proptest` and hand-edited variants of `tests/fixtures/wire/valid/*.json`: behavior version is **unchanged** by changing any `loc`, by reordering the `enums`/`nominals`/`entities`/`derived`/`invariants`/`actions` lists, and by writing an implicit `to_decimal`/`some` explicitly; the item hash of `margin` is unchanged by renaming it (module version changes); two structurally identical expressions in different rules have equal hashes; the version **changes** when two preconditions are swapped, a literal changes, a field is renamed, or a nominal type's `ops` change; no hash is computed for `cycle_a_b_c.json`

### Implementation for Foundational

- [X] T019 [P] Implement `crates/behavior-core/src/decimal.rs`: a `Dec` newtype over `rust_decimal::Decimal` with `parse_str`, `from_json` (string or integer only), `to_normalized_string`, checked `add/sub/mul/div` returning `Result<Dec, NumError>` (overflow, division by zero), half-to-even rounding at 28 significant digits; make T011 pass
- [X] T020 [P] Implement `crates/behavior-core/src/canonical.rs`: `to_canonical_string(&serde_json::Value) -> String` and a helper that serializes any `Serialize` via `serde_json::Value`; reject `f64` numbers with a typed error; make T012 pass
- [X] T021 Implement `crates/behavior-core/src/wire.rs`: serde structs for the wire document per contracts/ir-encoding.md with `#[serde(deny_unknown_fields)]` everywhere, `ir_version`, `enums`, `nominals`, `entities`, `derived`, `invariants`, `actions`, wire types (`t` tag), expression nodes (`op` tag, every node with `loc`), `SourceLoc {file, line}`, and parameter `role` (`state`, `input`, `context`; absent for derived/invariant parameters); decoding failures map to `DECODE_ERROR` with the JSON path in the message
- [X] T022 Implement `crates/behavior-core/src/semantic/types.rs`: `Type` (`Bool`, `Int`, `Decimal`, `String`, `Option(Box<Type>)`, `Enum(EnumRef)`, `Id(String)`, `Nominal(NominalRef)`), `NominalDecl {name, underlying, ops}` with `ops` as a bitmask (`order=1, add=2, scale=4, ratio=8`) where "`order`/`add`/`scale`/`ratio` require a numeric underlying type", and `fn type_of_op(op, &[Type]) -> Result<(Type, Vec<Conversion>), TypeErrorCode>` implementing data-model.md → Typing rules and the implicit conversions (`Int`→`Decimal` as `to_decimal`, `T`→`Option(T)` as `some`, `none` taking the required option type); make T014 pass
- [X] T023 Implement `crates/behavior-core/src/semantic/expr.rs` and `crates/behavior-core/src/semantic/module.rs`: semantic node types with **private fields** and `pub(crate)` constructors callable only from `admit` (every expression carries `ty`, `hash: [u8; 32]`, and `loc` outside the hash), items `EnumItem`, `NominalItem`, `EntityItem` (implicit `id: Id(self)` field first), `DerivedItem {kind, params, body, result_type}`, `InvariantItem {entity, param, body}`, `ActionItem {params, preconditions, effects, postconditions}` where each param has `role` and "≥ 1 state parameter", `Module {items, name_table: BTreeMap<(Kind, String), Hash>, evaluation_order, hash}`, plus read-only accessors
- [X] T024 Implement `crates/behavior-core/src/admit/resolve.rs`: name resolution for entities, enums, nominal types, parameters, fields, and derived references with errors `UNKNOWN_ENTITY`, `UNKNOWN_TYPE`, `UNKNOWN_FIELD`, `UNKNOWN_DERIVED`, `UNKNOWN_PARAM`, `DUPLICATE_NAME` ("names unique across enums, nominal types and entities"; "names unique across derived, invariants and actions"; parameter and field names unique within their declaration), `RESERVED_NAME` for a declared field named `id` or an effect targeting `id`
- [X] T025 Implement `crates/behavior-core/src/admit/graph.rs`: build the reference graph over derived values by name, detect cycles (report each cycle once, starting from the derived value that comes first by name, message `derived values form a cycle: a → b → c → a`, `related_locs` = all members), and compute `evaluation_order` as a topological order with ties broken by name
- [X] T026 Implement `crates/behavior-core/src/admit/typecheck.rs`: type-check derived values in `evaluation_order`, then invariants and actions, using `type_of_op`; enforce `NOT_BOOLEAN` for rule bodies, invariants, preconditions, postconditions; `ARITY_MISMATCH` for derived references; `EFFECT_ON_READONLY` for effects on input/context parameters; `DUPLICATE_ASSIGNMENT` ("target fields unique within the action"); `INVALID_LITERAL` for floats, non-normalizable decimals, out-of-range integers, or enum values not declared; `NESTED_OPTION`; `EMPTY_IN`; insert explicit conversion nodes for every implicit conversion
- [X] T027 Implement `crates/behavior-core/src/admit/hash.rs` exactly per contracts/hashing.md: tags `behavior.{enum,nominal,entity,expr,effect,derived,invariant,action,module}.v1`, `SHA-256(tag ‖ 0x00 ‖ body)`, primitive encodings (`u8`, `u32` BE, `i64` BE, length-prefixed `str`, counted `list`, 32-byte `ref`, normalized-string `decimal`), inline type encodings, opcode table, literal encodings, and the module entry list sorted by `(kind, name bytes)`; names of derived values, invariants, and actions appear only in module entries; source locations never enter the hash
- [X] T028 Implement `crates/behavior-core/src/admit/mod.rs` and the public API in `crates/behavior-core/src/lib.rs`: `admit(wire: &str) -> Result<Module, AdmissionResult>` and `admission_report(wire: &str) -> AdmissionResult` running decode → resolve → graph/cycles → typecheck → hash; collect errors from all stages that can run, stop before hashing if any error exists (a cycle blocks hashing), sort errors by (file, line, code); `AdmissionResult {ok, behavior_version?, errors, evaluation_order, items}` serialized via `canonical.rs` with `items` keyed `"<kind>:<name>"`; make T017 and T018 pass
- [X] T029 Generate `tests/fixtures/hash_vectors.json` (a list of small wire snippets: one per expression opcode, one per declaration kind, one full module, each with its expected `sha256:` hash) from the implementation, review each entry by hand against contracts/hashing.md, commit it as frozen, and write `crates/behavior-core/tests/hash_vectors.rs` asserting every vector; add a comment in the fixture stating that any change requires new hash tags; run `scripts/determinism-check.sh`

**Checkpoint**: `cargo test -p behavior-core` passes; hand-written wire IR is admitted into semantic IR with stable hashes, or rejected with exact errors

---

## Phase 3: User Story 1 - Author behavior in Python and get it validated (Priority: P1) 🎯 MVP

**Goal**: Authors write entities, nominal types, derived values, rules, invariants, and actions
in the Python DSL; ill-typed expressions fail at the author's line; the module compiles to wire
IR and is admitted by the engine with a behavior version, evaluation order, and item hashes.

**Independent Test**: write the invoice and margin examples in Python, admit them, and confirm
`ok`, the evaluation order, and stable versions; introduce each construction-time error from
quickstart.md §3 and a cycle, and confirm each is reported with file and line (quickstart §3–§6).

### Tests for User Story 1 (write first, must fail)

- [X] T030 [P] [US1] Write `python/tests/test_types.py` that loads `tests/fixtures/typing_cases.json` and asserts the DSL's typing function (`behavior.types.type_of_op`) agrees with every expected result or error code (errors surface as `BehaviorTypeError` carrying the code)
- [X] T031 [P] [US1] Create DSL fixtures `python/tests/fixtures/invoice_model.py` (the model from contracts/python-api.md plus `apply_discount(invoice: Invoice, *, discount: Input[Money])` with `set_(invoice.amount, invoice.amount - discount)`), `python/tests/fixtures/margin_model.py` (the same model as `tests/fixtures/wire/valid/project_margin.json`: Project with `flagged`, `margin`, `high_risk`, `flag_project`, `flag_project_unguarded`), and `python/tests/fixtures/cycles.py` (`a` uses `b`, `b` uses `c`, `c` uses `a`, all derived over one entity)
- [X] T032 [P] [US1] Write `python/tests/test_dsl.py` asserting, with the exact line number of the offending statement: `BehaviorTypeError` for `invoice.amount + invoice.status`, `invoice.amount + Decimal("10")`, `invoice.approved_by == project.id`, `invoice.approved_by <= actor.id`; `BehaviorDefinitionError` for `if invoice.amount > Money(Decimal("100")):`, `requires(a < b < c)`, `not expr`, `set_(actor.role, "x")` on a context parameter, `set_(invoice.id, …)`, `field(float)`, a field named `id`, `requires(...)` outside an `@action`, `@invariant` with two parameters; and that expressions build the expected node tree (e.g. `invoice.amount <= actor.approval_limit` is `le(field(invoice, amount), field(actor, approval_limit))`)
- [X] T033 [P] [US1] Write `python/tests/test_wire.py`: `BehaviorModule(...).to_wire_json()` for `invoice_model.py` and `margin_model.py` equals the golden files `tests/fixtures/wire/python/invoice.json` and `tests/fixtures/wire/python/project_margin.json` byte for byte (goldens created in T046); source `loc.file` values are relative to the module root; emitting twice gives identical bytes
- [X] T034 [P] [US1] Write `python/tests/test_admit.py`: `admit(model)` for both fixtures returns `ok` with `evaluation_order == ["margin", "high_risk"]` for the margin model; `cycles.py` returns `ok: false` with one `CYCLE` error naming `a → b → c → a` and three related locations, and no `behavior_version`; the identity checks of quickstart.md §5 against `tests/fixtures/versions.json` (comment/line change → same version; reordered entities → same version; renamed `margin` → same item hash, different version; swapped preconditions → different version)
- [X] T035 [P] [US1] Write `crates/behavior-cli/tests/cli_admit.rs` (using `std::process::Command` on the built binary): `behavior admit` exits `0` on `tests/fixtures/wire/valid/invoice.json` and `2` on each invalid fixture with the expected JSON; `behavior version` prints the same version as `admit`; `behavior hashes` prints the `items` table

### Implementation for User Story 1

- [X] T036 [P] [US1] Implement `python/behavior/types.py`: type representations mirroring data-model.md (`Bool`, `Int`, `Decimal`, `String`, `Option[T]` with "inner type is not an Option", enum types from `Enum` subclasses with `str` values, `Id[E]`, nominal types), `nominal(name, underlying, ops)` validating that ops ⊆ {`order`, `add`, `scale`, `ratio`} and that these require a numeric underlying type, `Context[T]`, `Input[T]`, the `none` literal, and `type_of_op` implementing the same typing table as the engine; make T030 pass
- [X] T037 [US1] Implement `python/behavior/expr.py`: typed immutable expression nodes (`Lit`, `FieldRef`, `ParamRef`, `DerivedRef`, comparison, arithmetic, `And`, `Or`, `Not`, `In`, `IsNone`, `IsSome`, `ValueOr`, `Some`, `ToDecimal`, `Wrap`, `Unwrap`), each recording `type` and `loc` (first stack frame outside the `behavior` package); operator overloads (`== != < <= > >= + - * / & | ~`) that call `type_of_op` and raise `BehaviorTypeError` at the caller's line; `.in_()`, `.is_none()`, `.is_some()`, `.value_or()`; Python literals (`int`, `Decimal`, `str`, `bool`, `None`, `Enum` members) converted to literals; `float` rejected; `__bool__` raising `BehaviorDefinitionError` explaining that behavior cannot drive Python control flow; `__hash__ = None`; `and_()`, `or_()`, `not_()`, `underlying()`
- [X] T038 [US1] Implement `python/behavior/decl.py`: `field(T)` (rejects `float`), `@entity` (fields in declaration order, implicit `id: Id[Self]` first, `id` reserved), `@derived`, `@rule`, `@invariant` (exactly one entity parameter), `@action` (bare entity annotation = state parameter; keyword-only `Context[T]`/`Input[T]` parameters) — all registering functions **without running them**; calling a registered derived/rule inside a traced body returns a `DerivedRef` by name
- [X] T039 [US1] Implement `python/behavior/statements.py`: a context variable holding the body currently being traced; `requires(e)` and `ensures(e)` (only inside `@action`, `e` must be Bool), `set_(target, value)` (target must be a `FieldRef` of a state parameter and not `id`; value type checked against the field type, allowing `T`→`Option[T]`); misuse raises `BehaviorDefinitionError` with file and line
- [X] T040 [US1] Implement `python/behavior/module.py`: `BehaviorModule(entities, derived, invariants, actions, enums=(), nominals=(), root=None)` that collects enums and nominal types used by fields, traces all bodies at compile time in dependency order (a reference to a derived value on a cycle gets a placeholder type so the cycle reaches the engine), computes `loc.file` relative to `root` (default: common directory of the definitions) with `/` separators, and emits canonical wire IR via `to_wire_json()` per contracts/ir-encoding.md (`json.dumps(sort_keys=True, separators=(",", ":"), ensure_ascii=False)`, decimals via `format(d.normalize(), "f")` with a trailing `.` stripped); make T032 and T033 pass once goldens exist
- [X] T041 [US1] Expose `admit(wire: str) -> str` in `crates/behavior-py/src/lib.rs` calling `behavior_core::admission_report` (no logic in the binding)
- [X] T042 [US1] Implement `python/behavior/results.py` (`AdmissionResult`, `AdmissionError` frozen dataclasses built from engine JSON) and `admit(model)` in `python/behavior/__init__.py` calling `behavior._engine.admit(model.to_wire_json())`; export the declaration API listed in contracts/python-api.md from `python/behavior/__init__.py`; make T034 pass
- [X] T043 [US1] Implement `admit`, `version`, and `hashes` subcommands in `crates/behavior-cli/src/main.rs` with exit codes `0` admitted, `2` admission failure, `64` usage error, output via `canonical.rs`; make T035 pass
- [X] T044 [P] [US1] Create `examples/invoice/behavior.py` (same model as `python/tests/fixtures/invoice_model.py`, exposing `model`) and `examples/invoice/dump.py` (prints `model.to_wire_json()` without a trailing newline)
- [X] T045 [P] [US1] Create `examples/project_margin/behavior.py` (same model as `python/tests/fixtures/margin_model.py`; `flag_project` precondition written as `(project.revenue != Money(Decimal("0"))) & high_risk(project)`) and `examples/project_margin/run.py` that admits the model and prints `evaluation_order`
- [X] T046 [US1] Generate the golden files `tests/fixtures/wire/python/invoice.json` and `tests/fixtures/wire/python/project_margin.json` from the DSL fixtures, review them by hand against contracts/ir-encoding.md, and generate `tests/fixtures/versions.json` (behavior versions and item hashes for the base models and for each identity variant used in T034); commit them as goldens
- [X] T047 [US1] Run the US1 validation: `pytest python/tests/test_types.py python/tests/test_dsl.py python/tests/test_wire.py python/tests/test_admit.py`, `cargo test -p behavior-cli`, `scripts/determinism-check.sh`, and quickstart.md §3–§6; fix any failures

**Checkpoint**: User Story 1 is fully functional: authoring, construction-time errors, admission,
identity (MVP)

---

## Phase 4: User Story 2 - Execute actions deterministically with a decision record (Priority: P2)

**Goal**: The engine evaluates a transition over state, input, and context: invariants on S,
preconditions, ΔS, postconditions and invariants on S'; returns a canonical decision record
citing hashes; replays records (research R6, R7, R14; contracts/engine-api.md).

**Independent Test**: evaluate `approve_invoice` and `apply_discount` for every branch using the
request fixtures and compare against golden records; re-run and replay them (quickstart §2, §8).
Can be tested with the hand-written wire fixtures and the CLI, without the Python DSL.

### Tests for User Story 2 (write first, must fail)

- [X] T048 [P] [US2] Create request fixtures in `tests/fixtures/requests/` against `tests/fixtures/wire/valid/invoice.json` per the EvaluationRequest format: `invoice_allowed.json` (amount `"43200"`, pending, `approved_by: null`, actor manager with limit `"50000"`), `invoice_above_limit.json` (amount `"60000"`), `invoice_wrong_role.json` (role `"clerk"`), `invoice_exact_boundary.json` (amount `"50000.00"`), `invoice_invalid_state.json` (amount `"-1"`), `discount_breaks_invariant.json` (`apply_discount` with discount larger than amount), `invalid_missing_field.json`, `invalid_extra_field.json`, `invalid_fractional_number.json` (amount `43200.5` as a JSON number), `invalid_wrong_section.json` (actor passed under `state`); and against `project_margin.json`: `margin_guarded_zero_revenue.json` (`flag_project`, revenue `"0"`) and `margin_unguarded_zero_revenue.json` (`flag_project_unguarded`, revenue `"0"`)
- [X] T049 [P] [US2] Create the expected golden records in `tests/fixtures/records/` (one per request, same base name) by hand from contracts/engine-api.md → DecisionRecord: results `ALLOW`, `DENY` (failed precondition, later ones `"skipped"`), `ALLOW` at the exact boundary, `INVALID_STATE` (only `invariant_pre` in the trace, no precondition), `DENY` from `invariant_post`, `INVALID_INPUT` for the four invalid requests, `DENY` for the guarded margin case (short-circuit, no division), `ERROR` naming the division for the unguarded case; `changes` empty unless `ALLOW`; hashes taken from `behavior hashes tests/fixtures/wire/valid/invoice.json` and `behavior hashes tests/fixtures/wire/valid/project_margin.json` (available after Foundational)
- [X] T050 [P] [US2] Write `crates/behavior-core/tests/evaluate.rs`: for each request fixture, admit the wire file, evaluate, and assert the canonical record equals the golden record byte for byte; evaluating twice gives identical bytes; a `proptest` over random amounts and limits checks that the result is `ALLOW` exactly when `amount <= limit` (exact decimal comparison) and that `changes` is empty otherwise
- [X] T051 [P] [US2] Write `crates/behavior-core/tests/replay.rs`: replaying every golden record against its wire file returns `{"matches": true}`; replaying against a wire file with one precondition changed returns `matches: false` with a `diff` naming `behavior_version`; a record with one altered trace value returns `matches: false` with the differing JSON path
- [X] T052 [P] [US2] Write `crates/behavior-cli/tests/cli_eval.rs`: `behavior eval` exit codes (`0` ALLOW, `1` DENY, `2` INVALID_INPUT/INVALID_STATE, `3` ERROR) and stdout equal to the golden records; `behavior replay` exits `0` on match and `2` on mismatch
- [X] T053 [P] [US2] Write `python/tests/test_evaluate.py`: `evaluate(model, …)` with the invoice DSL fixture reproduces each quickstart.md §2 row (including `TypeError` for a `float` amount before reaching the engine); `decision.record_json` is byte-identical across two calls; `replay(model, decision.record_json).matches` is `True`; `evaluate` on a module that fails admission raises `BehaviorInvalid`

### Implementation for User Story 2

- [X] T054 [P] [US2] Implement `crates/behavior-core/src/pretty.rs`: `expr_text` for semantic expressions showing explicit conversions (`some(actor.id)`, `Money(0)`), enum literals as `InvoiceStatus.pending`, string literals quoted, effects as `param.field := value`, and minimal parentheses by precedence
- [X] T055 [US2] Implement request decoding and input checking in `crates/behavior-core/src/eval.rs`: a `Value` enum for every semantic type; decode `state`, `input`, `context` sections per data-model.md → Evaluation request (Bool → bool; Int → integer; Decimal → normalized string or integer; String → string; Enum → value string; Id → string; Nominal → encoding of its underlying type; Option → `null` for none; entity → object of fields including `id`); every declared parameter present in its declared section, every field present with the right type, no extra parameters or fields, fractional JSON numbers rejected → `INVALID_INPUT` with all problems listed as reasons
- [X] T056 [US2] Implement transition evaluation in `crates/behavior-core/src/eval.rs` in the order of research R7: invariants on S for every state parameter whose entity declares invariants (false → `INVALID_STATE`, preconditions not evaluated), preconditions in order (first false → `DENY`, remaining `"skipped"`), effects evaluated against S into ΔS `{param, field, old, new}`, `S' = apply(S, ΔS)`, postconditions then invariants on S' (false → `DENY` with empty changes); any evaluation error (division by zero, overflow) → `ERROR` naming the expression; `and`/`or` short-circuit left to right; derived values memoized per phase and recorded with `phase_state` `"S"` or `"S'"`; every trace step records phase, name (for invariants), predicate or effect hash, `expr_text`, `reads` (every field, parameter, or derived value read), outcome, and `loc`
- [X] T057 [US2] Implement `crates/behavior-core/src/record.rs`: `DecisionRecord` with `record_version: "0.1"`, `behavior_version`, optional `git_revision`, `data_version`, `action {name, hash}`, normalized `state`/`input`/`context`, `result`, `reasons`, `trace`, `derived`, `changes`, serialized via `canonical.rs` with no timestamps or host data; `replay(module, record)` re-evaluating the stored request and comparing canonical bytes, returning `ReplayResult {matches, diff?}` (behavior version mismatch reported first); expose `evaluate` and `replay` in `crates/behavior-core/src/lib.rs`; make T050 and T051 pass
- [X] T058 [US2] Expose `evaluate(wire, request) -> str` and `replay(wire, record) -> str` in `crates/behavior-py/src/lib.rs` (admit first; on admission failure return the AdmissionResult JSON)
- [X] T059 [US2] Implement `eval` and `replay` subcommands in `crates/behavior-cli/src/main.rs` with the exit codes from contracts/engine-api.md; make T052 pass
- [X] T060 [US2] Implement `Decision`, `TraceStep`, `Change`, `ReplayResult` in `python/behavior/results.py` and `evaluate(model, action, *, state, input, context, data_version, git_revision=None)` and `replay(model, record_json)` in `python/behavior/__init__.py`, encoding values per data-model.md (rejecting `float` with `TypeError`, `Decimal` as normalized strings, `None` as option none, `Enum` members as their values) and raising `BehaviorInvalid` on admission failure; make T053 pass
- [X] T061 [P] [US2] Create `examples/invoice/run.py` printing the quickstart.md §2 cases (result, reasons, changes, and the record JSON) and extend `examples/project_margin/run.py` to evaluate `flag_project` with and without zero revenue
- [X] T062 [US2] Extend `scripts/determinism-check.sh` with the evaluation and replay cases: for every request fixture run `behavior eval` twice and `behavior replay` on the output, and run `python -m examples.invoice.run` twice; fail if any pair of outputs differs byte for byte or any replay does not match
- [X] T063 [US2] Run the US2 validation: `cargo test --workspace`, `pytest python/tests/test_evaluate.py`, `scripts/determinism-check.sh`, and quickstart.md §2 and §8; fix any failures

**Checkpoint**: User Stories 1 and 2 both work; decisions are deterministic, auditable, and
replayable

---

## Phase 5: User Story 3 - AI acts only through capabilities (Priority: P3)

**Goal**: Structured intents choose only capability, target ids, and input; the trusted host
supplies state, context, and data version; invalid intents are rejected before evaluation with
every problem listed; accepted intents evaluate exactly like direct requests (research R13).

**Independent Test**: run every recorded intent in `tests/fixtures/intents/` against the invoice
wire IR and host file; only the valid ones are evaluated, and their records equal the records of
the equivalent direct requests (quickstart §7).

### Tests for User Story 3 (write first, must fail)

- [X] T064 [P] [US3] Create `tests/fixtures/host/invoice_1042.json` (HostContext: `data_version`, `state.invoice` with `id` `"1042"`, `context.actor` manager `anna` with limit `"50000"`) and recorded intents in `tests/fixtures/intents/`: `valid_allowed.json`, `valid_denied.json` (host variant `tests/fixtures/host/invoice_1042_over_limit.json`), `unknown_capability.json`, `missing_argument.json` (`apply_discount` without `discount`), `extra_argument.json`, `wrong_type.json` (discount `"abc"`), `extra_field.json` (unknown top-level key), `missing_target.json`, `extra_target.json`, `target_mismatch.json` (targets invoice `"9999"`), and `multiple_problems.json` (three problems at once); for each rejected intent add `<name>.expected.json` with the IntentRejection errors sorted by `path`
- [X] T065 [P] [US3] Write `crates/behavior-core/tests/intent.rs`: each rejected intent returns exactly its expected IntentRejection and performs no evaluation; `valid_allowed` and `valid_denied` return records byte-identical to the records of the equivalent direct EvaluationRequests built from the same host file; an intent cannot supply `context` or `state` (such keys are `EXTRA_ARGUMENT`/decode errors)
- [X] T066 [P] [US3] Write `crates/behavior-cli/tests/cli_intent.rs` (exit `0` for `valid_allowed`, `1` for `valid_denied`, `2` for every rejected intent, stdout equal to the expected JSON) and `python/tests/test_intent.py` (`evaluate_intent` returns a `Decision` for valid intents and raises `IntentRejected` listing all errors otherwise)

### Implementation for User Story 3

- [X] T067 [US3] Implement `crates/behavior-core/src/intent.rs`: strict decoding of StructuredIntent `{capability, targets, input}` and HostContext `{data_version, state, context}`; checks producing `UNKNOWN_CAPABILITY`, `MISSING_ARGUMENT`, `EXTRA_ARGUMENT`, `WRONG_TYPE`, `MISSING_TARGET`, `EXTRA_TARGET`, `TARGET_MISMATCH` ("target id ≠ supplied state entity's `id`"), all collected and sorted by `path`; on success merge into an EvaluationRequest and call the same evaluation path as `evaluate`; expose `evaluate_intent` in `crates/behavior-core/src/lib.rs`; make T065 pass
- [X] T068 [US3] Expose `evaluate_intent(wire, intent, host) -> str` in `crates/behavior-py/src/lib.rs` and implement the `intent` subcommand in `crates/behavior-cli/src/main.rs` (exit `0` ALLOW, `1` DENY, `2` rejected or invalid, `3` ERROR)
- [X] T069 [US3] Implement `evaluate_intent(model, intent, *, state, context, data_version)` in `python/behavior/__init__.py` raising `IntentRejected` with all errors; make T066 pass
- [X] T070 [US3] Run the US3 validation: `cargo test --workspace`, `pytest python/tests/test_intent.py`, and quickstart.md §7; fix any failures

**Checkpoint**: all three user stories work independently

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Performance, schema, documentation, and final validation

- [X] T071 [P] Write the JSON Schema `schema/wire-ir-0.1.schema.json` for contracts/ir-encoding.md and `python/tests/test_schema.py` validating every file in `tests/fixtures/wire/valid/` and `tests/fixtures/wire/python/` against it (add `jsonschema` to the dev shell in `flake.nix`)
- [X] T072 [P] Write `crates/behavior-core/tests/perf.rs` (marked `#[ignore]`, run with `cargo test --release -- --ignored`): generate a wire module with 200 derived values/rules and 50 actions, assert admission < 2 s and a single evaluation < 10 ms (SC-005); the command `cargo test --release -p behavior-core -- --ignored perf` is listed in quickstart.md §1 and `README.md`
- [X] T073 [P] Run `mypy` (strict) on `python/behavior` and fix all findings; run `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` and fix all findings
- [X] T074 [P] Update `README.md`: purpose (link `PRINCIPLES.md`), dev shell, build, test gates from quickstart.md §1, and a short DSL example; do not duplicate the contracts
- [X] T075 Review the implementation against `PRINCIPLES.md` §2–§11 and constitution principles I–VI: no path evaluates unadmitted IR, no hash includes locations or human names, no output contains timestamps, effects only target state, no hand-written `unsafe` in `behavior-py`; record the review result in `specs/001-verifiable-behavior-ir/checklists/implementation-review.md`
- [X] T076 Run the full quickstart.md (§1–§8) from a clean `nix develop` shell and fix any failures

---

## Phase 7: Revision — Python as a binding to the engine (2026-09-25)

**Purpose**: Replace "Python emits wire JSON" with "Python drives the Rust builder through PyO3"
(research R10, R12, R17). The public Python API, hash vectors, versions, and decision records
must not change; they are the regression net.

- [X] T077 Update plan.md, research.md (R10, R12, R17), contracts/engine-api.md, contracts/python-api.md, and contracts/ir-encoding.md for the binding design
- [X] T078 [P] Write `crates/behavior-core/tests/serialize.rs`: for every file in `tests/fixtures/wire/valid/` and every hash vector, `admit → serialize → admit` gives the same behavior version and items, and serializing twice (serialize ∘ admit ∘ serialize) gives identical bytes; the output uses explicit `some`/`to_decimal` and lists declarations by name
- [X] T079 [P] Write `crates/behavior-core/tests/builder.rs`: building the invoice and project margin models through the `Builder` API yields exactly the behavior versions of `tests/fixtures/wire/valid/invoice.json` and `project_margin.json`; node constructors return typed nodes or errors with codes (`TYPE_MISMATCH` for Money + Decimal, `OP_NOT_ALLOWED`, `UNKNOWN_FIELD`, `UNKNOWN_PARAM`, `NOT_BOOLEAN` via `check_condition`, `EFFECT_ON_READONLY` and `RESERVED_NAME` via `check_effect`); a derived cycle built with untyped references is reported by `finish` as `CYCLE`
- [X] T080 Add source locations for enums, nominal types, entities, and fields to the semantic `Module` (outside the hash) and implement `crates/behavior-core/src/serialize.rs` (admitted module → canonical wire JSON); expose `admit_wire(WModule)` so JSON decoding and the builder share one admission entry point; make T078 pass
- [X] T081 Implement `crates/behavior-core/src/builder.rs`: declarations, scope stack, node constructors that run `admit/typecheck.rs` on each new node (untyped nodes above unresolved derived references skip the check), `check_condition`, `check_effect`, `add_derived`/`add_invariant`/`add_action`, and `finish(root)` that relativizes locations and calls `admit_wire`; make T079 pass
- [X] T082 Implement PyO3 classes `Builder`, `Node`, and `Module` in `crates/behavior-py/src/lib.rs` per contracts/engine-api.md (errors as `ValueError` with a JSON `{code, message}` payload); update `python/behavior/_engine.pyi`
- [X] T083 Rewrite `python/behavior/types.py`, `expr.py`, `decl.py`, `statements.py`, `module.py`, and `__init__.py` to drive the engine builder: remove the Python typing rules and JSON emission, keep decorators, tracing, locations, operator overloading, and float rejection; delete `python/tests/test_types.py` (the typing table now runs only against the engine)
- [X] T084 Regenerate `tests/fixtures/wire/python/*.json` from the engine serializer and review them; confirm `tests/fixtures/versions.json`, `tests/fixtures/hash_vectors.json`, and `tests/fixtures/records/*.json` are unchanged
- [X] T085 Run all gates (fmt, clippy, cargo tests, pytest, mypy, determinism check, perf) and quickstart.md §1–§8; update README and the implementation review

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: depends on Setup; blocks all user stories
- **US1 (Phase 3)**, **US2 (Phase 4)**, **US3 (Phase 5)**: each depends only on Foundational
- **Polish (Phase 6)**: depends on the stories you ship
- **Revision (Phase 7)**: depends on Phases 1–6; T078–T079 before T080–T081, T082 before T083

### User Story Dependencies

- **US1 (P1)**: after Foundational; no dependency on other stories
- **US2 (P2)**: after Foundational; uses hand-written wire fixtures, so it does not need US1.
  Only T053, T060, and T061 (Python evaluation API and examples) need US1's DSL
- **US3 (P3)**: after Foundational and US2's evaluation path (T055–T057); does not need US1
  except T066's Python test and T069

### Within Each Phase

- Test tasks first; run them and confirm they fail
- Rust: `types.rs` → `semantic/*` → `resolve` → `graph` → `typecheck` → `hash` → `admit/mod.rs`
- Python: `types.py` → `expr.py` → `decl.py` → `statements.py` → `module.py` → `__init__.py`
- Golden generation tasks (T029, T046) come after the implementation they freeze and require a
  human review of the generated content

### Parallel Opportunities

- Setup: T004–T008 in parallel after T003; T010 after T009
- Foundational tests: T011–T016 and T018 in parallel; T019 and T020 in parallel
- US1 tests: T030–T035 in parallel; T044 and T045 in parallel with T037–T043
- US2 tests: T048–T053 in parallel; T054 in parallel with T055
- US3 tests: T064–T066 in parallel
- Polish: T071–T074 in parallel
- With two people after Foundational: one on US1 (Python), one on US2 then US3 (Rust)

---

## Parallel Example: User Story 1

```bash
# Tests first, together:
Task: "Write python/tests/test_types.py against tests/fixtures/typing_cases.json"
Task: "Create DSL fixtures in python/tests/fixtures/"
Task: "Write python/tests/test_dsl.py construction-time error cases"
Task: "Write crates/behavior-cli/tests/cli_admit.rs"

# Examples alongside the DSL implementation:
Task: "Create examples/invoice/behavior.py and dump.py"
Task: "Create examples/project_margin/behavior.py and run.py"
```

## Parallel Example: User Story 2

```bash
Task: "Create request fixtures in tests/fixtures/requests/"
Task: "Create golden records in tests/fixtures/records/"
Task: "Write crates/behavior-core/tests/evaluate.rs"
Task: "Write crates/behavior-core/tests/replay.rs"
Task: "Write crates/behavior-cli/tests/cli_eval.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phase 1: Setup
2. Phase 2: Foundational (the admission pipeline and hashing are the heart of the system)
3. Phase 3: US1
4. **Stop and validate**: quickstart.md §3–§6; behavior is authored in Python, typed at both
   layers, admitted, and content-addressed

### Incremental Delivery

1. Setup + Foundational → hand-written wire IR admitted with stable hashes
2. + US1 → Python authoring (MVP)
3. + US2 → deterministic execution, decision records, replay
4. + US3 → capability boundary for AI intents
5. Polish → schema, performance, review

---

## Notes

- [P] tasks touch different files and do not depend on incomplete tasks
- Commit after each task or logical group; never commit a failing gate
- Goldens (hash vectors, wire files, versions, records) are reviewed by a human before commit;
  changing a frozen hash vector requires new hash tags
- Stop at any checkpoint to validate a story on its own
