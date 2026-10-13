//! RequestFlow's ownership rules over the admitted module (research R7). The status rules are
//! not here: the embedded machine is part of the internal composition set, so the existing
//! StateMachine pass enforces them unchanged.
//!
//! | operation on request type `E`      | allowed for                         |
//! |------------------------------------|-------------------------------------|
//! | create `E`                         | the flow's creation actions only    |
//! | write `E.requester`                | nobody (fixed by the initializer)   |
//! | write `E.assignee`, payload fields | any ordinary action                 |
//! | remove `E`                         | Core lifecycle (not a write)        |

use std::collections::BTreeSet;

use behavior_engine::semantic::Module;
use behavior_engine::wire::WType;

use crate::error::ModelError;
use crate::request_flow::resolve::{REQUESTER, ResolvedRequestFlow};

/// Every violation of the creation and requester rules in `module`, over all flows.
pub fn check(module: &Module, flows: &[ResolvedRequestFlow]) -> Vec<ModelError> {
    let mut errors = Vec::new();
    for f in flows {
        let creations: BTreeSet<&str> = f
            .creations
            .values()
            .map(|c| c.action_name.as_str())
            .collect();
        for (name, action) in module.actions() {
            for created in action.creates() {
                if created.entity() == f.entity && !creations.contains(name.as_str()) {
                    errors.push(ModelError::UnauthorizedRequestCreation {
                        action: name.clone(),
                        entity: f.entity.clone(),
                        owner: f.flow_id.to_string(),
                    });
                }
            }
            for effect in action.effects() {
                if effect.field() != REQUESTER {
                    continue;
                }
                let writes_request = action.params().iter().any(|p| {
                    p.name() == effect.param()
                        && p.ty().to_wire_type() == WType::Entity(f.entity.clone())
                });
                if writes_request {
                    errors.push(ModelError::UnauthorizedRequesterWrite {
                        action: name.clone(),
                        entity: f.entity.clone(),
                        owner: f.flow_id.to_string(),
                    });
                }
            }
        }
    }
    errors
}
