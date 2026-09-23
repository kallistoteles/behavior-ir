//! Declarations: names, types, and parameters (enums, nominal types, entities, and the
//! parameter lists of derived values, invariants, and actions).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::admit::{AdmissionError, hash};
use crate::semantic::module::{EntityItem, Param, ParamRole};
use crate::semantic::types::{EnumInfo, NominalInfo, Prim, Type, check_type, ops};
use crate::wire::{Loc, WModule, WParam, WType};

/// Resolved declarations of the information model.
pub(crate) struct Decls {
    pub enums: BTreeMap<String, Arc<EnumInfo>>,
    pub nominals: BTreeMap<String, Arc<NominalInfo>>,
    pub entities: BTreeMap<String, EntityItem>,
}

fn err(errs: &mut Vec<AdmissionError>, code: &str, msg: impl Into<String>, loc: &Loc) {
    errs.push(AdmissionError::new(code, msg, Some(loc)));
}

fn prim_of(t: &WType) -> Option<Prim> {
    Some(match t {
        WType::Bool => Prim::Bool,
        WType::Int => Prim::Int,
        WType::Decimal => Prim::Decimal,
        WType::String => Prim::String,
        _ => return None,
    })
}

/// Resolves a wire type against the declarations. `entities` is the set of entity names.
pub(crate) fn resolve_type(
    t: &WType,
    enums: &BTreeMap<String, Arc<EnumInfo>>,
    nominals: &BTreeMap<String, Arc<NominalInfo>>,
    entities: &BTreeSet<String>,
    loc: &Loc,
    errs: &mut Vec<AdmissionError>,
) -> Option<Type> {
    let ty = match t {
        WType::Bool => Type::Bool,
        WType::Int => Type::Int,
        WType::Decimal => Type::Decimal,
        WType::String => Type::String,
        WType::Option(inner) => Type::Option(Box::new(resolve_type(
            inner, enums, nominals, entities, loc, errs,
        )?)),
        WType::Enum(name) => match enums.get(name) {
            Some(e) => Type::Enum(e.clone()),
            None => {
                err(errs, "UNKNOWN_TYPE", format!("unknown enum `{name}`"), loc);
                return None;
            }
        },
        WType::Nominal(name) => match nominals.get(name) {
            Some(n) => Type::Nominal(n.clone()),
            None => {
                err(
                    errs,
                    "UNKNOWN_TYPE",
                    format!("unknown nominal type `{name}`"),
                    loc,
                );
                return None;
            }
        },
        WType::Id(entity) | WType::Entity(entity) => {
            if !entities.contains(entity) {
                err(
                    errs,
                    "UNKNOWN_ENTITY",
                    format!("unknown entity `{entity}`"),
                    loc,
                );
                return None;
            }
            if matches!(t, WType::Id(_)) {
                Type::Id(entity.clone())
            } else {
                Type::Entity(entity.clone())
            }
        }
    };
    if let Err(code) = check_type(&ty) {
        err(
            errs,
            code.as_str(),
            format!("`{ty}` is not a valid type"),
            loc,
        );
        return None;
    }
    Some(ty)
}

pub(crate) fn declarations(w: &WModule, errs: &mut Vec<AdmissionError>) -> Decls {
    let mut type_names: BTreeSet<String> = BTreeSet::new();
    let mut claim = |name: &str, loc: &Loc, errs: &mut Vec<AdmissionError>| -> bool {
        if type_names.insert(name.to_string()) {
            true
        } else {
            err(
                errs,
                "DUPLICATE_NAME",
                format!("`{name}` is declared more than once"),
                loc,
            );
            false
        }
    };

    let mut enums = BTreeMap::new();
    for e in &w.enums {
        if !claim(&e.name, &e.loc, errs) {
            continue;
        }
        if e.values.is_empty() {
            err(
                errs,
                "DECODE_ERROR",
                format!("enum `{}` has no values", e.name),
                &e.loc,
            );
            continue;
        }
        let unique: BTreeSet<&String> = e.values.iter().collect();
        if unique.len() != e.values.len() {
            err(
                errs,
                "DUPLICATE_NAME",
                format!("enum `{}` repeats a value", e.name),
                &e.loc,
            );
            continue;
        }
        let h = hash::enum_decl(&e.name, &e.values);
        enums.insert(
            e.name.clone(),
            Arc::new(EnumInfo {
                name: e.name.clone(),
                values: e.values.clone(),
                hash: h,
            }),
        );
    }

    let mut nominals = BTreeMap::new();
    for n in &w.nominals {
        if !claim(&n.name, &n.loc, errs) {
            continue;
        }
        let Some(underlying) = prim_of(&n.underlying) else {
            err(
                errs,
                "TYPE_MISMATCH",
                format!("nominal `{}` must wrap a primitive", n.name),
                &n.loc,
            );
            continue;
        };
        let mut bits = 0u8;
        let mut ok = true;
        for op in &n.ops {
            match ops::parse(op) {
                Some(b) => bits |= b,
                None => {
                    err(
                        errs,
                        "DECODE_ERROR",
                        format!("unknown operation `{op}`"),
                        &n.loc,
                    );
                    ok = false;
                }
            }
        }
        if bits != 0 && !underlying.is_numeric() {
            err(
                errs,
                "TYPE_MISMATCH",
                format!(
                    "operations on `{}` require a numeric underlying type",
                    n.name
                ),
                &n.loc,
            );
            ok = false;
        }
        if !ok {
            continue;
        }
        let h = hash::nominal_decl(&n.name, underlying, bits);
        nominals.insert(
            n.name.clone(),
            Arc::new(NominalInfo {
                name: n.name.clone(),
                underlying,
                ops: bits,
                hash: h,
            }),
        );
    }

    let mut entity_names = BTreeSet::new();
    for e in &w.entities {
        if claim(&e.name, &e.loc, errs) {
            entity_names.insert(e.name.clone());
        }
    }

    let mut entities = BTreeMap::new();
    for e in &w.entities {
        if entities.contains_key(&e.name) {
            continue;
        }
        let mut fields = vec![("id".to_string(), Type::Id(e.name.clone()))];
        let mut ok = true;
        for f in &e.fields {
            if f.name == "id" {
                err(
                    errs,
                    "RESERVED_NAME",
                    "`id` is reserved for the entity's identity",
                    &f.loc,
                );
                ok = false;
                continue;
            }
            if fields.iter().any(|(n, _)| *n == f.name) {
                err(
                    errs,
                    "DUPLICATE_NAME",
                    format!("field `{}` is declared twice", f.name),
                    &f.loc,
                );
                ok = false;
                continue;
            }
            match resolve_type(&f.ty, &enums, &nominals, &entity_names, &f.loc, errs) {
                Some(Type::Entity(_)) => {
                    err(errs, "TYPE_MISMATCH", "fields cannot hold entities", &f.loc);
                    ok = false;
                }
                Some(t) => fields.push((f.name.clone(), t)),
                None => ok = false,
            }
        }
        if ok {
            let h = hash::entity(&e.name, &fields);
            entities.insert(
                e.name.clone(),
                EntityItem {
                    name: e.name.clone(),
                    fields,
                    hash: h,
                },
            );
        }
    }

    let decls = Decls {
        enums,
        nominals,
        entities,
    };
    // Behavior refers to the information model; checking it against a broken model would
    // only report follow-on errors.
    if !errs.is_empty() {
        return decls;
    }

    let mut behavior_names = BTreeSet::new();
    let mut claim_behavior = |name: &str, loc: &Loc, errs: &mut Vec<AdmissionError>| {
        if !behavior_names.insert(name.to_string()) {
            err(
                errs,
                "DUPLICATE_NAME",
                format!("`{name}` is declared more than once"),
                loc,
            );
        }
    };

    for d in &w.derived {
        claim_behavior(&d.name, &d.loc, errs);
        let _ = params(&decls, &d.params, ParamSite::Derived, &d.loc, errs);
    }
    for i in &w.invariants {
        claim_behavior(&i.name, &i.loc, errs);
        if !decls.entities.contains_key(&i.entity) {
            err(
                errs,
                "UNKNOWN_ENTITY",
                format!("unknown entity `{}`", i.entity),
                &i.loc,
            );
        }
    }
    for a in &w.actions {
        claim_behavior(&a.name, &a.loc, errs);
        let _ = params(&decls, &a.params, ParamSite::Action, &a.loc, errs);
    }
    decls
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamSite {
    Derived,
    Action,
}

/// Resolves a parameter list, reporting problems at the declaration's location.
pub(crate) fn params(
    decls: &Decls,
    ps: &[WParam],
    site: ParamSite,
    loc: &Loc,
    errs: &mut Vec<AdmissionError>,
) -> Option<Vec<Param>> {
    let entity_names: BTreeSet<String> = decls.entities.keys().cloned().collect();
    let mut out: Vec<Param> = Vec::new();
    let mut ok = true;
    for p in ps {
        if out.iter().any(|q| q.name == p.name) {
            err(
                errs,
                "DUPLICATE_NAME",
                format!("parameter `{}` is declared twice", p.name),
                loc,
            );
            ok = false;
            continue;
        }
        let Some(ty) = resolve_type(
            &p.ty,
            &decls.enums,
            &decls.nominals,
            &entity_names,
            loc,
            errs,
        ) else {
            ok = false;
            continue;
        };
        let role = match (site, p.role) {
            (ParamSite::Derived, None) => ParamRole::Read,
            (ParamSite::Derived, Some(_)) => {
                err(
                    errs,
                    "DECODE_ERROR",
                    "derived value parameters have no role",
                    loc,
                );
                ok = false;
                continue;
            }
            (ParamSite::Action, Some(r)) => r.into(),
            (ParamSite::Action, None) => {
                err(
                    errs,
                    "DECODE_ERROR",
                    format!("parameter `{}` needs a role", p.name),
                    loc,
                );
                ok = false;
                continue;
            }
        };
        let entity_required = matches!(role, ParamRole::Read | ParamRole::State);
        if entity_required && !matches!(ty, Type::Entity(_)) {
            err(
                errs,
                "TYPE_MISMATCH",
                format!("parameter `{}` must be an entity", p.name),
                loc,
            );
            ok = false;
            continue;
        }
        out.push(Param {
            name: p.name.clone(),
            role,
            ty,
        });
    }
    let needed = match site {
        ParamSite::Derived => !ps.is_empty(),
        ParamSite::Action => out.iter().any(|p| p.role == ParamRole::State),
    };
    if ok && !needed {
        let what = match site {
            ParamSite::Derived => "a derived value needs at least one entity parameter",
            ParamSite::Action => "an action needs at least one state parameter",
        };
        err(errs, "ARITY_MISMATCH", what, loc);
        ok = false;
    }
    ok.then_some(out)
}
