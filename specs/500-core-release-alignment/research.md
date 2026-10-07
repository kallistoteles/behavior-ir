# Research

## Decision: Published Core Release first

Core main b525da46d4282166418a90141f0c94fd099e9b47 declares0.12.0. Its remote gates
and consumer pass, but fresh release checks must cover the merge revision; older acceptance
evidence does not. No path override is valid compatibility evidence.

## Decision: Native document boundaries

Two planning agents inspected actual core facade and binding code. Existing paths and Backend
signatures remain compatible. Preserve legacy Builder::new(). Add checked wire import rather
than design a new command DSL. Delegate invocation/replay and exact history/paging to core.
Use fetched release fixtures; retain raw JSON strings until the checked native decoder.

## Decision: Current governance refusal

Fresh v1 required-governance writes now refuse TRUSTED_GOVERNANCE_UPGRADE_REQUIRED.
Tests must assert no state/history mutation. Explicit v2 genesis and independent context
provide adoption; packaged governance CLI creates authenticated proofs and authorization.

## Decision: Detached diagnostics

Decision0.7 traces omit loc/expr_text. Python display fields must be optional and diagnostics
and candidate commands separate from canonical record_json. Core owns all identities.
