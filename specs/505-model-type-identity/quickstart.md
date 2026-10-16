# Quickstart: validating feature 505

Run everything from the repository root inside `nix develop`.

| # | Scenario | Command | Expected |
|---|---|---|---|
| 1 | A StateMachine follows its referenced declarations | `cargo test -p behavior-models --test identity declaration` | Changing the subject, an unused input enum, a nominal or an identity type changes the hash. An unrelated declaration, metadata and order do not. A renamed type does. |
| 2 | A RequestFlow follows its referenced declarations | `cargo test -p behavior-models --test request_flow_identity declaration` | The same for the actor, payload, a creation's input and context, a state binding, and an embedded transition's parameter. |
| 3 | An unresolvable reference fails | `cargo test -p behavior-models --lib identity` | `MODEL_IDENTITY_FAILED` names the model, the item and the type. |
| 4 | Lowering is unchanged | `git show main:models/fixtures/compat.json > /tmp/before.json`, then compare `behavior_hash` and `generated` per case with the new manifest | Every BehaviorHash and generated hash is equal. Only StateMachine and RequestFlow model and embedded hashes differ. |
| 5 | Approval is unchanged | `cargo test -p behavior-models --test approval_identity` and quickstart 4 | All tests pass, and every ApprovalHash is equal. |
| 6 | Gate supersession | `scripts/tests/test_models_compat.sh` | Every case of contract `compat-supersession.md` §6 passes. |
| 7 | Manifest | `python scripts/check-models-compat.py` | `OK`. One tuple per case: state_machine 2, request_flow 2 (embedded 2), and approval with `co_supplied` 2. Each v2 or re-keyed tuple lists the tuple it replaces in `supersedes`, with the same case names. |
| 8 | Pinned Python cases | `pytest python/tests/test_model_staging.py -k pinned_cases` | Passes against the v2 tuples. |
| 9 | Determinism | `BEHAVIOR_FULL_PROPERTIES=1 cargo test -p behavior-models --test request_flow_identity` | The permutation property passes with 1,000 cases. |
| 10 | Gates | `scripts/gates.sh` | `gates: OK` |
