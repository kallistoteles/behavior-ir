//! The WorkQueue's generated non-action items (data model §2): the `terminal` derived value
//! (added in P2 like Approval's definitions), the per-entity invariant `ValidWork` and the two
//! reads (added in P4 with the actions).
//!
//! ```text
//! <q>__terminal(w)     Rule  w.status in {SUCCEEDED, FAILED, CANCELLED}
//! <q>__valid_work(w)   0 ≤ attempts ≤ M
//!                      ∧ (status == LEASED) == (is_some(owner) ∧ is_some(lease_until))
//!                      ∧ (¬(status == READY) ∨ attempts < M)
//!                      ∧ (¬(status == LEASED) ∨ attempts ≥ 1)
//! <q>__ready(now)      project(where(select(E), w → status == READY ∧ available_at ≤ now ∧ attempts < M), [])
//! <q>__expired(now)    project(where(select(E), w → status == LEASED ∧ is_some(lease_until)
//!                                                 ∧ now ≥ value_or(lease_until, now)), [])
//! ```
//!
//! Every ordering against `lease_until` is guarded by an explicit `is_some`; the default of
//! `value_or` never decides an outcome (spec Q16, FR-023).

use std::collections::BTreeSet;

use behavior_engine::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_engine::wire::{DerivedKind, Loc, Role, WParam, WType};
use serde_json::json;

use crate::approval::resolve::{definition_name, location};
use crate::composition::DefinitionSpec;
use crate::error::ModelError;
use crate::ids::SemanticId;
use crate::work_queue::input::WorkQueueInput;
use crate::work_queue::resolve::{
    ATTEMPTS, AVAILABLE_AT, LEASE_UNTIL, NOW, OWNER, ResolvedWorkQueue, STATUS, TERMINAL,
    input_contributions, work_names,
};

/// The parameter of the derived value and the invariant, and the lambda member of the reads.
pub const ITEM: &str = "w";

/// The local ids of the generated items.
pub const TERMINAL_LOCAL: &str = "terminal";
pub const VALID_WORK_LOCAL: &str = "valid_work";
pub const READY_LOCAL: &str = "ready";
pub const EXPIRED_LOCAL: &str = "expired";

/// `w.status == lit(<E>Status, case)`
pub(crate) fn status_is(
    b: &mut Builder,
    param: &str,
    enum_name: &str,
    case: &str,
    loc: &Loc,
) -> Result<Node, BuildError> {
    let s = b.field(param, STATUS, loc.clone())?;
    let lit = b.lit(WType::Enum(enum_name.into()), json!(case), loc.clone())?;
    b.op("eq", vec![s, lit], loc.clone())
}

pub(crate) fn int(b: &mut Builder, v: i64, loc: &Loc) -> Result<Node, BuildError> {
    b.lit(WType::Int, json!(v), loc.clone())
}

/// `is_some(opt) ∧ op(left, value_or(opt, left))`, or `op(value_or(opt, right), right)` when
/// `opt_first`: an ordering against an optional deadline that is false when it is absent.
pub(crate) fn guarded_order(
    b: &mut Builder,
    opt: Node,
    other: Node,
    op: &str,
    opt_first: bool,
    loc: &Loc,
) -> Result<Node, BuildError> {
    let present = b.op("is_some", vec![opt.clone()], loc.clone())?;
    let value = b.op("value_or", vec![opt, other.clone()], loc.clone())?;
    let args = if opt_first {
        vec![value, other]
    } else {
        vec![other, value]
    };
    let ordered = b.op(op, args, loc.clone())?;
    b.op("and", vec![present, ordered], loc.clone())
}

/// `Defs(Q)`: the `terminal` derived value. Empty when the input is invalid; P2 reports those
/// errors.
pub fn definitions(input: &WorkQueueInput) -> Vec<DefinitionSpec> {
    if input_contributions(input).is_err() {
        return Vec::new();
    }
    let Some(q) = SemanticId::parse(&input.queue_id) else {
        return Vec::new();
    };
    let (_, entity) = work_names(&q);
    let loc = location(&input.source, &input.queue_id);
    let l = loc.clone();
    vec![DefinitionSpec {
        model_id: input.queue_id.clone(),
        local: TERMINAL_LOCAL.into(),
        name: definition_name(&q, TERMINAL_LOCAL),
        kind: DerivedKind::Rule,
        params: vec![WParam {
            name: ITEM.into(),
            role: None,
            ty: WType::Entity(entity),
        }],
        result: WType::Bool,
        deps: BTreeSet::new(),
        loc,
        body: Box::new(move |b: &mut Builder| {
            let s = b.field(ITEM, STATUS, l.clone())?;
            b.in_(s, TERMINAL.iter().map(|c| json!(c)).collect(), l.clone())
        }),
    }]
}

fn collision(model_ref: String, name: &str, e: BuildError) -> ModelError {
    if e.code == "DUPLICATE_NAME" {
        ModelError::GeneratedNameCollision {
            model_ref,
            name: name.to_string(),
        }
    } else {
        ModelError::FragmentInvalid {
            model_ref,
            core_code: e.code,
            core_message: e.message,
        }
    }
}

/// The body of `ValidWork(w)` in an open scope binding `w`.
fn valid_work_body(b: &mut Builder, q: &ResolvedWorkQueue, loc: &Loc) -> Result<Node, BuildError> {
    let e = &q.enum_name;
    let a = b.field(ITEM, ATTEMPTS, loc.clone())?;
    let zero = int(b, 0, loc)?;
    let one = int(b, 1, loc)?;
    let m = int(b, q.max_attempts, loc)?;
    let lower = b.op("ge", vec![a.clone(), zero], loc.clone())?;
    let upper = b.op("le", vec![a.clone(), m.clone()], loc.clone())?;
    let leased = status_is(b, ITEM, e, "LEASED", loc)?;
    let owner = b.field(ITEM, OWNER, loc.clone())?;
    let lease = b.field(ITEM, LEASE_UNTIL, loc.clone())?;
    let has_owner = b.op("is_some", vec![owner], loc.clone())?;
    let has_lease = b.op("is_some", vec![lease], loc.clone())?;
    let both = b.op("and", vec![has_owner, has_lease], loc.clone())?;
    let coherent = b.op("eq", vec![leased.clone(), both], loc.clone())?;
    let ready = status_is(b, ITEM, e, "READY", loc)?;
    let not_ready = b.op("not", vec![ready], loc.clone())?;
    let below = b.op("lt", vec![a.clone(), m], loc.clone())?;
    let ready_bound = b.op("or", vec![not_ready, below], loc.clone())?;
    let not_leased = b.op("not", vec![leased], loc.clone())?;
    let claimed = b.op("ge", vec![a, one], loc.clone())?;
    let leased_claimed = b.op("or", vec![not_leased, claimed], loc.clone())?;
    let mut body = lower;
    for part in [upper, coherent, ready_bound, leased_claimed] {
        body = b.op("and", vec![body, part], loc.clone())?;
    }
    Ok(body)
}

/// Adds `<q>__valid_work`, the per-entity invariant (FR-018).
pub fn add_invariant(b: &mut Builder, q: &ResolvedWorkQueue) -> Result<(), ModelError> {
    let name = definition_name(&q.queue_id, VALID_WORK_LOCAL);
    let model_ref = format!("{}/{VALID_WORK_LOCAL}", q.queue_id);
    let loc = location(&q.source, q.queue_id.as_str());
    let ps = vec![WParam {
        name: ITEM.into(),
        role: None,
        ty: WType::Entity(q.entity.clone()),
    }];
    b.push_scope(ScopeSite::Derived, ps, loc.clone())
        .map_err(|e| collision(model_ref.clone(), &name, e))?;
    // Added while its scope is open: Core checks that the body's bindings belong to it.
    let result = valid_work_body(b, q, &loc)
        .and_then(|body| b.add_invariant(&name, &q.entity, ITEM, body, loc.clone()));
    b.pop_scope();
    result.map_err(|e| collision(model_ref, &name, e))
}

/// Adds `<q>__ready(now)` and `<q>__expired(now)`, identity-only projections (FR-028).
pub fn add_reads(b: &mut Builder, q: &ResolvedWorkQueue) -> Result<(), ModelError> {
    for local in [READY_LOCAL, EXPIRED_LOCAL] {
        let name = definition_name(&q.queue_id, local);
        let model_ref = format!("{}/{local}", q.queue_id);
        let loc = location(&q.source, q.queue_id.as_str());
        let ps = vec![WParam {
            name: NOW.into(),
            role: Some(Role::Context),
            ty: q.time.clone(),
        }];
        b.push_scope(ScopeSite::Read, ps.clone(), loc.clone())
            .map_err(|e| collision(model_ref.clone(), &name, e))?;
        let query = read_query(b, q, local, &loc);
        b.pop_scope();
        let query = query.map_err(|e| collision(model_ref.clone(), &name, e))?;
        let read = Builder::read_projection(&name, ps, Ok(query), ITEM, vec![], loc);
        b.add_read(read)
            .map_err(|e| collision(model_ref, &name, e))?;
    }
    Ok(())
}

fn read_query(
    b: &mut Builder,
    q: &ResolvedWorkQueue,
    local: &str,
    loc: &Loc,
) -> Result<Node, BuildError> {
    let select = b.select(&q.entity, loc.clone())?;
    b.push_lambda(ITEM, &q.entity)?;
    let body = (|| {
        let now = b.param(NOW, loc.clone())?;
        if local == READY_LOCAL {
            let ready = status_is(b, ITEM, &q.enum_name, "READY", loc)?;
            let at = b.field(ITEM, AVAILABLE_AT, loc.clone())?;
            let due = b.op("le", vec![at, now], loc.clone())?;
            let a = b.field(ITEM, ATTEMPTS, loc.clone())?;
            let m = int(b, q.max_attempts, loc)?;
            let left = b.op("lt", vec![a, m], loc.clone())?;
            let both = b.op("and", vec![ready, due], loc.clone())?;
            b.op("and", vec![both, left], loc.clone())
        } else {
            let leased = status_is(b, ITEM, &q.enum_name, "LEASED", loc)?;
            let lease = b.field(ITEM, LEASE_UNTIL, loc.clone())?;
            let expired = guarded_order(b, lease, now, "ge", false, loc)?;
            b.op("and", vec![leased, expired], loc.clone())
        }
    })();
    b.pop_scope();
    b.lambda("where", select, ITEM, body?, loc.clone())
}
