# Data model

- Core pin: immutable commit/tag/version and artifact SHA256 values agree with Cargo resolution.
- BehaviorModule: native admitted module from legacy builder or raw current wire document.
- InvocationRecord: native canonical record, identity, outcome/refusal and optional inner record.
- Invocation: record plus optional uncommitted candidate; reads/refusals never produce a bundle.
- HistoryRef: checked format/store/state/position/record document.
- CommandStreamPage: observed head, ordered whole-event items, continuation and completeness.
- Decision: canonical record plus detached optional diagnostics and command intents.
- Governance context: execution-policy-validated, independently supplied at live commit.

Native core validates documents, hashes identities, orders events and determines refusals.
