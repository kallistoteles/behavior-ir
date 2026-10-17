//! WorkQueue lowering (data model §3, lowerer version 1).
//!
//! Enqueue is a creation, lowered here (like a RequestFlow creation):
//!
//! ```text
//! action   <q>__enqueue
//! params   id: Input Id(E), inputs (available_at, payload, authored), state bindings, context
//! pre      authorization ++ G
//! create   E { id, status: READY, attempts: 0, owner: none, lease_until: none, available_at, payload }
//! effects  authored, over the state bindings
//! post     authored          commands  authored
//! ```
//!
//! The eight transitions are lowered by the unchanged StateMachine lowerer. [`QueueSource`] is
//! the fragment source it calls: it places the generated requirements, effects and
//! postconditions of data model §3 first, then the authorization guard, then the author's parts.
//! The machine adds the status requirement, effect and postcondition in front of all of them.
//!
//! This file and `request_flow/create.rs` are the only files of the crate that may call
//! `into_wire`: the builder's creation input type takes wire expressions.

use behavior_engine::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_engine::wire::{Loc, WLifecycle, WType};
use serde_json::json;

use crate::composition::admission_failed;
use crate::error::ModelError;
use crate::state_machine::input::{
    BindingError, FragmentSource, TransitionFragments, TransitionScope,
};
use crate::work_queue::decls::{guarded_order, int};
use crate::work_queue::input::{CapabilityFragments, CapabilityScope, WorkQueueSource};
use crate::work_queue::resolve::{
    ATTEMPTS, AVAILABLE_AT, EXPECTED_GENERATION, GENERATION, ID, LEASE_UNTIL, NOW, OWNER,
    ResolvedCapability, ResolvedWorkQueue, STATUS, WORKER, capability_loc,
};

/// The binding-error code by which [`QueueSource`] stops the StateMachine lowerer after it has
/// recorded a typed WorkQueue error of its own.
pub const QUEUE_RULE: &str = "WORK_QUEUE_RULE";

fn fragment_error(model_ref: &str, e: BuildError) -> ModelError {
    ModelError::FragmentInvalid {
        model_ref: model_ref.to_string(),
        core_code: e.code,
        core_message: e.message,
    }
}

/// The generated parts of transition `c` (data model §3), in the open scope whose subject
/// parameter is `s`.
fn generated(
    b: &mut Builder,
    q: &ResolvedWorkQueue,
    c: &str,
    s: &str,
    loc: &Loc,
) -> Result<TransitionFragments, BuildError> {
    let l = || loc.clone();
    let field = |b: &mut Builder, f: &str| b.field(s, f, l());
    let none = |b: &mut Builder, t: WType| b.lit(WType::Option(Box::new(t)), json!(null), l());
    let m = q.max_attempts;
    let attempts = field(b, ATTEMPTS)?;
    // owner == worker ∧ attempts == generation ∧ is_some(lease_until) ∧ now < lease_until
    let current = |b: &mut Builder, attempts: Node| -> Result<Vec<Node>, BuildError> {
        let owner = field(b, OWNER)?;
        let worker = b.param(WORKER, l())?;
        let generation = b.param(GENERATION, l())?;
        let lease = field(b, LEASE_UNTIL)?;
        let now = b.param(NOW, l())?;
        Ok(vec![
            b.op("eq", vec![owner, worker], l())?,
            b.op("eq", vec![attempts, generation], l())?,
            guarded_order(b, lease, now, "lt", false, loc)?,
        ])
    };
    // is_some(lease_until) ∧ now ≥ lease_until
    let expired = |b: &mut Builder| -> Result<Node, BuildError> {
        let lease = field(b, LEASE_UNTIL)?;
        let now = b.param(NOW, l())?;
        guarded_order(b, lease, now, "ge", false, loc)
    };
    let below_max = |b: &mut Builder, attempts: Node| -> Result<Node, BuildError> {
        let max = int(b, m, loc)?;
        b.op("lt", vec![attempts, max], l())
    };
    // available_at(param) ≥ now
    let not_earlier = |b: &mut Builder| -> Result<Node, BuildError> {
        let at = b.param(AVAILABLE_AT, l())?;
        let now = b.param(NOW, l())?;
        b.op("ge", vec![at, now], l())
    };
    let release = |b: &mut Builder| -> Result<Vec<(String, String, Node, Loc)>, BuildError> {
        let worker_ty = WType::Id(q.worker.clone());
        Ok(vec![
            (s.into(), OWNER.into(), none(b, worker_ty)?, l()),
            (s.into(), LEASE_UNTIL.into(), none(b, q.time.clone())?, l()),
        ])
    };
    let same_generation = |b: &mut Builder, attempts: Node| -> Result<Node, BuildError> {
        let generation = b.param(GENERATION, l())?;
        b.op("eq", vec![attempts, generation], l())
    };

    let mut f = TransitionFragments::default();
    match c {
        "claim" => {
            let at = field(b, AVAILABLE_AT)?;
            let now = b.param(NOW, l())?;
            let expected = b.param(EXPECTED_GENERATION, l())?;
            let until = b.param(LEASE_UNTIL, l())?;
            let worker = b.param(WORKER, l())?;
            let one = int(b, 1, loc)?;
            f.guards = vec![
                (b.op("le", vec![at, now.clone()], l())?, l()),
                (below_max(b, attempts.clone())?, l()),
                (
                    b.op("eq", vec![attempts.clone(), expected.clone()], l())?,
                    l(),
                ),
                (b.op("gt", vec![until.clone(), now], l())?, l()),
            ];
            let next = b.op("add", vec![attempts.clone(), one.clone()], l())?;
            f.effects = vec![
                (s.into(), ATTEMPTS.into(), next, l()),
                (s.into(), OWNER.into(), worker, l()),
                (s.into(), LEASE_UNTIL.into(), until, l()),
            ];
            let expected_next = b.op("add", vec![expected, one], l())?;
            f.postconditions = vec![(b.op("eq", vec![attempts, expected_next], l())?, l())];
        }
        "renew" => {
            let mut guards = current(b, attempts.clone())?;
            let old = field(b, LEASE_UNTIL)?;
            let until = b.param(LEASE_UNTIL, l())?;
            guards.push(guarded_order(b, old, until.clone(), "lt", true, loc)?);
            f.guards = guards.into_iter().map(|g| (g, l())).collect();
            f.effects = vec![(s.into(), LEASE_UNTIL.into(), until, l())];
            f.postconditions = vec![(same_generation(b, attempts)?, l())];
        }
        "complete" | "fail" => {
            f.guards = current(b, attempts.clone())?
                .into_iter()
                .map(|g| (g, l()))
                .collect();
            f.effects = release(b)?;
            f.postconditions = vec![(same_generation(b, attempts)?, l())];
        }
        "retry" => {
            let mut guards = current(b, attempts.clone())?;
            guards.push(below_max(b, attempts.clone())?);
            guards.push(not_earlier(b)?);
            f.guards = guards.into_iter().map(|g| (g, l())).collect();
            let mut effects = release(b)?;
            effects.push((
                s.into(),
                AVAILABLE_AT.into(),
                b.param(AVAILABLE_AT, l())?,
                l(),
            ));
            f.effects = effects;
            f.postconditions = vec![(same_generation(b, attempts)?, l())];
        }
        "recover" => {
            f.guards = vec![
                (expired(b)?, l()),
                (below_max(b, attempts)?, l()),
                (not_earlier(b)?, l()),
            ];
            let mut effects = release(b)?;
            effects.push((
                s.into(),
                AVAILABLE_AT.into(),
                b.param(AVAILABLE_AT, l())?,
                l(),
            ));
            f.effects = effects;
        }
        "exhaust" => {
            let max = int(b, m, loc)?;
            f.guards = vec![
                (expired(b)?, l()),
                (b.op("eq", vec![attempts, max], l())?, l()),
            ];
            f.effects = release(b)?;
        }
        "cancel" => {
            f.effects = release(b)?;
        }
        _ => {}
    }
    Ok(f)
}

/// The fragment source of the embedded machine: generated parts, then the author's.
pub struct QueueSource<'a, 's> {
    pub queue: &'a ResolvedWorkQueue,
    pub inner: &'a mut (dyn WorkQueueSource + 's),
    /// Typed WorkQueue errors found while tracing; the caller reports these and drops the
    /// `QUEUE_RULE` fragment errors they cause.
    pub errors: Vec<ModelError>,
}

impl QueueSource<'_, '_> {
    fn stop(&mut self, e: ModelError) -> BindingError {
        self.errors.push(e);
        BindingError {
            code: QUEUE_RULE.into(),
            message: "see the typed WorkQueue error".into(),
        }
    }
}

impl FragmentSource for QueueSource<'_, '_> {
    fn trace(
        &mut self,
        b: &mut Builder,
        scope: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError> {
        let q = self.queue;
        let c_ref = q.capability_ref(&scope.transition_id);
        let Some(c) = q.capabilities.get(&scope.transition_id) else {
            return Ok(TransitionFragments::default());
        };
        let loc = capability_loc(q, c);
        let gen_parts = match generated(b, q, &scope.transition_id, &scope.subject_param, &loc) {
            Ok(f) => f,
            Err(e) if e.code == "OP_NOT_ALLOWED" => {
                return Err(self.stop(ModelError::TimeNotOrdered {
                    model_ref: q.queue_id.to_string(),
                    time: crate::work_queue::resolve::type_name(&q.time),
                }));
            }
            Err(e) => return Err(self.stop(fragment_error(&c_ref, e))),
        };
        let authored = self.inner.trace_capability(
            b,
            &CapabilityScope {
                queue_id: q.queue_id.to_string(),
                capability_id: scope.transition_id.clone(),
                params: scope.params.clone(),
            },
        )?;
        let authored = match check_authored(q, c, &c_ref, Some(&scope.subject_param), authored) {
            Ok(a) => a,
            Err(e) => return Err(self.stop(e)),
        };
        let CapabilityFragments {
            authorization,
            guards,
            effects,
            postconditions,
            commands,
        } = authored;
        Ok(TransitionFragments {
            guards: gen_parts
                .guards
                .into_iter()
                .chain(authorization)
                .chain(guards)
                .collect(),
            effects: gen_parts.effects.into_iter().chain(effects).collect(),
            postconditions: gen_parts
                .postconditions
                .into_iter()
                .chain(postconditions)
                .collect(),
            commands,
        })
    }
}

/// The WorkQueue rules on the author's parts: a mandatory authorization is present, and no
/// authored effect writes the work item (FR-020, FR-021).
fn check_authored(
    q: &ResolvedWorkQueue,
    c: &ResolvedCapability,
    c_ref: &str,
    subject: Option<&str>,
    mut f: CapabilityFragments,
) -> Result<CapabilityFragments, ModelError> {
    if c.needs_authorization && f.authorization.is_none() {
        return Err(ModelError::MissingAuthorization {
            model_ref: c_ref.to_string(),
        });
    }
    if !c.needs_authorization
        && let Some(a) = f.authorization.take()
    {
        f.guards.insert(0, a);
    }
    let _ = q;
    if let Some((_, field, _, _)) = f
        .effects
        .iter()
        .find(|(param, _, _, _)| Some(param.as_str()) == subject || param == ID)
    {
        return Err(ModelError::AuthoredWorkEffect {
            model_ref: c_ref.to_string(),
            field: field.clone(),
        });
    }
    Ok(f)
}

/// Lowers enqueue into `builder`. The scope is always popped again.
pub fn lower_enqueue(
    builder: &mut Builder,
    q: &ResolvedWorkQueue,
    source: &mut dyn WorkQueueSource,
) -> Result<(), Vec<ModelError>> {
    let Some(c) = q.capabilities.get("enqueue") else {
        return Ok(());
    };
    let c_ref = q.capability_ref("enqueue");
    let params = q.enqueue_params();
    let loc = capability_loc(q, c);
    builder
        .push_scope(ScopeSite::Action, params.clone(), loc.clone())
        .map_err(|e| vec![fragment_error(&c_ref, e)])?;
    let result = enqueue_in_scope(builder, q, c, &c_ref, params, loc, source);
    builder.pop_scope();
    result
}

fn enqueue_in_scope(
    builder: &mut Builder,
    q: &ResolvedWorkQueue,
    c: &ResolvedCapability,
    c_ref: &str,
    params: Vec<behavior_engine::wire::WParam>,
    loc: Loc,
    source: &mut dyn WorkQueueSource,
) -> Result<(), Vec<ModelError>> {
    let scope = CapabilityScope {
        queue_id: q.queue_id.to_string(),
        capability_id: "enqueue".into(),
        params: params.clone(),
    };
    let authored = source.trace_capability(builder, &scope).map_err(|e| {
        vec![ModelError::FragmentInvalid {
            model_ref: c_ref.to_string(),
            core_code: e.code,
            core_message: e.message,
        }]
    })?;
    let authored = check_authored(q, c, c_ref, None, authored).map_err(|e| vec![e])?;
    let mut errors = Vec::new();
    for (node, _) in authored
        .authorization
        .iter()
        .chain(&authored.guards)
        .chain(&authored.postconditions)
    {
        if let Err(e) = builder.check_condition(node, "an enqueue condition") {
            errors.push(fragment_error(c_ref, e));
        }
    }
    for (param, field, node, _) in &authored.effects {
        if let Err(e) = builder.check_effect(param, field, node) {
            errors.push(fragment_error(c_ref, e));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let CapabilityFragments {
        authorization,
        guards,
        effects,
        postconditions,
        commands,
    } = authored;
    let err = |e| vec![fragment_error(c_ref, e)];
    let id = builder.param(ID, loc.clone()).map_err(err)?;
    let mut values: Vec<(String, Node)> = vec![
        (
            STATUS.into(),
            builder
                .lit(
                    WType::Enum(q.enum_name.clone()),
                    json!("READY"),
                    loc.clone(),
                )
                .map_err(err)?,
        ),
        (ATTEMPTS.into(), int(builder, 0, &loc).map_err(err)?),
        (
            OWNER.into(),
            builder
                .lit(
                    WType::Option(Box::new(WType::Id(q.worker.clone()))),
                    json!(null),
                    loc.clone(),
                )
                .map_err(err)?,
        ),
        (
            LEASE_UNTIL.into(),
            builder
                .lit(
                    WType::Option(Box::new(q.time.clone())),
                    json!(null),
                    loc.clone(),
                )
                .map_err(err)?,
        ),
        (
            AVAILABLE_AT.into(),
            builder.param(AVAILABLE_AT, loc.clone()).map_err(err)?,
        ),
    ];
    for name in q.payload.keys() {
        values.push((name.clone(), builder.param(name, loc.clone()).map_err(err)?));
    }
    values.sort_by(|a, b| a.0.cmp(&b.0));
    let create = WLifecycle::Create {
        entity: q.entity.clone(),
        id: id.into_wire(),
        fields: values
            .into_iter()
            .map(|(n, v)| (n, v.into_wire()))
            .collect(),
        loc: loc.clone(),
    };
    let pre: Vec<(Node, Loc)> = authorization.into_iter().chain(guards).collect();
    let added = if commands.is_empty() {
        builder.add_action(
            &c.action_name,
            params,
            pre,
            effects,
            vec![create],
            postconditions,
            loc,
        )
    } else {
        builder.add_action_with_commands(
            &c.action_name,
            params,
            pre,
            effects,
            vec![create],
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
    // The creation's values reach the builder as wire expressions; admit now so an ill-typed
    // value is reported against enqueue.
    builder.finish(None).map(|_| ()).map_err(|r| {
        vec![match admission_failed(r) {
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
