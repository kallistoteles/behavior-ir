# Implementation Log: 501 Deterministic Model Lowering and StateMachine Model

**Date**: 2026-10-12 | **Branch**: `501-model-lowering-state-machine` | **Core Release**: 0.12.0
(`b525da46d4282166418a90141f0c94fd099e9b47`), unchanged.

## Result (T072, T073)

`nix develop -c scripts/gates.sh` passes all 16 gates: core pin, public surface, fetch core,
cargo fmt, clippy, **cargo test** (new), maturin develop, pytest (387 passed, 4 skipped), mypy,
**models compat** (new), determinism check (now including model receipts), ownership, terms,
workflows, and script tests (including the new `test_models_compat.sh`).

Rust suites in `crates/behavior-models/tests`:

| Suite | Tests | Notes |
|---|---|---|
| core_assumptions | 3 | permanent checks of the pinned Core (fragment order, entry-literal probe) |
| ids | 4 | 2,000-case injectivity property |
| errors | 3 | |
| resolve | 11 | |
| lower | 15 | |
| ownership | 10 | |
| identity | 7 | |
| laws | 5 | 1,000 cases each (fixed seed 501) |
| composition | 8 | |
| fixture_smoke | 1 | |

No change was made to behavior-ir-core. Its working tree shows only the maintainer's
in-progress feature 014 files.

## Pinned reference hashes (`models/fixtures/compat.json`)

Tuple: `state_machine`, model semantic version 1, lowerer version 1, Core 0.12.0.

| Case | BehaviorHash | ModelHash(es) |
|---|---|---|
| order | `sha256:eaeae2756f0624ccdb8acb57711fc4a9b07c032256c6f35d87aaa4fb1c65d970` | order `sha256:d1736e102f4cf0925fe1ca28cf8125bc0f3922b380a29b5921b69114c887aad4` |
| purchase_request | `sha256:02286fb386b82177012dfc4328b39b06bbe4ffbeb200da2b02ea59b69dbaa10a` | purchase_request `sha256:82fc685b77238ba8af3491771d51cc51faebc0c5c69a3a3a56192a33e380642b` |
| request_flow_reference | `sha256:7192caa4714bd3a2eff38a4058a2803ed04f718222316e3bf1157b08bff75d30` | request `sha256:6da1d88ce821d013f44b40724d4a975f20a07c3362f7da401580d79ffd0a683b` |
| two_machines_same_local_ids | `sha256:5bd2c5c65f10f56a6097a1942e342e6ff5c3db0ebc002a8e67d01f837efeb88f` | invoice `sha256:51330af4…7761`, quote `sha256:ac6c0189…7c423` |

## Findings during implementation

- **Entry-literal probe (analysis U1), confirmed and resolved.** `Builder::for_module` cannot
  admit literals of the module's enums (`UNKNOWN_TYPE`). The probe is instead a fresh builder
  that re-declares the enum with the module's cases and semantic profile, and it asserts that
  the enum hash is equal. This is recorded in research R8 and the composition contract, and kept
  as tests `entry_literal_probe_matches_creation_literal` and
  `for_module_probe_does_not_know_module_enums`. No Core change was needed.
- **Fragment order (analysis U2), confirmed.** Admitted actions keep the given order of
  preconditions, effects and postconditions (`admitted_action_keeps_fragment_order`).
  Positional fragment identity stands.
- **Core operator names** are words (`eq`, `ne`, `value_or`, …), not symbols.
- **Resolution snapshot.** `Builder` does not expose its declarations, but `finish(&self)` does
  not consume it. `compose` resolves models against an admitted snapshot of the ordinary
  behavior, which also reports inadmissible ordinary behavior before any model work.
- **Python fragment tracing.** Core nodes are tied to their builder. The Python glue moves the
  builder out of the Python `Builder` object for the duration of `compose`, and swaps it back in
  only while Python traces a transition's fragments in the scope Rust pushed.
- **The gates did not run Rust tests.** `gates.sh` ran only `clippy` for Rust. A `cargo test -p
  behavior-models` gate was added, because the constitution requires all tests to pass.
- **CI history.** `ecosystem-ci.yml`'s gates job now checks out full history (`fetch-depth: 0`)
  for the compat gate's merge-base comparison. Without history the gate fails loudly.

## Convergence (Phase 9, T075–T078)

All four convergence findings are closed:

- **T075, commands from Python.** The Python DSL gained minimal command authoring: `@command`
  declarations, `BehaviorModule(commands=[...])` (selects Core's command profile; modules without
  commands keep the legacy profile and their hashes), and `emit(...)` used through
  `StateMachine.transition(emits=[...])`. Core 012 semantics apply: a command-profile evaluation
  takes a complete state snapshot. Tests: `test_a_transition_emits_a_command_in_the_same_commit`,
  `test_a_denied_transition_emits_nothing`, `test_commands_enter_the_model_hash`.
- **T076, checkout-independent receipts.** Source-map locations are relative to the build
  `root`, or else to the common directory of the model's own sources. Core's own relativization
  could not be mirrored, because the Python session declares some enums with location
  `<unknown>`, which makes Core's common directory `/`. Tests:
  `receipts_are_identical_across_checkouts` (Rust, different roots) and
  `test_receipts_are_identical_across_checkouts` (Python, two temporary checkouts).
- **T077.** `UNREACHABLE_TRANSITION` for `F_t ∩ Reach(Q_entry) = ∅`
  (`a_transition_from_unreachable_states_is_reported_even_if_its_target_is_reachable`).
- **T078.** `LoweringReceipt` is a frozen Python type over the Rust receipt. `from_dict` fails on
  any field drift, so Rust keeps owning the structure (`test_receipts_are_typed_views_of_the_authoritys_receipt`).

## Deviations from tasks (recorded, not hidden)

- ~~No Python command emissions (T028)~~: closed by T075.
- ~~Receipts are dicts in Python (T054)~~: closed by T078. Diagnostics remain plain dicts.
  `api/public-api.json` gained `StateMachine`, `ModelBuildError`, `LoweringReceipt`,
  `Emission`, `command` and `emit`.
- **Receipts include the canonical document** (`model`), so ModelHash can be re-derived from a
  receipt alone. Data model §9 and identity contract §3 were updated.
- **Ownership manifest.** Rule 1 was extended with `crates/behavior-models/**`, instead of
  inserting a new row before rule 3. This avoids renumbering rules that other documents cite by
  number.
- **Parameter order (clarification during implementation).** Inputs and context are mappings,
  canonicalized by name in Rust. The spec, data model, contracts and a property test cover
  this.

## Anti-pattern audit with evidence (T074)

| Anti-pattern | Evidence it is absent |
|---|---|
| Python performs semantic normalization before Rust | `python/behavior/models.py` passes lists as given (`_case`, `_params` only convert syntax). `test_python_names_and_collection_order_are_not_semantic` uses duplicated and unsorted Python input; `resolve::duplicate_cases_in_input_sets_are_set_semantics` |
| Raw Wire JSON as the model interface | `scripts/check-public-surface.sh` fails on `into_wire` or `to_wire_json` in `crates/behavior-models/src` (negative-tested). Fragments are `Node`s in the shared builder |
| ModelHash based on authoring syntax | Fragment entries are Core semantic hashes (`identity::canonical_document`). `laws::fragment_identity_is_semantic`; `identity::metadata_and_source_map_data_never_change_identity` |
| Duplicate expression language | The crate has no expression type. `TransitionFragments` holds `behavior_engine::builder::Node` |
| Source-order-dependent generated identifiers | `ids::encode` uses ids only. `laws::order_independence`, `laws::composition_order` (1,000 cases each) |
| Automatic renaming to resolve collisions | `composition::a_hand_written_item_taking_a_generated_name_is_a_collision_never_renamed` |
| Nonexistence as a pseudo-state | The data model has no creation construct. Creations are checked by `composition::final_pass`; `ownership::creation_*` tests |
| Hand-written actions bypassing the owned field | `composition::final_pass` runs over the admitted module. `ownership::a_hand_written_writer_of_the_owned_field_is_rejected`, `…_owned_by_another_machine_is_rejected`, `every_public_way_to_compose_checks_ownership` (no option skips it) |
| Golden fixtures silently rewritten | `scripts/check-models-compat.py` merge-base check. `scripts/tests/test_models_compat.sh` ("edited expected hash, same tuple" → FAIL) |
| Model runtime logic surviving lowering | `lower::lowered_module_contains_only_ordinary_items`, `composition::state_machine_contributes_only_actions`. The worked example's lowered module equals a hand-written one by BehaviorHash |
