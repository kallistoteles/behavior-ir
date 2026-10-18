//! Structural permission checking over every operation in the exact admitted module.
use super::{Inventory, SafetyError, inventory::Operation, resolve::ResolvedOwnership};

pub(crate) fn check(
    inventory: &Inventory,
    policies: &[ResolvedOwnership],
) -> Result<(), SafetyError> {
    for a in inventory.actions().values() {
        for op in &a.operations {
            for p in policies.iter().filter(|p| p.entity == op.entity()) {
                let (allowed, field, code) = match op {
                    Operation::Assign { field, .. } => {
                        let Some(writers) = p.writers.get(field) else {
                            continue;
                        };
                        (
                            writers.contains(&a.hash),
                            Some(field.clone()),
                            "SAFETY_UNAUTHORIZED_WRITE",
                        )
                    }
                    Operation::Create { .. } => (
                        p.creators.contains(&a.hash),
                        None,
                        "SAFETY_UNAUTHORIZED_CREATE",
                    ),
                    Operation::Remove { .. } => (
                        p.removers.contains(&a.hash),
                        None,
                        "SAFETY_UNAUTHORIZED_REMOVE",
                    ),
                };
                if !allowed {
                    let mut e = SafetyError::new(
                        code,
                        format!(
                            "Action `{}` cannot {} `{}{}` under policy `{}`",
                            a.name,
                            op.kind(),
                            p.entity,
                            field.as_ref().map(|f| format!(".{f}")).unwrap_or_default(),
                            p.author_id
                        ),
                    );
                    e.action = Some(a.name.clone());
                    e.entity = Some(p.entity.clone());
                    e.field = field;
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}
