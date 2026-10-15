//! Decision lowering (data model §6, lowerer version 1). For capability `c` of approval `p`:
//!
//! ```text
//! action   encode(p, c)
//! params   decision_id: Input Id(X), inputs (sorted), subject: State S, context (sorted)
//! pre      [0] Open  [1] p__pending(subject)  [2] Eligible
//!          [3] ¬any(select(X), d → d.subject == subject.id ∧ d.actor == actor)  [4..] G_c
//! create   X { id: decision_id, actor, choice: lit(C, choice(c)), subject: subject.id, <payload> }
//! effects  none
//! post     [0] count(where(select(X), d → d.subject == subject.id ∧ d.actor == actor)) ≤ 1
//!          [1] ¬(p__approve_count(subject) ≥ k_A ∧ p__reject_count(subject) ≥ k_R)   iff k_R ≠ ⊥
//! commands K_c
//! ```
//!
//! The postconditions are Core postconditions, so they are evaluated on the child state
//! `S′ = Apply(S, CreateEntity(d))`, the state that includes the new decision. With structural
//! ownership (`ownership.rs`) they preserve the Approval laws inductively over every ordinary
//! transition: a decision action changes `Count(s, a)` only for its own `(subject, actor)` and the
//! counts only for its own subject, and no other ordinary action creates, writes or removes a
//! decision (spec FR-008b).
//!
//! This and `request_flow/create.rs` are the only files of the crate that may call `into_wire`:
//! the builder's creation input type (`WLifecycle::Create`) takes wire expressions (research R8).

use std::collections::BTreeSet;

use behavior_engine::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_engine::wire::{Loc, WLifecycle, WParam, WType};
use serde_json::json;

use crate::approval::input::{ApprovalSource, DecisionFragments, DecisionScope};
use crate::approval::resolve::{
    ACTOR, CHOICE, DECISION_ID, ID, ResolvedApproval, ResolvedCapability, SUBJECT,
};
use crate::composition::admission_failed;
use crate::error::ModelError;

/// Which generated conditions a decision action carries. Production always lowers the full
/// layout; the child-state test (T017) omits parts to show which check refuses a child state.
#[derive(Debug, Clone, Copy)]
struct Layout {
    preconditions: bool,
    postconditions: bool,
}

const FULL: Layout = Layout {
    preconditions: true,
    postconditions: true,
};

fn loc_of(a: &ResolvedApproval, c: &ResolvedCapability) -> Loc {
    match c.source.as_ref().or(a.source.as_ref()) {
        Some(s) => Loc {
            file: s.file.clone(),
            line: s.line,
        },
        None => Loc {
            file: format!("{}.model", a.approval_id),
            line: 1,
        },
    }
}

fn fragment_error(c_ref: &str, e: BuildError) -> ModelError {
    ModelError::FragmentInvalid {
        model_ref: c_ref.to_string(),
        core_code: e.code,
        core_message: e.message,
    }
}

/// A lambda parameter name that no scope parameter takes. Lambda parameter names are display
/// only (Core hashes the candidate, not its name), so the choice never reaches a hash.
fn candidate_name(params: &[WParam]) -> String {
    let taken: BTreeSet<&str> = params.iter().map(|p| p.name.as_str()).collect();
    std::iter::once("d".to_string())
        .chain((0..).map(|i| format!("d{i}")))
        .find(|n| !taken.contains(n.as_str()))
        .unwrap_or_else(|| "d".to_string())
}

/// The static initializer checks (data model §6): every payload field once, nothing else.
fn check_initializer(
    a: &ResolvedApproval,
    c_ref: &str,
    fragments: &DecisionFragments,
) -> Vec<ModelError> {
    let required = a.initialized_fields();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for (field, _) in &fragments.payload {
        if !required.contains(field) {
            errors.push(ModelError::InitializerUnknownField {
                model_ref: c_ref.to_string(),
                field: field.clone(),
            });
        } else if !seen.insert(field.clone()) {
            errors.push(ModelError::InitializerDuplicateField {
                model_ref: c_ref.to_string(),
                field: field.clone(),
            });
        }
    }
    for field in required.difference(&seen) {
        errors.push(ModelError::InitializerMissingField {
            model_ref: c_ref.to_string(),
            field: field.clone(),
        });
    }
    errors
}

/// Lowers capability `c` of `a` into `builder`. The scope is always popped again.
pub fn lower_decision(
    builder: &mut Builder,
    a: &ResolvedApproval,
    c: &ResolvedCapability,
    source: &mut dyn ApprovalSource,
) -> Result<(), Vec<ModelError>> {
    lower_with(builder, a, c, source, FULL)
}

fn lower_with(
    builder: &mut Builder,
    a: &ResolvedApproval,
    c: &ResolvedCapability,
    source: &mut dyn ApprovalSource,
    layout: Layout,
) -> Result<(), Vec<ModelError>> {
    let c_ref = a.capability_ref(&c.capability_id);
    let params = a.capability_params(c);
    let loc = loc_of(a, c);
    builder
        .push_scope(ScopeSite::Action, params.clone(), loc.clone())
        .map_err(|e| vec![fragment_error(&c_ref, e)])?;
    let result = lower_in_scope(builder, a, c, &c_ref, params, loc, source, layout);
    builder.pop_scope();
    result
}

/// `op(select(X), d → d.subject == subject.id ∧ d.actor == actor)`: the decisions of this
/// actor on this subject (`where`), or whether there is one (`any`). The actor expression is a
/// captured value of the action scope.
fn pair_lambda(
    b: &mut Builder,
    a: &ResolvedApproval,
    op: &str,
    d: &str,
    actor: &Node,
    loc: &Loc,
) -> Result<Node, BuildError> {
    let select = b.select(&a.entity, loc.clone())?;
    b.push_lambda(d, &a.entity)?;
    let body = (|| {
        let subject = b.field(d, SUBJECT, loc.clone())?;
        let id = b.field(SUBJECT, ID, loc.clone())?;
        let same_subject = b.op("eq", vec![subject, id], loc.clone())?;
        let who = b.field(d, ACTOR, loc.clone())?;
        let same_actor = b.op("eq", vec![who, actor.clone()], loc.clone())?;
        b.op("and", vec![same_subject, same_actor], loc.clone())
    })();
    b.pop_scope();
    b.lambda(op, select, d, body?, loc.clone())
}

/// `name(subject) ≥ k`
fn at_least(b: &mut Builder, name: &str, k: u32, loc: &Loc) -> Result<Node, BuildError> {
    let count = b.derived_ref(name, vec![SUBJECT.into()], loc.clone())?;
    let k = b.lit(WType::Int, json!(k), loc.clone())?;
    b.op("ge", vec![count, k], loc.clone())
}

#[allow(clippy::too_many_arguments)]
fn lower_in_scope(
    builder: &mut Builder,
    a: &ResolvedApproval,
    c: &ResolvedCapability,
    c_ref: &str,
    params: Vec<WParam>,
    loc: Loc,
    source: &mut dyn ApprovalSource,
    layout: Layout,
) -> Result<(), Vec<ModelError>> {
    let scope = DecisionScope {
        approval_id: a.approval_id.to_string(),
        capability_id: c.capability_id.to_string(),
        params: params.clone(),
        actor_type: WType::Id(a.actor.clone()),
    };
    let fragments = source.trace_decision(builder, &scope).map_err(|e| {
        vec![ModelError::FragmentInvalid {
            model_ref: c_ref.to_string(),
            core_code: e.code,
            core_message: e.message,
        }]
    })?;
    let mut errors = check_initializer(a, c_ref, &fragments);
    let conditions = fragments
        .open
        .iter()
        .chain(&fragments.eligible)
        .chain(&fragments.guards);
    for (node, _) in conditions {
        if let Err(e) = builder.check_condition(node, "a decision condition") {
            errors.push(fragment_error(c_ref, e));
        }
    }
    let actor_type = WType::Id(a.actor.clone());
    match fragments.actor.0.wire_type() {
        Some(t) if t == actor_type => {}
        found => errors.push(ModelError::FragmentInvalid {
            model_ref: c_ref.to_string(),
            core_code: "TYPE_MISMATCH".into(),
            core_message: format!(
                "the actor expression must be `{actor_type:?}`, found `{}`",
                found.map_or_else(|| "an untyped expression".into(), |t| format!("{t:?}"))
            ),
        }),
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let DecisionFragments {
        actor: (actor, _),
        open,
        eligible,
        guards,
        payload,
        commands,
    } = fragments;
    let err = |e| vec![fragment_error(c_ref, e)];
    let d = candidate_name(&params);
    let truth = |b: &mut Builder| b.lit(WType::Bool, json!(true), loc.clone());

    let mut preconditions = Vec::new();
    if layout.preconditions {
        let open = match open {
            Some(o) => o,
            None => (truth(builder).map_err(err)?, loc.clone()),
        };
        let eligible = match eligible {
            Some(e) => e,
            None => (truth(builder).map_err(err)?, loc.clone()),
        };
        let pending = builder
            .derived_ref(
                &a.definition_name("pending"),
                vec![SUBJECT.into()],
                loc.clone(),
            )
            .map_err(err)?;
        let decided = pair_lambda(builder, a, "any", &d, &actor, &loc).map_err(err)?;
        let fresh = builder.op("not", vec![decided], loc.clone()).map_err(err)?;
        preconditions.push(open);
        preconditions.push((pending, loc.clone()));
        preconditions.push(eligible);
        preconditions.push((fresh, loc.clone()));
        preconditions.extend(guards);
    }

    let mut postconditions = Vec::new();
    if layout.postconditions {
        let pair = pair_lambda(builder, a, "where", &d, &actor, &loc).map_err(err)?;
        let count = builder.op("count", vec![pair], loc.clone()).map_err(err)?;
        let one = builder
            .lit(WType::Int, json!(1), loc.clone())
            .map_err(err)?;
        let unique = builder
            .op("le", vec![count, one], loc.clone())
            .map_err(err)?;
        postconditions.push((unique, loc.clone()));
        if let Some(k_r) = a.reject_threshold {
            let approved = at_least(
                builder,
                &a.definition_name("approve_count"),
                a.approve_threshold,
                &loc,
            )
            .map_err(err)?;
            let rejected =
                at_least(builder, &a.definition_name("reject_count"), k_r, &loc).map_err(err)?;
            let both = builder
                .op("and", vec![approved, rejected], loc.clone())
                .map_err(err)?;
            let consistent = builder.op("not", vec![both], loc.clone()).map_err(err)?;
            postconditions.push((consistent, loc.clone()));
        }
    }

    let id = builder.param(DECISION_ID, loc.clone()).map_err(err)?;
    let choice = builder
        .lit(
            WType::Enum(a.enum_name.clone()),
            json!(c.choice),
            loc.clone(),
        )
        .map_err(err)?;
    let subject = builder.field(SUBJECT, ID, loc.clone()).map_err(err)?;
    // Field values in the entity's canonical (sorted) field order.
    let mut values: Vec<(String, Node)> = payload;
    values.push((ACTOR.to_string(), actor));
    values.push((CHOICE.to_string(), choice));
    values.push((SUBJECT.to_string(), subject));
    values.sort_by(|x, y| x.0.cmp(&y.0));
    let lifecycle = vec![WLifecycle::Create {
        entity: a.entity.clone(),
        id: id.into_wire(),
        fields: values
            .into_iter()
            .map(|(n, v)| (n, v.into_wire()))
            .collect(),
        loc: loc.clone(),
    }];
    let added = if commands.is_empty() {
        builder.add_action(
            &c.action_name,
            params,
            preconditions,
            Vec::new(),
            lifecycle,
            postconditions,
            loc,
        )
    } else {
        builder.add_action_with_commands(
            &c.action_name,
            params,
            preconditions,
            Vec::new(),
            lifecycle,
            commands,
            postconditions,
            loc,
        )
    };
    added.map_err(|e| {
        vec![if e.code == "DUPLICATE_NAME" {
            ModelError::GeneratedNameCollision {
                model_ref: c_ref.to_string(),
                name: c.action_name.clone(),
            }
        } else {
            fragment_error(c_ref, e)
        }]
    })?;
    // The creation's values reach the builder as wire expressions, which `add_action` does not
    // type-check; admit now so an ill-typed payload value is reported against this capability.
    builder.finish(None).map(|_| ()).map_err(|r| {
        let e = admission_failed(r);
        vec![match e {
            ModelError::CoreAdmissionFailed { errors } => {
                let (code, message) = errors.into_iter().next().unwrap_or_default();
                ModelError::FragmentInvalid {
                    model_ref: c_ref.to_string(),
                    core_code: code,
                    core_message: message,
                }
            }
            other => other,
        }]
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)
mod tests {
    use super::*;
    use crate::approval::input::{ApprovalInput, CapabilityInput};
    use crate::approval::resolve::resolve;
    use crate::kinds::{AnyModel, declare_models};
    use crate::state_machine::input::{BindingError, DisplayMeta};
    use behavior_engine::builder::SemanticProfile;
    use behavior_engine::semantic::Module;
    use behavior_engine::store::documents::SeedEntity;
    use behavior_engine::store::{InMemoryBackend, Store};
    use behavior_engine::wire::WField;

    fn loc() -> Loc {
        Loc {
            file: "lower_tests.rs".into(),
            line: 1,
        }
    }

    struct Standard;

    impl ApprovalSource for Standard {
        fn trace_decision(
            &mut self,
            b: &mut Builder,
            _: &DecisionScope,
        ) -> Result<DecisionFragments, BindingError> {
            Ok(DecisionFragments {
                actor: (b.param("actor", loc()).unwrap(), loc()),
                open: None,
                eligible: None,
                guards: vec![],
                payload: vec![],
                commands: vec![],
            })
        }
    }

    fn input(k_a: i64, k_r: Option<i64>) -> ApprovalInput {
        let cap = |id: &str| CapabilityInput {
            capability_id: id.into(),
            inputs: vec![],
            context: vec![("actor".into(), WType::Id("User".into()))],
            display: DisplayMeta::default(),
            source: None,
        };
        ApprovalInput {
            approval_id: "doc_approval".into(),
            subject_entity: "Document".into(),
            actor_entity: "User".into(),
            approve_threshold: k_a,
            reject_threshold: k_r,
            payload: vec![],
            capabilities: vec![cap("approve"), cap("reject")],
            display: DisplayMeta::default(),
            source: None,
        }
    }

    /// The module with both decision actions lowered with `layout` under `profile` (no
    /// ownership or identity pass: those are not what these tests are about).
    fn module_in(profile: SemanticProfile, k_a: i64, k_r: Option<i64>, layout: Layout) -> Module {
        let mut b = Builder::with_semantic_profile(profile);
        let field = |n: &str| WField {
            name: n.into(),
            ty: WType::String,
            loc: loc(),
        };
        b.declare_entity("User", vec![field("name")], loc())
            .unwrap();
        b.declare_entity("Document", vec![field("title")], loc())
            .unwrap();
        let specs: Vec<AnyModel<'_>> =
            vec![AnyModel::Approval(crate::approval::input::ApprovalSpec {
                input: input(k_a, k_r),
                fragments: Box::new(Standard),
            })];
        declare_models(&mut b, &specs).unwrap();
        let base = b.finish(None).unwrap();
        let a = resolve(&input(k_a, k_r), &base).unwrap();
        for c in a.capabilities.values() {
            lower_with(&mut b, &a, c, &mut Standard, layout).unwrap();
        }
        b.finish(None).unwrap()
    }

    fn store(module: &Module, decisions: &[(&str, &str, &str)]) -> Store<InMemoryBackend> {
        let mut seed = vec![SeedEntity {
            entity: "Document".into(),
            value: json!({"id": "d1", "title": "d1"}),
        }];
        for u in ["u1", "u2", "u3", "u4"] {
            seed.push(SeedEntity {
                entity: "User".into(),
                value: json!({"id": u, "name": u}),
            });
        }
        for (id, user, choice) in decisions {
            seed.push(SeedEntity {
                entity: "DocApprovalDecision".into(),
                value: json!({"id": id, "subject": "d1", "actor": user, "choice": choice}),
            });
        }
        let genesis = if module.semantic_profile() == SemanticProfile::CommandIntents {
            let policy = behavior_engine::verify::governance::EvidencePolicyV2::from_json(
                r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#,
            )
            .unwrap();
            behavior_engine::store::store::genesis_v2_for(module, policy, seed).unwrap()
        } else {
            behavior_engine::store::store::genesis_for(
                module,
                behavior_engine::store::documents::EvidencePolicy::none(),
                seed,
            )
        };
        Store::create(InMemoryBackend::default(), module, genesis).unwrap()
    }

    /// The outcome of one decision attempt: Core's result, the decision record, and whether a
    /// commit happened (only an allowed candidate is committed, on the head it was evaluated
    /// against).
    struct Attempted {
        result: String,
        record: serde_json::Value,
        committed: bool,
    }

    /// Invokes `doc_approval__<capability>` by `actor` on `d1`, committing an allowed candidate.
    fn decide(
        store: &mut Store<InMemoryBackend>,
        module: &Module,
        capability: &str,
        actor: &str,
    ) -> Result<Attempted, String> {
        let document = json!({
            "format": "behavior.invocation.v1",
            "capability": format!("doc_approval__{capability}"),
            "bindings": {"subject": {"entity": "Document", "id": "d1"}},
            "input": {"decision_id": "new"},
            "context": {"actor": actor},
        });
        let (requested, _) =
            behavior_engine::invocation::RequestedInvocation::decode(&document.to_string())
                .unwrap();
        let inv = store
            .invoke(
                module,
                &requested.unwrap(),
                "2026-10-14T00:00:00Z",
                None,
                None,
            )
            .map_err(|e| format!("{e:?}"))?;
        let record = inv.record.as_json()["outcome"]["record"].clone();
        let result = record["result"].as_str().unwrap_or_default().to_string();
        let committed = match inv.bundle {
            Some(bundle) => {
                let head = store.current().unwrap();
                store
                    .commit(module, &head, &bundle)
                    .map_err(|e| format!("{e:?}"))?;
                true
            }
            None => false,
        };
        Ok(Attempted {
            result,
            record,
            committed,
        })
    }

    /// Whether a commit happened.
    fn attempt(
        store: &mut Store<InMemoryBackend>,
        module: &Module,
        capability: &str,
        actor: &str,
    ) -> Result<bool, String> {
        decide(store, module, capability, actor).map(|a| a.committed)
    }

    /// Core assumption (research R13): a decision action observes queries (`¬any(pair)`, the
    /// thresholds) and commits under both profiles. Core 0.12.0 refused it under the command
    /// profile (`INVALID_CANDIDATE: invalid observed facts`); Core 0.13.0 (023) accepts it.
    #[test]
    fn decisions_observing_queries_commit_under_both_profiles() {
        for profile in [SemanticProfile::Legacy, SemanticProfile::CommandIntents] {
            let m = module_in(profile, 2, Some(2), FULL);
            assert_eq!(
                attempt(&mut store(&m, &[]), &m, "approve", "u1"),
                Ok(true),
                "{profile:?}"
            );
        }
    }

    // --- T017: the child-state backstop (quickstart 6) ------------------------------------------

    /// Only the postconditions: the preconditions are omitted, so each case reaches the checks
    /// on the child state `S′ = Apply(S, CreateEntity(d))`.
    const POSTCONDITIONS_ONLY: Layout = Layout {
        preconditions: false,
        postconditions: true,
    };
    const NEITHER: Layout = Layout {
        preconditions: false,
        postconditions: false,
    };

    /// The position of the failed postcondition among the evaluated ones (the trace lists them
    /// in order and stops at the first failure), and Core's reason code.
    fn failed_postcondition(record: &serde_json::Value) -> Option<(usize, String)> {
        let posts: Vec<&serde_json::Value> = record["trace"]
            .as_array()?
            .iter()
            .filter(|step| step["phase"] == "postcondition")
            .collect();
        let i = posts
            .iter()
            .position(|step| step["outcome"] == json!(false))?;
        let code = record["reasons"][0]["code"].as_str()?.to_string();
        Some((i, code))
    }

    /// (a) The actor already decided: `Count_S(s, a) = 1 ≤ 1`, but `Count_S′ = 2`. Uniqueness on
    /// the child state denies the second decision; nothing is committed. Seeds are test fixtures
    /// (the base-state rule), not valid Approval states.
    #[test]
    fn a_second_decision_by_one_actor_is_denied_on_the_child_state() {
        let m = module_in(SemanticProfile::Legacy, 2, None, POSTCONDITIONS_ONLY);
        let mut s = store(&m, &[("x0", "u1", "APPROVE")]);
        let head = s.current().unwrap();
        let a = decide(&mut s, &m, "approve", "u1").unwrap();
        assert_eq!(a.result, "DENY", "{}", a.record);
        assert!(!a.committed);
        assert_eq!(
            failed_postcondition(&a.record),
            Some((0, "POSTCONDITION_FAILED".into())),
            "uniqueness, {}",
            a.record
        );
        assert_eq!(s.current().unwrap(), head, "head unchanged");
    }

    /// (b) `k_A = 2, k_R = 2` with `A = 2, R = 1`: consistency holds on `S` and fails only on `S′`
    /// after one more REJECT. It is denied; nothing is committed.
    #[test]
    fn a_decision_reaching_both_thresholds_is_denied_on_the_child_state() {
        let m = module_in(SemanticProfile::Legacy, 2, Some(2), POSTCONDITIONS_ONLY);
        let seeds = [
            ("x1", "u1", "APPROVE"),
            ("x2", "u2", "APPROVE"),
            ("x3", "u3", "REJECT"),
        ];
        let mut s = store(&m, &seeds);
        let head = s.current().unwrap();
        let a = decide(&mut s, &m, "reject", "u4").unwrap();
        assert_eq!(a.result, "DENY", "{}", a.record);
        assert!(!a.committed);
        assert_eq!(
            failed_postcondition(&a.record),
            Some((1, "POSTCONDITION_FAILED".into())),
            "threshold consistency, {}",
            a.record
        );
        assert_eq!(s.current().unwrap(), head, "head unchanged");
    }

    /// (c) Control: with the postconditions omitted as well, both invalid child states commit.
    /// So the postconditions are evaluated on the child state, and they are what stops (a) and (b).
    #[test]
    fn without_postconditions_both_invalid_child_states_commit() {
        let m = module_in(SemanticProfile::Legacy, 2, None, NEITHER);
        let mut s = store(&m, &[("x0", "u1", "APPROVE")]);
        let a = decide(&mut s, &m, "approve", "u1").unwrap();
        assert!(a.committed && a.result == "ALLOW", "{}", a.record);

        let m = module_in(SemanticProfile::Legacy, 2, Some(2), NEITHER);
        let seeds = [
            ("x1", "u1", "APPROVE"),
            ("x2", "u2", "APPROVE"),
            ("x3", "u3", "REJECT"),
        ];
        let mut s = store(&m, &seeds);
        let a = decide(&mut s, &m, "reject", "u4").unwrap();
        assert!(a.committed && a.result == "ALLOW", "{}", a.record);
    }
}
