# Python document access contract

Core012/013 at the exact release define semantics; these wrappers add no interpretation.

- BehaviorModule.from_wire_json(text): native admission; legacy authoring profile unchanged.
- invoke(model, document, snapshot, context=None): InvocationRecord with data/json,
  record_id/outcome_kind/refusal_stage/inner_record/diagnostics native accessors.
- replay_invocation(model, record): native ReplayResult; altered records fail.
- Store.invoke/invoke_intent: Invocation(record,bundle), with explicit time/context/state.
  Store.invoke uses Core's typed requested-invocation path: malformed transport or decoded
  document shape raises BehaviorError with ordered native diagnostics; a valid document's
  resolution/evaluation refusals return records. Plain invoke and Store.invoke_intent retain
  Core's complete textual decode-refusal records for parseable inputs.
- Store.current_history/history_at/commands_since: checked HistoryRef and CommandStreamPage.
- Store.genesis_v2_for/commit_with_context/export_seed_at: explicit checked trust/adoption paths.
- Native candidate inspection/evidence attachment preserves independent context requirements.
- Decision.commands/diagnostics: candidate intents and detached display data, never added to
  canonical record_json; current trace source fields may be absent.
- Raw JSON document text reaches native checked decoders unchanged. Object inputs use existing
  engine conversion; Python must not discard duplicate keys from raw text.
  Store.commit/commit_with_context accept raw bundle text; Core decodes it before choosing its
  evaluated parent when expected_parent is omitted. Trusted commit takes independently supplied
  context and execution_policy documents, rather than recovering context from signed evidence.

Authenticated governance/proof generation is accessible through the bundled CLI. Prepared
migration store operations remain public Rust APIs. A new Python command/governance DSL is
outside this alignment.

Wire import retains admitted core semantics without reconstructing Python entity/type
declarations. Use Migration.from_json for migrations between imported schemas; Python
transformation authoring depends on Python declarations. commit_with_context uses the supplied
execution policy to decode the independent context; core independently enforces the actual
signed policy allowed by the immutable store genesis.
