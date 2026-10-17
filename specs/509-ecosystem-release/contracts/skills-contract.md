# Contract: Consumer Skills for Behavior 0.13.0

## Existing interface retained

Keep exactly behavior-authoring, behavior-verification and behavior-application with their
current SKILL.md/examples layout and established section sequence. Add concise subsections
where needed; preserve working existing examples. All three release fields and scope lines
remain 0.13.0. Skills are consumed from the release tag alongside the matching installed wheel.

Keep the existing semantic-gap rule verbatim and the seven fixed gap-log fields. Correct current
capability guidance without erasing historical gap records or inventing host-side business rules.
Excerpts must cite `# from examples/<file>.py` and match executable source lines in order.
Imports use public root exports from behavior; repository helpers, private modules and
hand-written IR are excluded. A small explicit policy document accepted by the public Store
API is host configuration, not a replacement implementation of Core semantics.

## Essential coverage through four compact standalone examples

| Planned file under skills/ | Required observable coverage |
|---|---|
| behavior-authoring/examples/models_composition.py | StateMachine on an authored entity; RequestFlow with generated request_entity; Approval over that entity and its generated decision_entity; outcome predicates, receipts and a rejected ownership violation |
| behavior-authoring/examples/work_queue.py | WorkQueue declaration and explicit authorization, generated work handle/reads, state binding and completion effect; all lower through public Models |
| behavior-verification/examples/approval_verification.py | Assert actual 0.10.0 local postcondition proof outcomes for a small Approval; show what is proved without promoting global arguments/tests to SMT |
| behavior-application/examples/work_queue_store.py | Public Store enqueue/claim/current-generation completion, trusted context, atomic bound-entity effect plus durable command intent, command observation and replay |

Examples may be combined or existing ones extended when that stays shorter and satisfies the
same coverage. No exhaustive API or composition matrix is required. Each example must work
alone when copied to an empty directory with the installed package. If the command example
needs Store-v2, author its minimal explicit public policy within the example; do not load or
copy a Core fixture or import the repository workflow. An external command is an intent only;
there is no scheduler, physical worker implementation or external side effect in these examples.

Reuse existing behavior-application/examples/conflict.py for conflict re-evaluation and
lost-ack original-bundle resubmission. Its current assertions already distinguish the cases.

## Guidance that examples do not turn into universal guarantees

- Models generate ordinary Core semantics and enforce ownership over the final module.
  Additional state bindings grant no authority to write another Model's owned fields.
- Keep existing fragment calling conventions: SM/RF positional, Approval's established leading
  arguments, WorkQueue positional subject with named remaining arguments and no subject at enqueue.
- Approval query congruence is improved in verifier 0.10.0; other inconclusive patterns do not
  become proven by association. Keep precision-debt examples aligned with actual outcomes.
- Local SMT checks, structural/inductive safety arguments, tests and Store contracts retain
  separate evidence classes. INCONCLUSIVE remains unproven and blocking where proof is required.
- WorkQueue time is recorded evaluation time. Freshness at commit, clocks, backend conformity,
  imported-state provenance and migration restrictions remain host assumptions/contracts.
  Valid migration output alone does not preserve fencing or terminality across migrations.
- Ordinary retries re-evaluate a new attempt; lost acknowledgement recovers the original bundle
  and committed event. Durable command intents are not proof of exactly-once external execution.

## Existing validation path

Run test_skills.py for layout, release, public references, verbatim excerpts and executable
examples. release-check.sh independently copies every example to a clean consumer environment
and executes it against the built wheel. Existing public API/equivalence and conformance checks
remain authoritative. Add no example registry, new validation framework, external application
review or manual agent acceptance campaign.
