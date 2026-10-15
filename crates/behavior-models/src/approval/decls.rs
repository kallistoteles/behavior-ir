//! `Decl(AM) = { C, X }` and `Defs(AM)`, the five derived definitions (data model §3, §5).
//!
//! Everything here follows from the approval input alone (law D1): no lowering, no fragments. So
//! the binding can have the choice enum, the decision entity and the derived values declared in
//! P2, before any hand-written behavior refers to them.
//!
//! ```text
//! p__approve_count(s)  Derived Int   count(where(select(X), d → d.subject == s.id ∧ d.choice == APPROVE))
//! p__reject_count(s)   Derived Int   count(where(select(X), d → d.subject == s.id ∧ d.choice == REJECT))
//! p__approved(s)       Rule          approve_count(s) ≥ k_A  [∧ ¬(reject_count(s) ≥ k_R)]
//! p__rejected(s)       Rule          false  |  reject_count(s) ≥ k_R
//! p__pending(s)        Rule          ¬approved(s) ∧ ¬rejected(s)
//! ```
//!
//! REJECTED takes precedence (spec: Outcome): `approved` excludes `rejected` explicitly, so the
//! three predicates are mutually exclusive and exhaustive on every state, valid or not.

use std::collections::BTreeSet;

use behavior_engine::builder::{BuildError, Builder, Node};
use behavior_engine::wire::{DerivedKind, Loc, WParam, WType};
use serde_json::json;

use crate::approval::input::ApprovalInput;
use crate::approval::resolve::{
    CHOICE, SUBJECT, decision_names, definition_name, input_contributions, location,
};
use crate::composition::{Contribution, DefinitionSpec};
use crate::ids::SemanticId;

/// The fixed local ids of the five definitions, in their dependency order.
pub const DEFINITIONS: [&str; 5] = [
    "approve_count",
    "reject_count",
    "approved",
    "rejected",
    "pending",
];

/// The parameter of every definition: the subject, read-only.
pub const DEFINITION_PARAM: &str = "s";

/// `Needs(AM)`: every type the contributions reference that they do not declare themselves.
pub fn needs(input: &ApprovalInput) -> BTreeSet<String> {
    let Ok(contributions) = input_contributions(input) else {
        // Without contributions there is nothing to order; the errors are reported by P2.
        return BTreeSet::new();
    };
    let own: BTreeSet<&str> = contributions.iter().map(Contribution::name).collect();
    contributions
        .iter()
        .flat_map(Contribution::references)
        .filter(|n| !own.contains(n.as_str()))
        .collect()
}

/// Everything a definition body needs, owned, so the body can be rebuilt (law D4′).
#[derive(Debug, Clone)]
struct Facts {
    enum_name: String,
    entity: String,
    approve_count: String,
    reject_count: String,
    approved: String,
    rejected: String,
    k_a: u32,
    k_r: Option<u32>,
}

/// `count(where(select(X), d → d.subject == s.id ∧ d.choice == lit(C, choice)))`
fn count_body(b: &mut Builder, f: &Facts, choice: &str, loc: &Loc) -> Result<Node, BuildError> {
    let select = b.select(&f.entity, loc.clone())?;
    b.push_lambda("d", &f.entity)?;
    let body = (|| {
        let subject = b.field("d", SUBJECT, loc.clone())?;
        let id = b.field(DEFINITION_PARAM, "id", loc.clone())?;
        let same_subject = b.op("eq", vec![subject, id], loc.clone())?;
        let c = b.field("d", CHOICE, loc.clone())?;
        let lit = b.lit(WType::Enum(f.enum_name.clone()), json!(choice), loc.clone())?;
        let same_choice = b.op("eq", vec![c, lit], loc.clone())?;
        b.op("and", vec![same_subject, same_choice], loc.clone())
    })();
    b.pop_scope();
    let filtered = b.lambda("where", select, "d", body?, loc.clone())?;
    b.op("count", vec![filtered], loc.clone())
}

/// `name(s) ≥ k`
fn at_least(b: &mut Builder, name: &str, k: u32, loc: &Loc) -> Result<Node, BuildError> {
    let count = b.derived_ref(name, vec![DEFINITION_PARAM.into()], loc.clone())?;
    let k = b.lit(WType::Int, json!(k), loc.clone())?;
    b.op("ge", vec![count, k], loc.clone())
}

fn approved_body(b: &mut Builder, f: &Facts, loc: &Loc) -> Result<Node, BuildError> {
    let enough = at_least(b, &f.approve_count, f.k_a, loc)?;
    match f.k_r {
        None => Ok(enough),
        Some(k_r) => {
            let vetoed = at_least(b, &f.reject_count, k_r, loc)?;
            let not_vetoed = b.op("not", vec![vetoed], loc.clone())?;
            b.op("and", vec![enough, not_vetoed], loc.clone())
        }
    }
}

fn rejected_body(b: &mut Builder, f: &Facts, loc: &Loc) -> Result<Node, BuildError> {
    match f.k_r {
        None => b.lit(WType::Bool, json!(false), loc.clone()),
        Some(k_r) => at_least(b, &f.reject_count, k_r, loc),
    }
}

fn pending_body(b: &mut Builder, f: &Facts, loc: &Loc) -> Result<Node, BuildError> {
    let args = || vec![DEFINITION_PARAM.to_string()];
    let approved = b.derived_ref(&f.approved, args(), loc.clone())?;
    let rejected = b.derived_ref(&f.rejected, args(), loc.clone())?;
    let not_approved = b.op("not", vec![approved], loc.clone())?;
    let not_rejected = b.op("not", vec![rejected], loc.clone())?;
    b.op("and", vec![not_approved, not_rejected], loc.clone())
}

/// `Defs(AM)`: the five definitions with their signatures and dependencies (data model §5).
/// Empty when the input is invalid; P2 reports those errors.
pub fn definitions(input: &ApprovalInput) -> Vec<DefinitionSpec> {
    if input_contributions(input).is_err() {
        return Vec::new();
    }
    let Some(p) = SemanticId::parse(&input.approval_id) else {
        return Vec::new();
    };
    let (enum_name, entity) = decision_names(&p);
    let name = |local: &str| definition_name(&p, local);
    let facts = Facts {
        enum_name,
        entity,
        approve_count: name("approve_count"),
        reject_count: name("reject_count"),
        approved: name("approved"),
        rejected: name("rejected"),
        k_a: u32::try_from(input.approve_threshold).unwrap_or(1),
        k_r: input
            .reject_threshold
            .map(|k| u32::try_from(k).unwrap_or(1)),
    };
    let loc = location(&input.source, &input.approval_id);
    let params = vec![WParam {
        name: DEFINITION_PARAM.into(),
        role: None,
        ty: WType::Entity(input.subject_entity.clone()),
    }];
    let deps =
        |names: &[&String]| -> BTreeSet<String> { names.iter().map(|n| (*n).clone()).collect() };
    let approved_deps = match facts.k_r {
        None => deps(&[&facts.approve_count]),
        Some(_) => deps(&[&facts.approve_count, &facts.reject_count]),
    };
    let rejected_deps = match facts.k_r {
        None => BTreeSet::new(),
        Some(_) => deps(&[&facts.reject_count]),
    };
    let spec = |local: &str,
                kind: DerivedKind,
                result: WType,
                deps: BTreeSet<String>,
                body: fn(&mut Builder, &Facts, &Loc) -> Result<Node, BuildError>| {
        let f = facts.clone();
        let l = loc.clone();
        DefinitionSpec {
            model_id: input.approval_id.clone(),
            local: local.to_string(),
            name: name(local),
            kind,
            params: params.clone(),
            result,
            deps,
            loc: loc.clone(),
            body: Box::new(move |b: &mut Builder| body(b, &f, &l)),
        }
    };
    vec![
        spec(
            "approve_count",
            DerivedKind::Derived,
            WType::Int,
            BTreeSet::new(),
            |b, f, l| count_body(b, f, "APPROVE", l),
        ),
        spec(
            "reject_count",
            DerivedKind::Derived,
            WType::Int,
            BTreeSet::new(),
            |b, f, l| count_body(b, f, "REJECT", l),
        ),
        spec(
            "approved",
            DerivedKind::Rule,
            WType::Bool,
            approved_deps,
            approved_body,
        ),
        spec(
            "rejected",
            DerivedKind::Rule,
            WType::Bool,
            rejected_deps,
            rejected_body,
        ),
        spec(
            "pending",
            DerivedKind::Rule,
            WType::Bool,
            deps(&[&facts.approved, &facts.rejected]),
            pending_body,
        ),
    ]
}
