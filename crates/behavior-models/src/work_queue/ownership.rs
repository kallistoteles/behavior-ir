//! WorkQueue's ownership rules over the admitted module (research R4, spec FR-019).
//!
//! | item of work entity `E`  | allowed writers (through their subject parameter)            |
//! |--------------------------|-------------------------------------------------------------|
//! | creation of `E`          | `<q>__enqueue`                                              |
//! | removal of `E`           | nobody                                                      |
//! | payload fields           | nobody after creation (enqueue sets them at creation)       |
//! | `attempts`               | claim                                                       |
//! | `owner`                  | claim, complete, retry, fail, recover, exhaust, cancel      |
//! | `lease_until`            | claim, renew, complete, retry, fail, recover, exhaust, cancel |
//! | `available_at`           | retry, recover                                              |
//! | `status`                 | the embedded StateMachine (its own final pass)              |
//!
//! Completeness is Approval's argument (`approval/ownership.rs`): an admitted action changes
//! state only through `effects()`, `creates()` and `removes()`, and effects and removals reach
//! `E` only through a parameter of type `Entity(E)`. Every parameter of every action of the final
//! module is checked, the queue's own included, with no exemption by origin; an allowed writer
//! passes only for the field its row names and only through its subject parameter. A binding
//! never grants authority (D1(a)).

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::semantic::Module;
use behavior_engine::wire::WType;

use crate::error::ModelError;
use crate::work_queue::resolve::{
    ATTEMPTS, AVAILABLE_AT, LEASE_UNTIL, OWNER, ResolvedWorkQueue, STATUS,
};

/// The capabilities that may write `field` (research R4); `status` is the machine's.
fn writers(field: &str) -> &'static [&'static str] {
    match field {
        ATTEMPTS => &["claim"],
        OWNER => &[
            "claim", "complete", "retry", "fail", "recover", "exhaust", "cancel",
        ],
        LEASE_UNTIL => &[
            "claim", "renew", "complete", "retry", "fail", "recover", "exhaust", "cancel",
        ],
        AVAILABLE_AT => &["retry", "recover"],
        _ => &[],
    }
}

/// Every violation of the work rules in `module`, over all queues.
pub fn check(module: &Module, queues: &[ResolvedWorkQueue]) -> Vec<ModelError> {
    let mut errors = Vec::new();
    for q in queues {
        let owner = q.queue_id.to_string();
        let work = WType::Entity(q.entity.clone());
        let enqueue = q
            .capabilities
            .get("enqueue")
            .map(|c| c.action_name.as_str());
        // action name → capability id, for the queue's own actions.
        let own: BTreeMap<&str, &str> = q
            .capabilities
            .iter()
            .map(|(id, c)| (c.action_name.as_str(), id.as_str()))
            .collect();
        let subject = q.subject_param();
        for (name, action) in module.actions() {
            let work_params: BTreeSet<&str> = action
                .params()
                .iter()
                .filter(|p| p.ty().to_wire_type() == work)
                .map(|p| p.name())
                .collect();
            for created in action.creates() {
                if created.entity() == q.entity && Some(name.as_str()) != enqueue {
                    errors.push(ModelError::UnauthorizedWorkCreation {
                        action: name.clone(),
                        entity: q.entity.clone(),
                        owner: owner.clone(),
                    });
                }
            }
            for effect in action.effects() {
                if !work_params.contains(effect.param()) || effect.field() == STATUS {
                    continue;
                }
                let allowed = effect.param() == subject
                    && own
                        .get(name.as_str())
                        .is_some_and(|c| writers(effect.field()).contains(c));
                if !allowed {
                    errors.push(ModelError::UnauthorizedWorkWrite {
                        action: name.clone(),
                        entity: q.entity.clone(),
                        field: effect.field().to_string(),
                        owner: owner.clone(),
                    });
                }
            }
            for removal in action.removes() {
                if work_params.contains(removal.param()) {
                    errors.push(ModelError::UnauthorizedWorkRemoval {
                        action: name.clone(),
                        entity: q.entity.clone(),
                        owner: owner.clone(),
                    });
                }
            }
        }
    }
    errors
}
