# Remaining boundaries and deferred work

The complete frozen capacity corpus and existing full release checks passed. These boundaries
remain explicit; they are not waivers for future failed profile cases or missing proof obligations.

- **Original permission-list cost.** The authority predicate is unchanged. Increasing both
  action and writer-list sizes retains quadratic membership work (isolated prototype authority
  grew from roughly 15 s at A256 to 60 s at A512). This is implementation overhead, not a
  mathematical necessity. A replacement needs a checked equivalence and full reevaluation;
  it is unnecessary to alter it merely for the passing frozen profile floors.
- **Fresh dependency cost.** Generic compilation and fresh replay of the actual pinned import
  closure impose a substantial constant cost. No result caching or omitted kernel checks is
  accepted. Further dependency reductions are optional research, subject to the same theorem,
  axiom, statement and fresh-replay contract, not an approved extension of this work.
- **Measured capacity is finite.** A256/A512 are minimum regression references, never an action
  count limit or a promise of bounded runtime for arbitrary modules. Report the entire fixed
  corpus and residual growth. Finite stage/resource controls may return Inconclusive; they
  are not safety counterexamples or Unsupported relabeling of valid v1 language.
- **Correspondence remains trusted.** Rust extraction, policy/probe interpretation, exact
  encoding/expected wrappers and Core/backend semantics are explicit assumptions. The generic
  and concrete Lean proofs do not establish complete end-to-end verification of that code.
  A verified extractor or larger correspondence project requires its own justified scope.
- **Platform measurements.** The binding performance profile is x86_64-linux. No ARM timings
  have been measured. Preserve supported-platform correctness requirements and document actual
  availability; a new binding ARM performance profile requires reproducible measurements.
- **Measurement precision.** Consumed heartbeat/recursion high-water counts are unavailable.
  Configured limits are not consumption. Linux RSS/VmSize sampling is not an exact memory peak;
  short-lived processes/peaks may be missed and shared pages may be counted repeatedly. Per-tool
  selected-process samples and whole-role process-tree samples have different scopes.
- **Observation safeguards.** The external harness has a 600 s per-role watchdog and
  accepts compact worker summaries up to 1 MiB. These bound observation, not the public
  Safety language or mathematical truth. The accepted profile still requires each complete
  role within 300 s; an interrupted or unreadable result never counts as a passing case.
- **Conditional deployment guarantee.** Initial-state validation, migrations, exact approved
  BehaviorHash/complete policy sets and a fresh enforced activation boundary remain separate
  obligations. Certificates are build artifacts and grant no Core governance authorization.

Published artifacts remain immutable. Historical reproducibility is distinct from continuing
trust in the old verifier. Publication of any new artifact is a separate explicit action; later
relevant build/source/checker/proof changes require a new frozen build and complete evaluation.
