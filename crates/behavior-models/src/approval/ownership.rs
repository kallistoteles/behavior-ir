//! Approval's ownership rules over the admitted module (data model §7).
//!
//! | operation on decision type `X_p` | allowed for                                  |
//! |----------------------------------|----------------------------------------------|
//! | create `X_p`                     | `encode(p, approve)`, `encode(p, reject)`    |
//! | write any field of `X_p`         | nobody, including the approval's own actions |
//! | remove `X_p`                     | nobody (ordinary behavior)                   |
//! | read, query, derived, reads      | anyone                                       |
//!
//! **Completeness (the premise of the induction, research R5).** In an admitted Core module an
//! action changes state only through the three parts of `ActionItem`: `effects()`, which write
//! fields of a bound State parameter; `creates()`, which create entities; and `removes()`, which
//! remove a bound State parameter's entity. It has no other state-changing part, and Core has no
//! cascading removal. Reads, derived values, invariants, constraints and command emissions
//! change nothing. A decision's `subject` and `actor` are `Id`, never `Ref`, so removing a
//! subject or an actor touches no decision.
//!
//! So checking those three parts of **every** action of the final module, with no exemption by
//! origin (hand-written actions, StateMachine transitions, RequestFlow creations and transitions,
//! other approvals' actions), covers every path by which ordinary behavior can change `X_p`.
//! Effects and removals reach `X_p` only through a parameter whose type is `Entity(X_p)`, which is
//! resolved through `params()` as 502's requester rule does. With this check, the decision set
//! `D` changes only by one creation of one of `p`'s two actions, whose preconditions and
//! postconditions preserve the approval's invariant `P`. Migrations, genesis seeds and imports
//! are outside ordinary behavior: they must establish `P` themselves (the base-state rule).
//!
//! The approval owns nothing on its subject or actor type, and `C` has no rule: an enum has no
//! state.

use std::collections::BTreeSet;

use behavior_engine::semantic::Module;
use behavior_engine::wire::WType;

use crate::approval::resolve::ResolvedApproval;
use crate::error::ModelError;

/// Every violation of the decision rules in `module`, over all approvals.
pub fn check(module: &Module, approvals: &[ResolvedApproval]) -> Vec<ModelError> {
    let mut errors = Vec::new();
    for a in approvals {
        let owner = a.approval_id.to_string();
        let creators: BTreeSet<&str> = a
            .capabilities
            .values()
            .map(|c| c.action_name.as_str())
            .collect();
        let decision = WType::Entity(a.entity.clone());
        for (name, action) in module.actions() {
            let binds_decision = |param: &str| {
                action
                    .params()
                    .iter()
                    .any(|p| p.name() == param && p.ty().to_wire_type() == decision)
            };
            for created in action.creates() {
                if created.entity() == a.entity && !creators.contains(name.as_str()) {
                    errors.push(ModelError::UnauthorizedDecisionCreation {
                        action: name.clone(),
                        entity: a.entity.clone(),
                        owner: owner.clone(),
                    });
                }
            }
            for effect in action.effects() {
                if binds_decision(effect.param()) {
                    errors.push(ModelError::UnauthorizedDecisionWrite {
                        action: name.clone(),
                        entity: a.entity.clone(),
                        field: effect.field().to_string(),
                        owner: owner.clone(),
                    });
                }
            }
            for removal in action.removes() {
                if binds_decision(removal.param()) {
                    errors.push(ModelError::UnauthorizedDecisionRemoval {
                        action: name.clone(),
                        entity: a.entity.clone(),
                        owner: owner.clone(),
                    });
                }
            }
        }
    }
    errors
}
