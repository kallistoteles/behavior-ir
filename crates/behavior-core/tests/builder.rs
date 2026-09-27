#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The construction API used by the Python binding (research R17).

mod common;

use behavior_core::builder::{Builder, Node, ScopeSite};
use behavior_core::wire::{DerivedKind, Loc, Role, WField, WParam, WType};
use behavior_core::{admission_report, admission_result};
use serde_json::json;

fn l(line: u64) -> Loc {
    Loc {
        file: "/src/model.py".into(),
        line,
    }
}

fn money() -> WType {
    WType::Nominal("Money".into())
}

fn p(name: &str, role: Option<Role>, entity: &str) -> WParam {
    WParam {
        name: name.into(),
        role,
        ty: WType::Entity(entity.into()),
    }
}

fn invoice_builder() -> Builder {
    let mut b = Builder::new();
    b.declare_nominal(
        "Money",
        WType::Decimal,
        vec!["add".into(), "order".into(), "ratio".into(), "scale".into()],
        Some(2),
        l(1),
    )
    .unwrap();
    b.declare_enum(
        "InvoiceStatus",
        vec!["pending".into(), "approved".into()],
        l(2),
    )
    .unwrap();
    b.declare_entity(
        "User",
        vec![
            WField {
                name: "role".into(),
                ty: WType::String,
                loc: l(3),
            },
            WField {
                name: "approval_limit".into(),
                ty: money(),
                loc: l(4),
            },
        ],
        l(3),
    )
    .unwrap();
    b.declare_entity(
        "Invoice",
        vec![
            WField {
                name: "amount".into(),
                ty: money(),
                loc: l(5),
            },
            WField {
                name: "status".into(),
                ty: WType::Enum("InvoiceStatus".into()),
                loc: l(6),
            },
            WField {
                name: "approved_by".into(),
                ty: WType::Option(Box::new(WType::Id("User".into()))),
                loc: l(7),
            },
        ],
        l(5),
    )
    .unwrap();
    b
}

fn status(b: &mut Builder, value: &str) -> Node {
    b.lit(WType::Enum("InvoiceStatus".into()), json!(value), l(10))
        .unwrap()
}

fn build_invoice() -> Builder {
    let mut b = invoice_builder();
    // invariant non_negative_amount(invoice) = invoice.amount >= Money(0)
    b.push_scope(
        ScopeSite::Derived,
        vec![p("invoice", None, "Invoice")],
        l(8),
    )
    .unwrap();
    let amount = b.field("invoice", "amount", l(9)).unwrap();
    let zero = b.lit(money(), json!("0"), l(9)).unwrap();
    let ge = b.op("ge", vec![amount, zero], l(9)).unwrap();
    b.pop_scope();
    b.add_invariant("non_negative_amount", "Invoice", "invoice", ge, l(8))
        .unwrap();

    // approve_invoice(invoice: state, actor: context)
    let params = vec![
        p("invoice", Some(Role::State), "Invoice"),
        p("actor", Some(Role::Context), "User"),
    ];
    b.push_scope(ScopeSite::Action, params.clone(), l(11))
        .unwrap();
    let s = b.field("invoice", "status", l(12)).unwrap();
    let pending = status(&mut b, "pending");
    let c1 = b.op("eq", vec![s, pending], l(12)).unwrap();
    let role = b.field("actor", "role", l(13)).unwrap();
    let manager = b.lit(WType::String, json!("manager"), l(13)).unwrap();
    let c2 = b.op("eq", vec![role, manager], l(13)).unwrap();
    let amt = b.field("invoice", "amount", l(14)).unwrap();
    let limit = b.field("actor", "approval_limit", l(14)).unwrap();
    let c3 = b.op("le", vec![amt, limit], l(14)).unwrap();
    for c in [&c1, &c2, &c3] {
        b.check_condition(c, "a precondition").unwrap();
    }
    let approved = status(&mut b, "approved");
    b.check_effect("invoice", "status", &approved).unwrap();
    let actor_id = b.field("actor", "id", l(16)).unwrap();
    b.check_effect("invoice", "approved_by", &actor_id).unwrap();
    let ab = b.field("invoice", "approved_by", l(17)).unwrap();
    let aid = b.field("actor", "id", l(17)).unwrap();
    let post = b.op("eq", vec![ab, aid], l(17)).unwrap();
    b.pop_scope();
    b.add_action(
        "approve_invoice",
        params,
        vec![(c1, l(12)), (c2, l(13)), (c3, l(14))],
        vec![
            ("invoice".into(), "status".into(), approved, l(15)),
            ("invoice".into(), "approved_by".into(), actor_id, l(16)),
        ],
        vec![(post, l(17))],
        l(11),
    )
    .unwrap();

    // apply_discount(invoice: state, discount: input Money)
    let params = vec![
        p("invoice", Some(Role::State), "Invoice"),
        WParam {
            name: "discount".into(),
            role: Some(Role::Input),
            ty: money(),
        },
    ];
    b.push_scope(ScopeSite::Action, params.clone(), l(20))
        .unwrap();
    let s = b.field("invoice", "status", l(21)).unwrap();
    let pending = status(&mut b, "pending");
    let c = b.op("eq", vec![s, pending], l(21)).unwrap();
    let amt = b.field("invoice", "amount", l(22)).unwrap();
    let disc = b.param("discount", l(22)).unwrap();
    let new_amount = b.op("sub", vec![amt, disc], l(22)).unwrap();
    b.check_effect("invoice", "amount", &new_amount).unwrap();
    b.pop_scope();
    b.add_action(
        "apply_discount",
        params,
        vec![(c, l(21))],
        vec![("invoice".into(), "amount".into(), new_amount, l(22))],
        vec![],
        l(20),
    )
    .unwrap();
    b
}

#[test]
fn builder_and_wire_json_give_the_same_identity() {
    let module = build_invoice().finish(None).unwrap();
    let wire = common::read(&common::fixtures().join("wire/valid/invoice.json"));
    let from_json = admission_report(&wire);
    let from_builder = admission_result(&module);
    assert_eq!(from_builder.behavior_version, from_json.behavior_version);
    assert_eq!(from_builder.items, from_json.items);
}

#[test]
fn locations_are_relative_to_the_common_root() {
    let module = build_invoice().finish(None).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&behavior_core::serialize::to_wire_json(&module)).unwrap();
    assert_eq!(v["actions"][0]["loc"]["file"], "model.py");
}

#[test]
fn project_margin_matches_wire_fixture() {
    let mut b = Builder::new();
    b.declare_nominal(
        "Money",
        WType::Decimal,
        vec!["add".into(), "order".into(), "ratio".into(), "scale".into()],
        None,
        l(1),
    )
    .unwrap();
    b.declare_entity(
        "Project",
        vec![
            WField {
                name: "revenue".into(),
                ty: money(),
                loc: l(2),
            },
            WField {
                name: "cost".into(),
                ty: money(),
                loc: l(3),
            },
            WField {
                name: "flagged".into(),
                ty: WType::Bool,
                loc: l(4),
            },
        ],
        l(2),
    )
    .unwrap();
    let dp = vec![p("project", None, "Project")];
    b.push_scope(ScopeSite::Derived, dp.clone(), l(5)).unwrap();
    let rev = b.field("project", "revenue", l(6)).unwrap();
    let cost = b.field("project", "cost", l(6)).unwrap();
    let diff = b.op("sub", vec![rev, cost], l(6)).unwrap();
    let rev2 = b.field("project", "revenue", l(6)).unwrap();
    let margin = b.op("div", vec![diff, rev2], l(6)).unwrap();
    b.pop_scope();
    b.add_derived(
        "margin",
        DerivedKind::Derived,
        dp.clone(),
        margin,
        None,
        l(5),
    )
    .unwrap();
    b.push_scope(ScopeSite::Derived, dp.clone(), l(7)).unwrap();
    let m = b
        .derived_ref("margin", vec!["project".into()], l(8))
        .unwrap();
    assert_eq!(m.type_wire_json(), Some(json!({"t": "exact"})));
    let five = b.lit(WType::Decimal, json!("0.05"), l(8)).unwrap();
    let hr = b.op("lt", vec![m, five], l(8)).unwrap();
    b.pop_scope();
    b.add_derived("high_risk", DerivedKind::Rule, dp, hr, None, l(7))
        .unwrap();

    for (name, guarded) in [("flag_project", true), ("flag_project_unguarded", false)] {
        let ap = vec![p("project", Some(Role::State), "Project")];
        b.push_scope(ScopeSite::Action, ap.clone(), l(10)).unwrap();
        let risk = b
            .derived_ref("high_risk", vec!["project".into()], l(11))
            .unwrap();
        let cond = if guarded {
            let rev = b.field("project", "revenue", l(11)).unwrap();
            let zero = b.lit(money(), json!("0"), l(11)).unwrap();
            let ne = b.op("ne", vec![rev, zero], l(11)).unwrap();
            b.op("and", vec![ne, risk], l(11)).unwrap()
        } else {
            risk
        };
        let t = b.lit(WType::Bool, json!(true), l(12)).unwrap();
        b.pop_scope();
        b.add_action(
            name,
            ap,
            vec![(cond, l(11))],
            vec![("project".into(), "flagged".into(), t, l(12))],
            vec![],
            l(10),
        )
        .unwrap();
    }
    let module = b.finish(None).unwrap();
    let wire = common::read(&common::fixtures().join("wire/valid/project_margin.json"));
    assert_eq!(
        admission_result(&module).behavior_version,
        admission_report(&wire).behavior_version
    );
}

#[test]
fn nodes_are_checked_as_they_are_built() {
    let mut b = invoice_builder();
    let params = vec![
        p("invoice", Some(Role::State), "Invoice"),
        p("actor", Some(Role::Context), "User"),
    ];
    b.push_scope(ScopeSite::Action, params, l(1)).unwrap();

    let amount = b.field("invoice", "amount", l(2)).unwrap();
    let ten = b.lit(WType::Decimal, json!("10"), l(2)).unwrap();
    assert_eq!(
        b.op("add", vec![amount.clone(), ten], l(2))
            .unwrap_err()
            .code,
        "TYPE_MISMATCH"
    );

    let role = b.field("actor", "role", l(3)).unwrap();
    let role2 = b.field("actor", "role", l(3)).unwrap();
    assert_eq!(
        b.op("lt", vec![role, role2], l(3)).unwrap_err().code,
        "TYPE_MISMATCH"
    );

    assert_eq!(
        b.field("invoice", "title", l(4)).unwrap_err().code,
        "UNKNOWN_FIELD"
    );
    assert_eq!(
        b.field("approver", "role", l(5)).unwrap_err().code,
        "UNKNOWN_PARAM"
    );
    assert_eq!(
        b.check_condition(&amount, "a precondition")
            .unwrap_err()
            .code,
        "NOT_BOOLEAN"
    );

    let x = b.lit(WType::String, json!("x"), l(6)).unwrap();
    assert_eq!(
        b.check_effect("actor", "role", &x).unwrap_err().code,
        "EFFECT_ON_READONLY"
    );
    let id = b.field("invoice", "id", l(7)).unwrap();
    assert_eq!(
        b.check_effect("invoice", "id", &id).unwrap_err().code,
        "RESERVED_NAME"
    );
    assert_eq!(
        b.check_effect("invoice", "amount", &x).unwrap_err().code,
        "TYPE_MISMATCH"
    );
}

#[test]
fn op_not_allowed_on_nominal_without_order() {
    let mut b = Builder::new();
    b.declare_nominal("Plain", WType::Decimal, vec![], None, l(1))
        .unwrap();
    b.declare_entity(
        "E",
        vec![WField {
            name: "v".into(),
            ty: WType::Nominal("Plain".into()),
            loc: l(2),
        }],
        l(2),
    )
    .unwrap();
    b.push_scope(ScopeSite::Derived, vec![p("e", None, "E")], l(3))
        .unwrap();
    let a = b.field("e", "v", l(4)).unwrap();
    let c = b.field("e", "v", l(4)).unwrap();
    assert_eq!(
        b.op("lt", vec![a, c], l(4)).unwrap_err().code,
        "OP_NOT_ALLOWED"
    );
}

#[test]
fn cycles_are_reported_by_finish() {
    let mut b = Builder::new();
    b.declare_entity(
        "N",
        vec![WField {
            name: "w".into(),
            ty: WType::Int,
            loc: l(1),
        }],
        l(1),
    )
    .unwrap();
    let dp = vec![p("n", None, "N")];
    // a uses b, b uses a; neither is typed when referenced.
    for (name, other) in [("a", "b"), ("b", "a")] {
        b.push_scope(ScopeSite::Derived, dp.clone(), l(2)).unwrap();
        let r = b.derived_ref(other, vec!["n".into()], l(3)).unwrap();
        assert_eq!(r.type_wire_json(), None);
        let one = b.lit(WType::Int, json!(1), l(3)).unwrap();
        let body = b.op("add", vec![r, one], l(3)).unwrap();
        b.pop_scope();
        b.add_derived(name, DerivedKind::Derived, dp.clone(), body, None, l(2))
            .unwrap();
    }
    let err = b.finish(None).unwrap_err();
    assert_eq!(err.errors.len(), 1);
    assert_eq!(err.errors[0].code, "CYCLE");
}

#[test]
fn constraints_built_through_the_builder_match_the_wire_fixture() {
    let mut b = Builder::new();
    b.declare_nominal(
        "Money",
        WType::Decimal,
        vec!["add".into(), "order".into(), "ratio".into(), "scale".into()],
        Some(2),
        l(1),
    )
    .unwrap();
    b.declare_entity(
        "Employee",
        vec![
            WField {
                name: "role".into(),
                ty: WType::String,
                loc: l(2),
            },
            WField {
                name: "approval_limit".into(),
                ty: money(),
                loc: l(3),
            },
        ],
        l(2),
    )
    .unwrap();
    b.declare_entity(
        "Account",
        vec![WField {
            name: "balance".into(),
            ty: money(),
            loc: l(4),
        }],
        l(4),
    )
    .unwrap();
    for (name, entity, param, field) in [
        ("non_negative_limit", "Employee", "e", "approval_limit"),
        ("non_negative_balance", "Account", "a", "balance"),
    ] {
        b.push_scope(ScopeSite::Derived, vec![p(param, None, entity)], l(5))
            .unwrap();
        let f = b.field(param, field, l(6)).unwrap();
        let zero = b.lit(money(), json!("0"), l(6)).unwrap();
        let ge = b.op("ge", vec![f, zero], l(6)).unwrap();
        b.pop_scope();
        b.add_constraint(name, entity, param, ge, l(5)).unwrap();
    }
    // transfer
    let tp = vec![
        p("from_", Some(Role::State), "Account"),
        p("to", Some(Role::State), "Account"),
        WParam {
            name: "amount".into(),
            role: Some(Role::Input),
            ty: money(),
        },
    ];
    b.push_scope(ScopeSite::Action, tp.clone(), l(7)).unwrap();
    let amt = b.param("amount", l(8)).unwrap();
    let zero = b.lit(money(), json!("0"), l(8)).unwrap();
    let pre = b.op("gt", vec![amt, zero], l(8)).unwrap();
    let fb = b.field("from_", "balance", l(9)).unwrap();
    let a1 = b.param("amount", l(9)).unwrap();
    let e1 = b.op("sub", vec![fb, a1], l(9)).unwrap();
    let tb = b.field("to", "balance", l(10)).unwrap();
    let a2 = b.param("amount", l(10)).unwrap();
    let e2 = b.op("add", vec![tb, a2], l(10)).unwrap();
    b.pop_scope();
    b.add_action(
        "transfer",
        tp,
        vec![(pre, l(8))],
        vec![
            ("from_".into(), "balance".into(), e1, l(9)),
            ("to".into(), "balance".into(), e2, l(10)),
        ],
        vec![],
        l(7),
    )
    .unwrap();
    // review
    let rp = vec![
        p("account", Some(Role::State), "Account"),
        p("actor", Some(Role::Context), "Employee"),
    ];
    b.push_scope(ScopeSite::Action, rp.clone(), l(11)).unwrap();
    let role = b.field("actor", "role", l(12)).unwrap();
    let mgr = b.lit(WType::String, json!("manager"), l(12)).unwrap();
    let pre = b.op("eq", vec![role, mgr], l(12)).unwrap();
    b.pop_scope();
    b.add_action("review", rp, vec![(pre, l(12))], vec![], vec![], l(11))
        .unwrap();
    // assign
    let ap = vec![
        p("account", Some(Role::State), "Account"),
        p("approver", Some(Role::Input), "Employee"),
    ];
    b.push_scope(ScopeSite::Action, ap.clone(), l(13)).unwrap();
    let lim = b.field("approver", "approval_limit", l(14)).unwrap();
    let bal = b.field("account", "balance", l(14)).unwrap();
    let pre = b.op("ge", vec![lim, bal], l(14)).unwrap();
    b.pop_scope();
    b.add_action("assign", ap, vec![(pre, l(14))], vec![], vec![], l(13))
        .unwrap();

    let module = b.finish(None).unwrap();
    let wire = common::read(&common::fixtures().join("wire/valid/constraints.json"));
    assert_eq!(
        admission_result(&module).behavior_version,
        admission_report(&wire).behavior_version
    );
}
