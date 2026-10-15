//! Approval resolution (feature 503, T014): one test per data model §3 rule, and goldens of the
//! decision schema (§3) and of the decision capability parameters (§4).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::semantic::Module;
use behavior_engine::wire::{Role, WType};
use behavior_models::approval::resolve::{ResolvedApproval, resolve};
use behavior_models::{ApprovalInput, ModelError};
use common::ap::*;
use common::codes;

/// The declaration environment: `User`, `Document` (no models).
fn environment() -> Module {
    authored().finish(None).unwrap()
}

fn resolved(input: &ApprovalInput) -> Result<ResolvedApproval, Vec<ModelError>> {
    resolve(input, &environment())
}

fn refused(input: &ApprovalInput) -> Vec<ModelError> {
    resolved(input).expect_err("resolution must refuse")
}

/// The single error, with its code and the name it carries.
fn only(errors: &[ModelError]) -> (&'static str, String) {
    assert_eq!(errors.len(), 1, "{errors:#?}");
    let named = match &errors[0] {
        ModelError::InvalidSemanticId { id, .. } => id.clone(),
        ModelError::UnknownEntity { entity, .. } => entity.clone(),
        ModelError::InvalidThreshold { which, .. } => which.clone(),
        ModelError::ReservedField { name, .. } | ModelError::InvalidParameter { name, .. } => {
            name.clone()
        }
        other => format!("{other:?}"),
    };
    (errors[0].code(), named)
}

#[test]
fn an_approval_id_that_is_not_a_semantic_id_is_refused() {
    let mut input = approval(1, None);
    input.approval_id = "Doc-Approval".into();
    assert_eq!(
        only(&refused(&input)),
        ("INVALID_SEMANTIC_ID", "Doc-Approval".into())
    );
}

#[test]
fn an_unknown_subject_entity_is_refused() {
    let mut input = approval(1, None);
    input.subject_entity = "Contract".into();
    assert_eq!(
        only(&refused(&input)),
        ("UNKNOWN_ENTITY", "Contract".into())
    );
}

#[test]
fn an_unknown_actor_entity_is_refused() {
    let mut input = approval(1, None);
    input.actor_entity = "Employee".into();
    assert_eq!(
        only(&refused(&input)),
        ("UNKNOWN_ENTITY", "Employee".into())
    );
}

#[test]
fn a_zero_approve_threshold_is_refused() {
    let errors = refused(&approval(0, None));
    assert_eq!(only(&errors), ("INVALID_THRESHOLD", "approve".into()));
    assert!(matches!(
        errors[0],
        ModelError::InvalidThreshold { value: 0, .. }
    ));
}

#[test]
fn a_zero_reject_threshold_is_refused() {
    let errors = refused(&approval(1, Some(0)));
    assert_eq!(only(&errors), ("INVALID_THRESHOLD", "reject".into()));
}

#[test]
fn negative_thresholds_are_refused() {
    assert_eq!(
        codes(&refused(&approval(-1, Some(-2)))),
        vec!["INVALID_THRESHOLD", "INVALID_THRESHOLD"]
    );
}

#[test]
fn reserved_payload_fields_are_refused() {
    for name in ["subject", "actor", "choice", "id"] {
        let mut input = approval(1, None);
        input.payload = vec![(name.into(), WType::String)];
        assert_eq!(
            only(&refused(&input)),
            ("RESERVED_FIELD", name.into()),
            "{name}"
        );
    }
}

#[test]
fn a_duplicate_payload_field_is_refused() {
    let mut input = approval(1, None);
    input.payload = vec![
        ("comment".into(), WType::String),
        ("comment".into(), WType::Int),
    ];
    assert_eq!(
        only(&refused(&input)),
        ("INVALID_PARAMETER", "comment".into())
    );
}

#[test]
fn a_payload_field_that_is_not_an_identifier_is_refused() {
    let mut input = approval(1, None);
    input.payload = vec![("due date".into(), WType::String)];
    assert_eq!(
        only(&refused(&input)),
        ("INVALID_PARAMETER", "due date".into())
    );
}

#[test]
fn capability_parameters_named_subject_or_decision_id_are_refused() {
    for name in ["subject", "decision_id"] {
        for in_context in [false, true] {
            let mut input = approval(1, None);
            let c = &mut input.capabilities[0];
            if in_context {
                c.context.push((name.into(), WType::String));
            } else {
                c.inputs.push((name.into(), WType::String));
            }
            let errors = refused(&input);
            assert_eq!(only(&errors), ("INVALID_PARAMETER", name.into()), "{name}");
            assert!(
                matches!(&errors[0], ModelError::InvalidParameter { model_ref, .. }
                    if model_ref == "doc_approval/capability/approve"),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn a_parameter_name_used_twice_in_a_capability_is_refused() {
    let mut input = approval(1, None);
    input.capabilities[1]
        .inputs
        .push(("actor".into(), WType::Id("User".into())));
    assert_eq!(
        only(&refused(&input)),
        ("INVALID_PARAMETER", "actor".into())
    );
}

#[test]
fn a_missing_capability_is_refused() {
    let mut input = approval(1, None);
    input.capabilities.retain(|c| c.capability_id != "reject");
    assert_eq!(
        only(&refused(&input)),
        ("INVALID_PARAMETER", "reject".into())
    );
}

#[test]
fn an_extra_capability_is_refused() {
    let mut input = approval(1, None);
    input.capabilities.push(capability("abstain"));
    assert_eq!(
        only(&refused(&input)),
        ("INVALID_PARAMETER", "abstain".into())
    );
}

#[test]
fn a_capability_supplied_twice_is_refused() {
    let mut input = approval(1, None);
    input.capabilities.push(capability("approve"));
    assert_eq!(
        only(&refused(&input)),
        ("INVALID_PARAMETER", "approve".into())
    );
}

#[test]
fn every_error_is_reported_together() {
    let mut input = approval(0, Some(0));
    input.subject_entity = "Contract".into();
    input.payload = vec![("choice".into(), WType::String)];
    let mut got = codes(&refused(&input));
    got.sort_unstable();
    assert_eq!(
        got,
        vec![
            "INVALID_THRESHOLD",
            "INVALID_THRESHOLD",
            "RESERVED_FIELD",
            "UNKNOWN_ENTITY"
        ]
    );
}

/// Golden (data model §3): the payload's declaration order never reaches the schema; `subject`
/// and `actor` are identities, never references.
#[test]
fn the_decision_schema_is_sorted_with_identities() {
    for payload in [
        vec![
            ("comment".to_string(), WType::String),
            ("amount".to_string(), WType::Int),
        ],
        vec![
            ("amount".to_string(), WType::Int),
            ("comment".to_string(), WType::String),
        ],
    ] {
        let mut input = approval(2, Some(1));
        input.payload = payload;
        let r = resolved(&input).unwrap();
        assert_eq!(r.enum_name, "DocApprovalDecisionChoice");
        assert_eq!(r.entity, "DocApprovalDecision");
        assert_eq!(
            r.schema,
            vec![
                ("actor".into(), WType::Id("User".into())),
                ("amount".into(), WType::Int),
                (
                    "choice".into(),
                    WType::Enum("DocApprovalDecisionChoice".into())
                ),
                ("comment".into(), WType::String),
                ("subject".into(), WType::Id("Document".into())),
            ]
        );
        assert!(
            r.schema
                .iter()
                .all(|(_, t)| !matches!(t, WType::Ref(_) | WType::Option(_))),
            "never Ref"
        );
        assert_eq!((r.approve_threshold, r.reject_threshold), (2, Some(1)));
    }
}

/// Golden (data model §4): `decision_id`, inputs sorted, `subject`, context sorted; the action
/// names are `encode(p, approve | reject)` and the choice is fixed by the capability.
#[test]
fn the_capability_parameters_have_the_fixed_layout() {
    let mut input = approval(1, None);
    for c in &mut input.capabilities {
        c.inputs = vec![
            ("reason".into(), WType::String),
            ("note".into(), WType::String),
        ];
        c.context = vec![
            ("time".into(), WType::String),
            ("actor".into(), WType::Id("User".into())),
        ];
    }
    let r = resolved(&input).unwrap();
    for (id, choice) in [("approve", "APPROVE"), ("reject", "REJECT")] {
        let c = &r.capabilities[id];
        assert_eq!(c.choice, choice);
        assert_eq!(c.action_name, format!("doc_approval__{id}"));
        let layout: Vec<(String, Option<Role>, WType)> = r
            .capability_params(c)
            .into_iter()
            .map(|p| (p.name, p.role, p.ty))
            .collect();
        assert_eq!(
            layout,
            vec![
                (
                    "decision_id".into(),
                    Some(Role::Input),
                    WType::Id("DocApprovalDecision".into())
                ),
                ("note".into(), Some(Role::Input), WType::String),
                ("reason".into(), Some(Role::Input), WType::String),
                (
                    "subject".into(),
                    Some(Role::State),
                    WType::Entity("Document".into())
                ),
                (
                    "actor".into(),
                    Some(Role::Context),
                    WType::Id("User".into())
                ),
                ("time".into(), Some(Role::Context), WType::String),
            ]
        );
    }
}
