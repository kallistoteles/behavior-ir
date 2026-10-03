//! Type checking: turns wire expressions into typed, hashed semantic expressions, making every
//! implicit conversion explicit, and assembles the semantic module.

use std::collections::{BTreeMap, BTreeSet};

use crate::admit::resolve::{Decls, ParamSite, action_site, params, resolve_type};
use crate::admit::{AdmissionError, hash};
use crate::decimal::Dec;
use crate::exact::Rounding;
use crate::semantic::expr::{CANDIDATE, Expr, ExprKind, FoldOp, QueryKind, QueryNode, SetOp};
use crate::semantic::module::{
    ActionItem, Condition, ConstraintItem, CreateEffect, DerivedItem, Effect, InvariantItem, Kind,
    Module, Param, ParamRole, RemoveEffect,
};
use crate::semantic::types::{
    ArithOp, CmpOp, Conv, OpSig, Type, TypeCode, Unit, coerce, fixed_scale, type_of_op,
};
use crate::semantic::value::{Value, decode_scalar};
use crate::wire::{DerivedKind, LambdaOp, Loc, OpName, WExpr, WExprKind, WLifecycle, WModule};

struct Ctx<'a> {
    decls: &'a Decls,
    derived: &'a BTreeMap<String, DerivedItem>,
    scope: &'a [Param],
    errs: &'a mut Vec<AdmissionError>,
    /// Relational forms are allowed here (not in entity constraints, per-entity invariants or
    /// lambda bodies; feature 007).
    queries: bool,
    /// Inside a lambda body: only candidate-local reads (feature 007).
    lambda: bool,
    /// A migration expression (feature 009): `strict_unwrap` and enum maps are allowed.
    migration: bool,
}

/// Whether an expression (through the derived values it references) uses a relational form, and
/// whether it uses `exists` or `referenced`.
fn universe_use(e: &Expr, derived: &BTreeMap<String, DerivedItem>) -> (bool, bool) {
    let mut relational = false;
    let mut existential = false;
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        match &x.kind {
            ExprKind::Count(_) | ExprKind::Fold { .. } => relational = true,
            ExprKind::Exists(_) | ExprKind::Referenced(_) => existential = true,
            ExprKind::DerivedRef { name, .. } => {
                if let Some(d) = derived.get(name) {
                    let (r, x) = universe_use(&d.body, derived);
                    relational |= r;
                    existential |= x;
                }
            }
            _ => {}
        }
        stack.extend(x.children());
    }
    (relational, existential)
}

/// A lambda body with its parameter renamed to the candidate marker (a lambda parameter shadows
/// an enclosing one of the same name).
fn rename_candidate(w: &WExpr, from: &str) -> WExpr {
    let mut out = w.clone();
    fn walk(e: &mut WExpr, from: &str) {
        match &mut e.kind {
            WExprKind::Field { param, .. } if param == from => *param = CANDIDATE.to_string(),
            WExprKind::Param(name) if name == from => *name = CANDIDATE.to_string(),
            WExprKind::Derived { args, .. } => {
                for a in args.iter_mut().filter(|a| a.as_str() == from) {
                    *a = CANDIDATE.to_string();
                }
            }
            WExprKind::Op { args, .. } => args.iter_mut().for_each(|a| walk(a, from)),
            WExprKind::In { arg, .. }
            | WExprKind::Wrap { arg, .. }
            | WExprKind::Rescale { arg, .. }
            | WExprKind::StrictUnwrap(arg)
            | WExprKind::EnumMap { arg, .. } => walk(arg, from),
            WExprKind::Lambda { query, body, .. } => {
                walk(query, from);
                walk(body, from);
            }
            WExprKind::Lit { .. }
            | WExprKind::Field { .. }
            | WExprKind::Param(_)
            | WExprKind::Select { .. } => {}
        }
    }
    walk(&mut out, from);
    out
}

fn merge_signature(
    dst: &mut crate::semantic::module::Signature,
    src: crate::semantic::module::Signature,
) {
    for (entity, fields) in src.types {
        let slot = dst
            .types
            .entry(entity)
            .or_insert_with(|| Some(BTreeSet::new()));
        match fields {
            None => *slot = None,
            Some(new) => {
                if let Some(existing) = slot.as_mut() {
                    existing.extend(new);
                }
            }
        }
    }
}

fn derived_signature(
    name: &str,
    derived: &BTreeMap<String, DerivedItem>,
    cache: &mut BTreeMap<String, crate::semantic::module::Signature>,
    visiting: &mut BTreeSet<String>,
) -> crate::semantic::module::Signature {
    if let Some(sig) = cache.get(name) {
        return sig.clone();
    }
    if !visiting.insert(name.to_string()) {
        return crate::semantic::module::Signature::default();
    }
    let sig = derived
        .get(name)
        .map(|d| signature_inner(d.body(), derived, cache, visiting))
        .unwrap_or_default();
    visiting.remove(name);
    cache.insert(name.to_string(), sig.clone());
    sig
}

fn collect_candidate_dependencies(
    entity: &str,
    body: &Expr,
    derived: &BTreeMap<String, DerivedItem>,
    sig: &mut crate::semantic::module::Signature,
    cache: &mut BTreeMap<String, crate::semantic::module::Signature>,
    visiting: &mut BTreeSet<String>,
) {
    let mut stack = vec![body];
    while let Some(x) = stack.pop() {
        match &x.kind {
            ExprKind::Field { param, field } if param == CANDIDATE => {
                let slot = sig
                    .types
                    .entry(entity.to_string())
                    .or_insert_with(|| Some(BTreeSet::new()));
                if let Some(fields) = slot.as_mut() {
                    fields.insert(field.clone());
                }
            }
            ExprKind::DerivedRef { name, args, .. } => {
                if args.iter().any(|a| a == CANDIDATE) {
                    let slot = sig
                        .types
                        .entry(entity.to_string())
                        .or_insert_with(|| Some(BTreeSet::new()));
                    *slot = None;
                }
                merge_signature(sig, derived_signature(name, derived, cache, visiting));
            }
            _ => {}
        }
        stack.extend(x.children());
    }
}

fn signature_inner(
    e: &Expr,
    derived: &BTreeMap<String, DerivedItem>,
    cache: &mut BTreeMap<String, crate::semantic::module::Signature>,
    visiting: &mut BTreeSet<String>,
) -> crate::semantic::module::Signature {
    let mut sig = crate::semantic::module::Signature::default();
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        let (entity, bodies) = match &x.kind {
            ExprKind::Count(q) => (Some(q.entity.clone()), q.bodies()),
            ExprKind::Fold { query, body, .. } => {
                let mut b = query.bodies();
                b.push(body);
                (Some(query.entity.clone()), b)
            }
            ExprKind::DerivedRef { name, .. } => {
                merge_signature(&mut sig, derived_signature(name, derived, cache, visiting));
                (None, Vec::new())
            }
            _ => (None, Vec::new()),
        };
        if let Some(entity) = entity {
            for body in bodies {
                collect_candidate_dependencies(&entity, body, derived, &mut sig, cache, visiting);
            }
        }
        stack.extend(x.children());
    }
    sig
}

/// The dependency signature of a module-level invariant: the queried entity types and the fields
/// its lambdas read (`None` when a derived value over the candidate may read any field).
pub(crate) fn signature(
    e: &Expr,
    derived: &BTreeMap<String, DerivedItem>,
) -> crate::semantic::module::Signature {
    signature_inner(e, derived, &mut BTreeMap::new(), &mut BTreeSet::new())
}

fn op_label(op: OpName) -> &'static str {
    match op {
        OpName::Eq => "==",
        OpName::Ne => "!=",
        OpName::Lt => "<",
        OpName::Le => "<=",
        OpName::Gt => ">",
        OpName::Ge => ">=",
        OpName::Add => "+",
        OpName::Sub => "-",
        OpName::Mul => "*",
        OpName::Div => "/",
        OpName::And => "and",
        OpName::Or => "or",
        OpName::Not => "not",
        OpName::IsNone => "is_none",
        OpName::IsSome => "is_some",
        OpName::Some => "some",
        OpName::ToDecimal => "to_decimal",
        OpName::Unwrap => "unwrap",
        OpName::ValueOr => "value_or",
        OpName::Exists => "exists",
        OpName::Referenced => "referenced",
        OpName::Count => "count",
        OpName::Union => "union",
        OpName::Intersection => "intersection",
        OpName::Difference => "difference",
    }
}

/// Builds `some(e)`, folding literals so implicit and explicit forms hash the same.
fn make_some(e: Expr, loc: &Loc) -> Expr {
    let ty = Type::Option(Box::new(e.ty.clone()));
    match e.kind {
        ExprKind::Lit(v) => Expr::new(ExprKind::Lit(v), ty, loc.clone()),
        _ => Expr::new(ExprKind::Some(Box::new(e)), ty, loc.clone()),
    }
}

/// Builds `to_decimal(e)`, folding integer literals into decimal literals.
fn make_to_decimal(e: Expr, loc: &Loc) -> Expr {
    match e.kind {
        ExprKind::Lit(Value::Int(i)) => Expr::new(
            ExprKind::Lit(Value::Dec(Dec::from_i64(i))),
            Type::Decimal,
            loc.clone(),
        ),
        _ => Expr::new(ExprKind::ToDecimal(Box::new(e)), Type::Decimal, loc.clone()),
    }
}

/// `Exact<T>` into `T`, `Exact<Decimal>` into `Decimal`: accepted by typing, then proven
/// representable by the bound analysis (research R3) or rejected with `LOSSY_CONVERSION`.
pub(crate) fn exact_store(value: &Type, target: &Type) -> bool {
    match (value, target) {
        (Type::Exact(Unit::Nominal(m)), Type::Nominal(n)) => m == n,
        (Type::Exact(Unit::Dimensionless), Type::Decimal) => true,
        _ => false,
    }
}

/// Why an implicitly stored exact value of type `ty` with bound facts `f` may not be
/// representable in its target (research R3), or `None` if admission proves it is: a fixed-scale
/// target needs `scale ≤ scale(F)` (its range is checked at runtime); a general decimal target
/// needs `scale ≤ 28` and at most 28 coefficient digits.
fn unrepresentable_store(ty: &Type, f: &crate::admit::bounds::Facts) -> Option<String> {
    let places = || {
        f.scale
            .map_or("unboundedly many".to_string(), |s| format!("up to {s}"))
    };
    match ty {
        Type::Exact(Unit::Nominal(n)) if n.scale.is_some() => {
            let scale = u32::from(n.scale.unwrap_or(0));
            f.scale.is_none_or(|s| s > scale).then(|| {
                format!(
                    "the exact value may have {} decimal places, `{}` has {scale}",
                    places(),
                    n.name
                )
            })
        }
        Type::Exact(_) => match (f.scale, f.cd) {
            (Some(s), Some(c)) if s <= 28 && c <= 28 => None,
            (Some(s), Some(c)) if s <= 28 => Some(format!(
                "the exact value may need {c} significant digits, decimals have 28"
            )),
            _ => Some(format!(
                "the exact value may have {} decimal places, decimals have at most 28",
                places()
            )),
        },
        _ => None,
    }
}

fn convert(e: Expr, c: Conv) -> Expr {
    let loc = e.loc.clone();
    match c {
        Conv::Keep => e,
        Conv::ToDecimal => make_to_decimal(e, &loc),
        Conv::Some => make_some(e, &loc),
    }
}

impl Ctx<'_> {
    fn err(&mut self, code: &str, msg: impl Into<String>, loc: &Loc) {
        self.errs.push(AdmissionError::new(code, msg, Some(loc)));
    }

    fn param(&self, name: &str) -> Option<&Param> {
        self.scope.iter().find(|p| p.name == name)
    }

    fn resolve_type(&mut self, t: &crate::wire::WType, loc: &Loc) -> Option<Type> {
        let entities: BTreeSet<String> = self.decls.entities.keys().cloned().collect();
        resolve_type(
            t,
            &self.decls.enums,
            &self.decls.nominals,
            &entities,
            loc,
            self.errs,
        )
    }

    fn literal(&mut self, ty: &Type, v: &serde_json::Value, loc: &Loc) -> Option<Value> {
        if v.is_f64() {
            self.err(
                "INVALID_LITERAL",
                "floating-point literals are not allowed",
                loc,
            );
            return None;
        }
        if matches!(ty, Type::Entity(_)) {
            self.err("TYPE_MISMATCH", "entities cannot be literals", loc);
            return None;
        }
        if matches!(ty, Type::Exact(_)) {
            self.err("TYPE_MISMATCH", "exact quantities cannot be literals", loc);
            return None;
        }
        match decode_scalar(ty, v) {
            Ok(val) => {
                self.check_grid(ty, &val, loc)?;
                Some(val)
            }
            Err(msg) => {
                self.err(
                    "INVALID_LITERAL",
                    format!("invalid `{ty}` literal {v}: {msg}"),
                    loc,
                );
                None
            }
        }
    }

    /// A fixed-scale literal must lie on its grid and within its range.
    fn check_grid(&mut self, ty: &Type, v: &Value, loc: &Loc) -> Option<()> {
        let (Some(n), Value::Dec(d)) = (fixed_scale(ty), v) else {
            return Some(());
        };
        let scale = n.scale.unwrap_or(0);
        if !d.on_grid(scale) || !d.in_fixed_range(scale) {
            self.err(
                "OFF_GRID_LITERAL",
                format!(
                    "`{d}` is not a `{}` value (scale {scale}, at most {} integer digits)",
                    n.name,
                    28 - scale
                ),
                loc,
            );
            return None;
        }
        Some(())
    }

    fn type_error(&mut self, code: TypeCode, what: &str, operands: &[Type], loc: &Loc) {
        let list: Vec<String> = operands.iter().map(|t| format!("`{t}`")).collect();
        let msg = match code {
            TypeCode::OpNotAllowed => {
                format!("`{what}` is not declared for {}", list.join(" and "))
            }
            TypeCode::EmptyIn => "`in` needs at least one value".to_string(),
            TypeCode::NestedOption => "options cannot be nested".to_string(),
            TypeCode::TypeMismatch => format!("cannot apply `{what}` to {}", list.join(" and ")),
            TypeCode::LossyConversion if what == "unwrap" => format!(
                "`unwrap` on {} would erase its nominal unit; exact quantities keep their unit \
                 until an explicit `rescale(value, Type, rounding)`",
                list.join(" and ")
            ),
            TypeCode::LossyConversion => format!(
                "`{what}` on {} would lose information; narrow explicitly with \
                 `rescale(value, Type, rounding)`",
                list.join(" and ")
            ),
        };
        self.err(code.as_str(), msg, loc);
    }

    fn expr(&mut self, w: &WExpr) -> Option<Expr> {
        let loc = &w.loc;
        match &w.kind {
            WExprKind::Lit { ty, value } => {
                let ty = self.resolve_type(ty, loc)?;
                let v = self.literal(&ty, value, loc)?;
                Some(Expr::new(ExprKind::Lit(v), ty, loc.clone()))
            }
            WExprKind::Field { param, field } => {
                let Some(p) = self.param(param) else {
                    self.err("UNKNOWN_PARAM", format!("unknown parameter `{param}`"), loc);
                    return None;
                };
                let Type::Entity(entity) = &p.ty else {
                    let msg = format!("`{param}` is not an entity; use it directly");
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                };
                let entity = entity.clone();
                let Some(fty) = self
                    .decls
                    .entities
                    .get(&entity)
                    .and_then(|e| e.field_type(field))
                    .cloned()
                else {
                    self.err(
                        "UNKNOWN_FIELD",
                        format!("`{entity}` has no field `{field}`"),
                        loc,
                    );
                    return None;
                };
                let kind = ExprKind::Field {
                    param: param.clone(),
                    field: field.clone(),
                };
                Some(Expr::new(kind, fty, loc.clone()))
            }
            WExprKind::Param(name) => {
                let Some(p) = self.param(name) else {
                    self.err("UNKNOWN_PARAM", format!("unknown parameter `{name}`"), loc);
                    return None;
                };
                if matches!(p.ty, Type::Entity(_)) {
                    let msg = format!("`{name}` is an entity; read one of its fields");
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                }
                let ty = p.ty.clone();
                Some(Expr::new(ExprKind::Param(name.clone()), ty, loc.clone()))
            }
            WExprKind::Derived { name, args } => {
                let Some(d) = self.derived.get(name) else {
                    self.err(
                        "UNKNOWN_DERIVED",
                        format!("unknown derived value `{name}`"),
                        loc,
                    );
                    return None;
                };
                if d.params.len() != args.len() {
                    let msg = format!(
                        "`{name}` takes {} argument(s), got {}",
                        d.params.len(),
                        args.len()
                    );
                    self.err("ARITY_MISMATCH", msg, loc);
                    return None;
                }
                for (arg, expected) in args.iter().zip(&d.params) {
                    let Some(p) = self.param(arg) else {
                        self.err("UNKNOWN_PARAM", format!("unknown parameter `{arg}`"), loc);
                        return None;
                    };
                    if p.ty != expected.ty {
                        let msg =
                            format!("`{arg}` is `{}`, `{name}` expects `{}`", p.ty, expected.ty);
                        self.err("TYPE_MISMATCH", msg, loc);
                        return None;
                    }
                }
                let (relational, existential) = universe_use(&d.body, self.derived);
                if relational && !self.queries {
                    let msg = format!("`{name}` uses a query, which is not allowed here");
                    self.err("QUERY_NOT_ALLOWED", msg, loc);
                    return None;
                }
                if self.lambda && (existential || args.iter().any(|a| a != CANDIDATE)) {
                    let msg = format!(
                        "a query predicate may use `{name}` only over the candidate alone, and only \
                         if it reads no other entities"
                    );
                    self.err("NON_LOCAL_PREDICATE", msg, loc);
                    return None;
                }
                let kind = ExprKind::DerivedRef {
                    name: name.clone(),
                    target: d.hash,
                    args: args.clone(),
                };
                Some(Expr::new(kind, d.body.ty.clone(), loc.clone()))
            }
            WExprKind::Select { .. }
            | WExprKind::Lambda {
                op: LambdaOp::Where,
                ..
            } => {
                self.err(
                    "TYPE_MISMATCH",
                    "a query is not a value; use count, any, all, sum, min, max or unique",
                    loc,
                );
                None
            }
            WExprKind::Lambda {
                op,
                query,
                param,
                body,
            } => self.fold(*op, query, param, body, loc),
            WExprKind::In { arg, values } => {
                let a = self.expr(arg)?;
                let (ty, _) = match type_of_op(
                    &OpSig::In {
                        count: values.len(),
                    },
                    std::slice::from_ref(&a.ty),
                ) {
                    Ok(r) => r,
                    Err(code) => {
                        self.type_error(code, "in", std::slice::from_ref(&a.ty), loc);
                        return None;
                    }
                };
                let mut vals = Vec::new();
                for v in values {
                    vals.push(self.literal(&a.ty, v, loc)?);
                }
                Some(Expr::new(ExprKind::In(Box::new(a), vals), ty, loc.clone()))
            }
            WExprKind::Wrap { nominal, arg } => {
                let Some(n) = self.decls.nominals.get(nominal).cloned() else {
                    self.err(
                        "UNKNOWN_TYPE",
                        format!("unknown nominal type `{nominal}`"),
                        loc,
                    );
                    return None;
                };
                let a = self.expr(arg)?;
                let operands = [a.ty.clone()];
                match type_of_op(&OpSig::Wrap(n), &operands) {
                    Ok((ty, convs)) => {
                        let inner = convert(a, convs[0]);
                        // Fold literals: `Money(0)` is a nominal literal.
                        match inner.kind {
                            ExprKind::Lit(v) => {
                                self.check_grid(&ty, &v, loc)?;
                                Some(Expr::new(ExprKind::Lit(v), ty, loc.clone()))
                            }
                            _ => Some(Expr::new(ExprKind::Wrap(Box::new(inner)), ty, loc.clone())),
                        }
                    }
                    Err(code) => {
                        self.type_error(code, nominal, &operands, loc);
                        None
                    }
                }
            }
            WExprKind::Rescale {
                nominal,
                rounding,
                arg,
            } => {
                let Some(n) = self.decls.nominals.get(nominal).cloned() else {
                    self.err(
                        "UNKNOWN_TYPE",
                        format!("unknown nominal type `{nominal}`"),
                        loc,
                    );
                    return None;
                };
                let Some(mode) = Rounding::parse(rounding) else {
                    self.err(
                        "UNKNOWN_ROUNDING",
                        format!(
                            "unknown rounding `{rounding}` (half_even, half_up, down, up, floor, ceiling)"
                        ),
                        loc,
                    );
                    return None;
                };
                let a = self.expr(arg)?;
                if n.scale.is_none() {
                    let msg =
                        format!("`rescale` needs a fixed-scale target; `{nominal}` has no scale");
                    self.err("EXACT_NOT_FIXED_SCALE", msg, loc);
                    return None;
                }
                let ok = match &a.ty {
                    Type::Int | Type::Decimal => true,
                    Type::Nominal(m) => *m == n,
                    Type::Exact(u) => u.nominal().is_none_or(|m| *m == n),
                    _ => false,
                };
                if !ok {
                    let msg = format!("cannot rescale `{}` to `{nominal}`", a.ty);
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                }
                let kind = ExprKind::Rescale {
                    arg: Box::new(a),
                    rounding: mode,
                };
                Some(Expr::new(kind, Type::Nominal(n), loc.clone()))
            }
            WExprKind::StrictUnwrap(arg) => {
                if !self.migration {
                    self.err(
                        "TYPE_MISMATCH",
                        "`strict_unwrap` is available only in migrations",
                        loc,
                    );
                    return None;
                }
                let a = self.expr(arg)?;
                let Type::Option(inner) = &a.ty else {
                    let msg = format!("`strict_unwrap` takes an option; `{}` is not one", a.ty);
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                };
                let ty = (**inner).clone();
                Some(Expr::new(
                    ExprKind::StrictUnwrap(Box::new(a)),
                    ty,
                    loc.clone(),
                ))
            }
            WExprKind::EnumMap {
                arg,
                to,
                mapping,
                strict,
            } => self.enum_map(arg, to, mapping, *strict, loc),
            WExprKind::Op {
                op: OpName::Union | OpName::Intersection | OpName::Difference,
                ..
            } => {
                self.err(
                    "TYPE_MISMATCH",
                    "a query is not a value; use count, any, all, sum, min, max or unique",
                    loc,
                );
                None
            }
            WExprKind::Op {
                op: OpName::Count,
                args,
            } => {
                if !self.relational_allowed(loc) {
                    return None;
                }
                let q = self.query(args.first()?)?;
                Some(Expr::new(
                    ExprKind::Count(Box::new(q)),
                    Type::Int,
                    loc.clone(),
                ))
            }
            WExprKind::Op {
                op: OpName::Exists | OpName::Referenced,
                ..
            } if self.lambda => {
                self.err(
                    "NON_LOCAL_PREDICATE",
                    "a query predicate reads only the candidate, captured values, inputs and \
                     context; `exists` and `referenced` read other entities",
                    loc,
                );
                None
            }
            WExprKind::Op { op, args } => {
                let mut typed = Vec::new();
                let mut failed = false;
                for a in args {
                    match self.expr(a) {
                        Some(e) => typed.push(e),
                        None => failed = true,
                    }
                }
                if failed {
                    return None;
                }
                let operands: Vec<Type> = typed.iter().map(|e| e.ty.clone()).collect();
                if matches!(op, OpName::Exists | OpName::Referenced) {
                    // `exists` takes `Id<T>` or `Option<Id<T>>`; `referenced` takes `Id<T>`.
                    let ok = match (&operands[..], op) {
                        ([Type::Id(_)], _) => true,
                        ([Type::Option(inner)], OpName::Exists) => {
                            matches!(inner.as_ref(), Type::Id(_))
                        }
                        _ => false,
                    };
                    if !ok {
                        self.type_error(TypeCode::TypeMismatch, op_label(*op), &operands, loc);
                        return None;
                    }
                    let arg = Box::new(typed.pop()?);
                    let kind = if *op == OpName::Exists {
                        ExprKind::Exists(arg)
                    } else {
                        ExprKind::Referenced(arg)
                    };
                    return Some(Expr::new(kind, Type::Bool, loc.clone()));
                }
                let sig = match op {
                    OpName::Eq => OpSig::Cmp(CmpOp::Eq),
                    OpName::Ne => OpSig::Cmp(CmpOp::Ne),
                    OpName::Lt => OpSig::Cmp(CmpOp::Lt),
                    OpName::Le => OpSig::Cmp(CmpOp::Le),
                    OpName::Gt => OpSig::Cmp(CmpOp::Gt),
                    OpName::Ge => OpSig::Cmp(CmpOp::Ge),
                    OpName::Add => OpSig::Arith(ArithOp::Add),
                    OpName::Sub => OpSig::Arith(ArithOp::Sub),
                    OpName::Mul => OpSig::Arith(ArithOp::Mul),
                    OpName::Div => OpSig::Arith(ArithOp::Div),
                    OpName::And => OpSig::And,
                    OpName::Or => OpSig::Or,
                    OpName::Not => OpSig::Not,
                    OpName::IsNone => OpSig::IsNone,
                    OpName::IsSome => OpSig::IsSome,
                    OpName::Some => OpSig::Some,
                    OpName::ToDecimal => OpSig::ToDecimal,
                    OpName::Unwrap => OpSig::Unwrap,
                    OpName::ValueOr => OpSig::ValueOr,
                    OpName::Exists
                    | OpName::Referenced
                    | OpName::Count
                    | OpName::Union
                    | OpName::Intersection
                    | OpName::Difference => return None,
                };
                let (ty, convs) = match type_of_op(&sig, &operands) {
                    Ok(r) => r,
                    Err(code) => {
                        self.type_error(code, op_label(*op), &operands, loc);
                        return None;
                    }
                };
                let mut xs: Vec<Expr> = typed
                    .into_iter()
                    .zip(convs)
                    .map(|(e, c)| convert(e, c))
                    .collect();
                let l = loc.clone();
                let b = |e: Expr| Box::new(e);
                let expr = match (op, xs.len()) {
                    (OpName::Some, 1) => return xs.pop().map(|e| make_some(e, loc)),
                    (OpName::ToDecimal, 1) => return xs.pop().map(|e| make_to_decimal(e, loc)),
                    (OpName::Unwrap, 1) => {
                        let e = xs.pop()?;
                        match e.kind {
                            ExprKind::Lit(v) => Expr::new(ExprKind::Lit(v), ty, l),
                            _ => Expr::new(ExprKind::Unwrap(b(e)), ty, l),
                        }
                    }
                    (OpName::Not, 1) => Expr::new(ExprKind::Not(b(xs.pop()?)), ty, l),
                    (OpName::IsNone, 1) => Expr::new(ExprKind::IsNone(b(xs.pop()?)), ty, l),
                    (OpName::IsSome, 1) => Expr::new(ExprKind::IsSome(b(xs.pop()?)), ty, l),
                    (OpName::And, _) => Expr::new(ExprKind::And(xs), ty, l),
                    (OpName::Or, _) => Expr::new(ExprKind::Or(xs), ty, l),
                    (_, 2) => {
                        let second = xs.pop()?;
                        let first = xs.pop()?;
                        let kind = match &sig {
                            OpSig::Cmp(c) => ExprKind::Cmp(*c, b(first), b(second)),
                            OpSig::Arith(a) => ExprKind::Arith(*a, b(first), b(second)),
                            OpSig::ValueOr => ExprKind::ValueOr(b(first), b(second)),
                            _ => return None,
                        };
                        Expr::new(kind, ty, l)
                    }
                    _ => return None,
                };
                Some(expr)
            }
        }
    }

    /// Reports `QUERY_NOT_ALLOWED` where relational forms are not allowed.
    /// `enum_map` / `strict_enum_map` (feature 009): every mapped value must exist on its side;
    /// a total map must cover every source value. The mapping is kept in source value order.
    fn enum_map(
        &mut self,
        arg: &WExpr,
        to: &crate::wire::WType,
        mapping: &[(String, String)],
        strict: bool,
        loc: &Loc,
    ) -> Option<Expr> {
        let name = if strict {
            "strict_enum_map"
        } else {
            "enum_map"
        };
        if !self.migration {
            self.err(
                "TYPE_MISMATCH",
                format!("`{name}` is available only in migrations"),
                loc,
            );
            return None;
        }
        let a = self.expr(arg)?;
        let (source, optional) = match &a.ty {
            Type::Enum(e) => (e.clone(), false),
            Type::Option(inner) => match inner.as_ref() {
                Type::Enum(e) => (e.clone(), true),
                _ => {
                    let msg = format!("`{name}` maps an enum; `{}` is not one", a.ty);
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                }
            },
            _ => {
                let msg = format!("`{name}` maps an enum; `{}` is not one", a.ty);
                self.err("TYPE_MISMATCH", msg, loc);
                return None;
            }
        };
        let target = match self.resolve_type(to, loc)? {
            Type::Enum(e) => e,
            other => {
                let msg = format!("`{name}` maps to an enum; `{other}` is not one");
                self.err("TYPE_MISMATCH", msg, loc);
                return None;
            }
        };
        let mut map: BTreeMap<&str, &str> = BTreeMap::new();
        for (from, into) in mapping {
            if !source.values.contains(from) {
                let msg = format!("`{from}` is not a value of `{}`", source.name);
                self.err("INVALID_LITERAL", msg, loc);
                return None;
            }
            if !target.values.contains(into) {
                let msg = format!("`{into}` is not a value of the target `{}`", target.name);
                self.err("INVALID_LITERAL", msg, loc);
                return None;
            }
            if map.insert(from, into).is_some() {
                let msg = format!("`{}.{from}` is mapped twice", source.name);
                self.err("INVALID_LITERAL", msg, loc);
                return None;
            }
        }
        let unmapped: Vec<&str> = source
            .values
            .iter()
            .filter(|v| !map.contains_key(v.as_str()))
            .map(String::as_str)
            .collect();
        if !strict && !unmapped.is_empty() {
            let msg = format!(
                "`enum_map` must map every value of `{}`; {} has no target value (narrow with \
                 `strict_enum_map` under a source requirement that proves it absent)",
                source.name,
                unmapped.join(", ")
            );
            self.err("UNMAPPED_ENUM_VALUE", msg, loc);
            return None;
        }
        let ordered: Vec<(String, String)> = source
            .values
            .iter()
            .filter_map(|v| map.get(v.as_str()).map(|t| (v.clone(), t.to_string())))
            .collect();
        let ty = if optional {
            Type::Option(Box::new(Type::Enum(target)))
        } else {
            Type::Enum(target)
        };
        let kind = ExprKind::EnumMap {
            arg: Box::new(a),
            mapping: ordered,
            strict,
        };
        Some(Expr::new(kind, ty, loc.clone()))
    }

    fn relational_allowed(&mut self, loc: &Loc) -> bool {
        if self.queries {
            return true;
        }
        let msg = if self.lambda {
            "a query predicate cannot contain another query"
        } else {
            "queries describe the whole state: they belong in module-level invariants and \
             action conditions, not in entity constraints or per-entity invariants"
        };
        self.err("QUERY_NOT_ALLOWED", msg, loc);
        false
    }

    /// A query expression: `select`, `where` or set algebra (feature 007).
    fn query(&mut self, w: &WExpr) -> Option<QueryNode> {
        let loc = &w.loc;
        match &w.kind {
            WExprKind::Select { entity } => {
                if !self.decls.entities.contains_key(entity) {
                    self.err("UNKNOWN_ENTITY", format!("unknown entity `{entity}`"), loc);
                    return None;
                }
                Some(QueryNode::new(
                    QueryKind::Select,
                    entity.clone(),
                    loc.clone(),
                ))
            }
            WExprKind::Lambda {
                op: LambdaOp::Where,
                query,
                param,
                body,
            } => {
                let base = self.query(query)?;
                let body = self.lambda_body(&base.entity, param, body)?;
                if body.ty != Type::Bool {
                    let msg = format!("a `where` predicate must be Bool, found `{}`", body.ty);
                    self.err("NOT_BOOLEAN", msg, body.loc());
                    return None;
                }
                let entity = base.entity.clone();
                let kind = QueryKind::Where {
                    base: Box::new(base),
                    param: param.clone(),
                    body: Box::new(body),
                };
                Some(QueryNode::new(kind, entity, loc.clone()))
            }
            WExprKind::Op {
                op: op @ (OpName::Union | OpName::Intersection | OpName::Difference),
                args,
            } => {
                let a = self.query(args.first()?)?;
                let b = self.query(args.get(1)?)?;
                if a.entity != b.entity {
                    let msg = format!(
                        "`{}` combines queries over different entity types (`{}` and `{}`)",
                        op_label(*op),
                        a.entity,
                        b.entity
                    );
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                }
                let set = match op {
                    OpName::Union => SetOp::Union,
                    OpName::Intersection => SetOp::Intersection,
                    _ => SetOp::Difference,
                };
                let entity = a.entity.clone();
                let kind = QueryKind::Set {
                    op: set,
                    a: Box::new(a),
                    b: Box::new(b),
                };
                Some(QueryNode::new(kind, entity, loc.clone()))
            }
            _ => {
                self.err(
                    "TYPE_MISMATCH",
                    "expected a query (`select`, `where`, `union`, `intersection` or `difference`)",
                    loc,
                );
                None
            }
        }
    }

    /// A lambda body over a candidate of type `entity`, candidate-local (feature 007).
    fn lambda_body(&mut self, entity: &str, param: &str, body: &WExpr) -> Option<Expr> {
        let renamed = rename_candidate(body, param);
        let mut scope: Vec<Param> = self
            .scope
            .iter()
            .filter(|p| p.name != param)
            .cloned()
            .collect();
        scope.push(Param {
            name: CANDIDATE.to_string(),
            role: ParamRole::Read,
            ty: Type::Entity(entity.to_string()),
        });
        let mut ctx = Ctx {
            decls: self.decls,
            derived: self.derived,
            scope: &scope,
            errs: &mut *self.errs,
            queries: false,
            lambda: true,
            migration: self.migration,
        };
        ctx.expr(&renamed)
    }

    /// `any`, `all`, `sum`, `min`, `max` or `unique` over a query (feature 007).
    fn fold(
        &mut self,
        op: LambdaOp,
        query: &WExpr,
        param: &str,
        body: &WExpr,
        loc: &Loc,
    ) -> Option<Expr> {
        if !self.relational_allowed(loc) {
            return None;
        }
        let q = self.query(query)?;
        let body = self.lambda_body(&q.entity, param, body)?;
        let e = body.ty.clone();
        let (fold, ty) = match op {
            LambdaOp::Any | LambdaOp::All => {
                if e != Type::Bool {
                    let msg = format!("`{}` needs a Bool predicate, found `{e}`", op.as_str());
                    self.err("NOT_BOOLEAN", msg, body.loc());
                    return None;
                }
                let f = if op == LambdaOp::Any {
                    FoldOp::Any
                } else {
                    FoldOp::All
                };
                (f, Type::Bool)
            }
            LambdaOp::Sum => {
                // An additive numeric type with a canonical zero; the result is the type of
                // repeated exact addition.
                let numeric = match &e {
                    Type::Int | Type::Decimal | Type::Exact(_) => true,
                    Type::Nominal(n) => n.underlying.is_numeric(),
                    _ => false,
                };
                let ty = if numeric {
                    type_of_op(&OpSig::Arith(ArithOp::Add), &[e.clone(), e.clone()])
                        .ok()
                        .map(|(t, _)| t)
                } else {
                    None
                };
                let Some(ty) = ty else {
                    let msg =
                        format!("`sum` needs an additive numeric type with a zero, found `{e}`");
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                };
                (FoldOp::Sum, ty)
            }
            LambdaOp::Min | LambdaOp::Max => {
                let ordered = match &e {
                    Type::Int | Type::Decimal | Type::Exact(_) => true,
                    Type::Nominal(n) => {
                        n.underlying.is_numeric()
                            && type_of_op(&OpSig::Cmp(CmpOp::Lt), &[e.clone(), e.clone()]).is_ok()
                    }
                    _ => false,
                };
                let ty = Type::Option(Box::new(e.clone()));
                if !ordered || crate::semantic::types::check_type(&ty).is_err() {
                    let msg = format!("`{}` needs an ordered type, found `{e}`", op.as_str());
                    self.err("TYPE_MISMATCH", msg, loc);
                    return None;
                }
                let f = if op == LambdaOp::Min {
                    FoldOp::Min
                } else {
                    FoldOp::Max
                };
                (f, ty)
            }
            LambdaOp::Unique => (FoldOp::Unique, Type::Bool),
            LambdaOp::Where => return None,
        };
        let kind = ExprKind::Fold {
            op: fold,
            query: Box::new(q),
            param: param.to_string(),
            body: Box::new(body),
        };
        Some(Expr::new(kind, ty, loc.clone()))
    }

    fn boolean(&mut self, w: &WExpr, at: &Loc, what: &str) -> Option<Expr> {
        let e = self.expr(w)?;
        if e.ty != Type::Bool {
            self.err(
                "NOT_BOOLEAN",
                format!("{what} must be Bool, found `{}`", e.ty),
                at,
            );
            return None;
        }
        Some(e)
    }
}

fn param_triples(ps: &[Param]) -> Vec<(String, ParamRole, Type)> {
    ps.iter()
        .map(|p| (p.name.clone(), p.role, p.ty.clone()))
        .collect()
}

/// Type-checks all behavior and assembles the module. Returns `None` if any error occurred.
pub(crate) fn build_module(
    w: &WModule,
    decls: Decls,
    order: Vec<String>,
    errs: &mut Vec<AdmissionError>,
) -> Option<Module> {
    let mut derived: BTreeMap<String, DerivedItem> = BTreeMap::new();
    let by_name: BTreeMap<&str, &crate::wire::WDerived> =
        w.derived.iter().map(|d| (d.name.as_str(), d)).collect();

    for name in &order {
        let Some(d) = by_name.get(name.as_str()) else {
            continue;
        };
        let Some(item) = derived_item(&decls, &derived, d, errs) else {
            continue;
        };
        derived.insert(name.clone(), item);
    }

    let mut invariants = BTreeMap::new();
    for i in &w.invariants {
        let scope = [Param {
            name: i.param.clone(),
            role: ParamRole::Read,
            ty: Type::Entity(i.entity.clone()),
        }];
        let mut ctx = Ctx {
            decls: &decls,
            derived: &derived,
            scope: &scope,
            errs,
            queries: false,
            lambda: false,
            migration: false,
        };
        let Some(body) = ctx.boolean(&i.body, &i.body.loc, "an invariant") else {
            continue;
        };
        let h = hash::invariant(&i.entity, &i.param, &body.hash);
        invariants.insert(
            i.name.clone(),
            InvariantItem {
                entity: i.entity.clone(),
                param: i.param.clone(),
                body,
                hash: h,
                loc: i.loc.clone(),
            },
        );
    }

    // Module-level invariants (feature 007): closed state expressions (no parameters in scope).
    let mut global_invariants = BTreeMap::new();
    for g in &w.global_invariants {
        let mut ctx = Ctx {
            decls: &decls,
            derived: &derived,
            scope: &[],
            errs,
            queries: true,
            lambda: false,
            migration: false,
        };
        let Some(body) = ctx.boolean(&g.body, &g.body.loc, "a module invariant") else {
            continue;
        };
        let h = hash::global_invariant(&body.hash);
        global_invariants.insert(
            g.name.clone(),
            crate::semantic::module::GlobalInvariantItem {
                signature: signature(&body, &derived),
                body,
                hash: h,
                loc: g.loc.clone(),
            },
        );
    }

    let mut constraints = BTreeMap::new();
    for c in &w.constraints {
        let scope = [Param {
            name: c.param.clone(),
            role: ParamRole::Read,
            ty: Type::Entity(c.entity.clone()),
        }];
        let mut ctx = Ctx {
            decls: &decls,
            derived: &derived,
            scope: &scope,
            errs,
            queries: false,
            lambda: false,
            migration: false,
        };
        let Some(body) = ctx.boolean(&c.body, &c.body.loc, "an entity constraint") else {
            continue;
        };
        let h = hash::constraint(&c.entity, &c.param, &body.hash);
        constraints.insert(
            c.name.clone(),
            ConstraintItem {
                entity: c.entity.clone(),
                param: c.param.clone(),
                body,
                hash: h,
                loc: c.loc.clone(),
                reference: None,
            },
        );
    }

    // `Ref<T>` fields: a synthesized constraint `exists(field)` (`is_none or exists` for an
    // optional reference), so evaluation and verification see one existence semantics.
    for (entity, item) in &decls.entities {
        for (field, fty) in item.fields.iter().filter(|(f, _)| item.is_reference(f)) {
            let loc = item
                .fields
                .iter()
                .skip(1)
                .zip(&item.field_locs)
                .find(|((f, _), _)| f == field)
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| item.loc.clone());
            let param = "self".to_string();
            let read = Expr::new(
                ExprKind::Field {
                    param: param.clone(),
                    field: field.clone(),
                },
                fty.clone(),
                loc.clone(),
            );
            let exists = Expr::new(
                ExprKind::Exists(Box::new(read.clone())),
                Type::Bool,
                loc.clone(),
            );
            let body = if matches!(fty, Type::Option(_)) {
                let none = Expr::new(ExprKind::IsNone(Box::new(read)), Type::Bool, loc.clone());
                Expr::new(ExprKind::Or(vec![none, exists]), Type::Bool, loc.clone())
            } else {
                exists
            };
            let h = hash::constraint(entity, &param, &body.hash);
            constraints.insert(
                format!("ref({entity}.{field})"),
                ConstraintItem {
                    entity: entity.clone(),
                    param,
                    body,
                    hash: h,
                    loc,
                    reference: Some(field.clone()),
                },
            );
        }
    }

    let mut actions = BTreeMap::new();
    for a in &w.actions {
        let Some(ps) = params(&decls, &a.params, action_site(a), &a.loc, errs) else {
            continue;
        };
        let mut ctx = Ctx {
            decls: &decls,
            derived: &derived,
            scope: &ps,
            errs,
            queries: true,
            lambda: false,
            migration: false,
        };
        let mut ok = true;
        let mut pre = Vec::new();
        for c in &a.preconditions {
            match ctx.boolean(&c.expr, &c.loc, "a precondition") {
                Some(expr) => pre.push(Condition {
                    expr,
                    loc: c.loc.clone(),
                }),
                None => ok = false,
            }
        }
        let mut effects = Vec::new();
        let mut assigned: BTreeSet<(String, String)> = BTreeSet::new();
        for e in &a.effects {
            let fty = match effect_target(&decls, &ps, &e.param, &e.field) {
                Ok(t) => t,
                Err((code, msg)) => {
                    ctx.err(code, msg, &e.loc);
                    ok = false;
                    continue;
                }
            };
            if !assigned.insert((e.param.clone(), e.field.clone())) {
                let msg = format!("`{}.{}` is assigned more than once", e.param, e.field);
                ctx.err("DUPLICATE_ASSIGNMENT", msg, &e.loc);
                ok = false;
                continue;
            }
            let Some(value) = ctx.expr(&e.value) else {
                ok = false;
                continue;
            };
            let coerced = if exact_store(&value.ty, &fty) {
                Some(Conv::Keep)
            } else {
                coerce(&fty, &value.ty)
            };
            let Some(c) = coerced else {
                let lossy = matches!(value.ty, Type::Exact(_))
                    || (fixed_scale(&fty).is_some() && value.ty == Type::Decimal);
                let (code, msg) = if lossy {
                    (
                        "LOSSY_CONVERSION",
                        format!(
                            "assigning `{}` to `{}.{}: {fty}` would lose information; \
                             use `rescale(value, {fty}, rounding)`",
                            value.ty, e.param, e.field
                        ),
                    )
                } else {
                    (
                        "TYPE_MISMATCH",
                        format!(
                            "cannot assign `{}` to `{}.{}: {fty}`",
                            value.ty, e.param, e.field
                        ),
                    )
                };
                ctx.err(code, msg, &e.loc);
                ok = false;
                continue;
            };
            let value = convert(value, c);
            let h = hash::effect(&e.param, &e.field, &value.hash);
            effects.push(Effect {
                param: e.param.clone(),
                field: e.field.clone(),
                value,
                hash: h,
                loc: e.loc.clone(),
            });
        }
        let (creates, removes) = lifecycle(&mut ctx, &ps, &effects, &a.lifecycle, &mut ok);
        let mut post = Vec::new();
        for c in &a.postconditions {
            match ctx.boolean(&c.expr, &c.loc, "a postcondition") {
                Some(expr) => post.push(Condition {
                    expr,
                    loc: c.loc.clone(),
                }),
                None => ok = false,
            }
        }
        if !ok {
            continue;
        }
        // Lifecycle effects follow the field effects; actions without them hash as before.
        let effect_hashes: Vec<_> = effects
            .iter()
            .map(|e| e.hash)
            .chain(creates.iter().map(|c| c.hash))
            .chain(removes.iter().map(|r| r.hash))
            .collect();
        let h = hash::action(
            &param_triples(&ps),
            &pre.iter().map(|c| c.expr.hash).collect::<Vec<_>>(),
            &effect_hashes,
            &post.iter().map(|c| c.expr.hash).collect::<Vec<_>>(),
        );
        actions.insert(
            a.name.clone(),
            ActionItem {
                params: ps,
                preconditions: pre,
                effects,
                creates,
                removes,
                postconditions: post,
                hash: h,
                loc: a.loc.clone(),
            },
        );
    }

    // Every exact value must fit the runtime's representation (feature 004, research R4).
    let mut facts = BTreeMap::new();
    let mut lossy_stores = Vec::new();
    let mut check = |e: &Expr, facts: &BTreeMap<String, crate::admit::bounds::Facts>| {
        let mut err = None;
        let f = crate::admit::bounds::facts(e, facts, &mut err);
        if let Some(e) = err {
            errs.push(e);
        }
        f
    };
    for name in &order {
        if let Some(d) = derived.get(name) {
            let f = check(&d.body, &facts);
            facts.insert(name.clone(), f);
        }
    }
    for i in invariants.values() {
        check(&i.body, &facts);
    }
    for g in global_invariants.values() {
        check(&g.body, &facts);
    }
    for c in constraints.values() {
        check(&c.body, &facts);
    }
    for a in actions.values() {
        for c in a.preconditions.iter().chain(&a.postconditions) {
            check(&c.expr, &facts);
        }
        for e in &a.effects {
            let f = check(&e.value, &facts);
            if let Some(problem) = unrepresentable_store(e.value.ty(), &f) {
                lossy_stores.push(AdmissionError::new(
                    "LOSSY_CONVERSION",
                    format!(
                        "assigning `{}` to `{}.{}` would lose information: {problem}; use \
                         `rescale(value, Type, rounding)` with a fixed-scale type",
                        e.value.ty(),
                        e.param,
                        e.field
                    ),
                    Some(&e.loc),
                ));
            }
        }
        for c in &a.creates {
            check(&c.id, &facts);
            for (field, value) in &c.fields {
                let f = check(value, &facts);
                if let Some(problem) = unrepresentable_store(value.ty(), &f) {
                    lossy_stores.push(AdmissionError::new(
                        "LOSSY_CONVERSION",
                        format!(
                            "creating `{}` with `{field}: {}` would lose information: {problem}; \
                             use `rescale(value, Type, rounding)` with a fixed-scale type",
                            c.entity,
                            value.ty()
                        ),
                        Some(&c.loc),
                    ));
                }
            }
        }
    }
    errs.extend(lossy_stores);

    if !errs.is_empty() {
        return None;
    }

    let mut name_table = BTreeMap::new();
    for (n, e) in &decls.enums {
        name_table.insert((Kind::Enum, n.clone()), e.hash);
    }
    for (n, x) in &decls.nominals {
        name_table.insert((Kind::Nominal, n.clone()), x.hash);
    }
    for (n, x) in &decls.entities {
        name_table.insert((Kind::Entity, n.clone()), x.hash);
    }
    for (n, x) in &derived {
        name_table.insert((Kind::Derived, n.clone()), x.hash);
    }
    for (n, x) in &invariants {
        name_table.insert((Kind::Invariant, n.clone()), x.hash);
    }
    for (n, x) in &global_invariants {
        name_table.insert((Kind::Invariant, n.clone()), x.hash);
    }
    // Synthesized reference constraints are part of their entity's hash, not items.
    for (n, x) in constraints.iter().filter(|(_, c)| c.reference.is_none()) {
        name_table.insert((Kind::Constraint, n.clone()), x.hash);
    }
    for (n, x) in &actions {
        name_table.insert((Kind::Action, n.clone()), x.hash);
    }
    let module_hash = hash::module(name_table.iter());

    Some(Module {
        enums: decls.enums,
        nominals: decls.nominals,
        entities: decls.entities,
        derived,
        invariants,
        global_invariants,
        constraints,
        actions,
        name_table,
        evaluation_order: order,
        enum_locs: w
            .enums
            .iter()
            .map(|e| (e.name.clone(), e.loc.clone()))
            .collect(),
        nominal_locs: w
            .nominals
            .iter()
            .map(|n| (n.name.clone(), n.loc.clone()))
            .collect(),
        hash: module_hash,
    })
}

/// Converts a value for storage in a field of type `fty`, or reports why it cannot be stored.
pub(crate) fn store_value(
    value: Expr,
    fty: &Type,
    what: &str,
) -> Result<Expr, (&'static str, String)> {
    let coerced = if exact_store(&value.ty, fty) {
        Some(Conv::Keep)
    } else {
        coerce(fty, &value.ty)
    };
    match coerced {
        Some(c) => Ok(convert(value, c)),
        None => {
            let lossy = matches!(value.ty, Type::Exact(_))
                || (fixed_scale(fty).is_some() && value.ty == Type::Decimal);
            if lossy {
                Err((
                    "LOSSY_CONVERSION",
                    format!(
                        "assigning `{}` to `{what}: {fty}` would lose information; \
                         use `rescale(value, {fty}, rounding)`",
                        value.ty
                    ),
                ))
            } else {
                Err((
                    "TYPE_MISMATCH",
                    format!("cannot assign `{}` to `{what}: {fty}`", value.ty),
                ))
            }
        }
    }
}

/// Type-checks an action's lifecycle effects (feature 006, research R2): complete, well-typed
/// creations with a typed identity; removals of state parameters; statically visible conflicts.
fn lifecycle(
    ctx: &mut Ctx<'_>,
    ps: &[Param],
    effects: &[Effect],
    ws: &[WLifecycle],
    ok: &mut bool,
) -> (Vec<CreateEffect>, Vec<RemoveEffect>) {
    let mut creates: Vec<CreateEffect> = Vec::new();
    let mut removes: Vec<RemoveEffect> = Vec::new();
    for w in ws {
        match w {
            WLifecycle::Create {
                entity,
                id,
                fields,
                loc,
            } => {
                let Some(item) = ctx.decls.entities.get(entity).cloned() else {
                    ctx.err("UNKNOWN_ENTITY", format!("unknown entity `{entity}`"), loc);
                    *ok = false;
                    continue;
                };
                let id_expr = ctx.expr(id);
                let mut failed = id_expr.is_none();
                if let Some(e) = &id_expr
                    && e.ty != Type::Id(entity.clone())
                {
                    let msg = format!(
                        "the identity of a created `{entity}` must be `Id<{entity}>`, found `{}`",
                        e.ty
                    );
                    ctx.err("TYPE_MISMATCH", msg, loc);
                    failed = true;
                }
                let mut values = Vec::new();
                for (name, value) in fields {
                    if name == "id" {
                        let msg = "the identity is given by `id`, not as a field";
                        ctx.err("RESERVED_NAME", msg, loc);
                        failed = true;
                        continue;
                    }
                    let Some(fty) = item.field_type(name).cloned() else {
                        ctx.err(
                            "UNKNOWN_FIELD",
                            format!("`{entity}` has no field `{name}`"),
                            loc,
                        );
                        failed = true;
                        continue;
                    };
                    let Some(v) = ctx.expr(value) else {
                        failed = true;
                        continue;
                    };
                    match store_value(v, &fty, &format!("{entity}.{name}")) {
                        Ok(v) => values.push((name.clone(), v)),
                        Err((code, msg)) => {
                            ctx.err(code, msg, loc);
                            failed = true;
                        }
                    }
                }
                let missing: Vec<&str> = item
                    .fields
                    .iter()
                    .skip(1)
                    .map(|(n, _)| n.as_str())
                    .filter(|n| !fields.iter().any(|(f, _)| f == n))
                    .collect();
                if !missing.is_empty() {
                    let msg = format!(
                        "creating `{entity}` needs a complete initial value; missing: {}",
                        missing.join(", ")
                    );
                    ctx.err("CREATE_INCOMPLETE", msg, loc);
                    failed = true;
                }
                let Some(id_expr) = id_expr.filter(|_| !failed) else {
                    *ok = false;
                    continue;
                };
                if creates
                    .iter()
                    .any(|c| c.entity == *entity && c.id.hash == id_expr.hash)
                {
                    let msg = format!(
                        "`{entity}` is created twice with the same identity `{}`",
                        crate::pretty::text(&id_expr)
                    );
                    ctx.err("LIFECYCLE_CONFLICT", msg, loc);
                    *ok = false;
                    continue;
                }
                // Declaration order, whatever the order in the document.
                let ordered: Vec<(String, Expr)> = item
                    .fields
                    .iter()
                    .skip(1)
                    .filter_map(|(n, _)| values.iter().find(|(f, _)| f == n).cloned())
                    .collect();
                let field_hashes: Vec<_> =
                    ordered.iter().map(|(n, e)| (n.clone(), e.hash)).collect();
                let h = hash::create(entity, &id_expr.hash, &field_hashes);
                creates.push(CreateEffect {
                    entity: entity.clone(),
                    id: id_expr,
                    fields: ordered,
                    hash: h,
                    loc: loc.clone(),
                });
            }
            WLifecycle::Remove { param, loc } => {
                let Some(p) = ps.iter().find(|p| p.name == *param) else {
                    ctx.err("UNKNOWN_PARAM", format!("unknown parameter `{param}`"), loc);
                    *ok = false;
                    continue;
                };
                let problem = match (&p.ty, p.role) {
                    (Type::Entity(_), ParamRole::State) => None,
                    (Type::Entity(_), _) => Some((
                        "EFFECT_ON_READONLY",
                        format!("`{param}` is read-only; only state parameters can be removed"),
                    )),
                    _ => Some((
                        "TYPE_MISMATCH",
                        format!("`{param}` is not an entity; `remove` takes a state parameter"),
                    )),
                };
                if let Some((code, msg)) = problem {
                    ctx.err(code, msg, loc);
                    *ok = false;
                    continue;
                }
                if removes.iter().any(|r| r.param == *param) {
                    ctx.err(
                        "LIFECYCLE_CONFLICT",
                        format!("`{param}` is removed twice"),
                        loc,
                    );
                    *ok = false;
                    continue;
                }
                if effects.iter().any(|e| e.param == *param) {
                    let msg = format!(
                        "`{param}` is both updated and removed; a transition performs at most one \
                         lifecycle operation per identity and a removed entity has no new value"
                    );
                    ctx.err("LIFECYCLE_CONFLICT", msg, loc);
                    *ok = false;
                    continue;
                }
                removes.push(RemoveEffect {
                    param: param.clone(),
                    hash: hash::remove(param),
                    loc: loc.clone(),
                });
            }
        }
    }
    (creates, removes)
}

/// Type-checks one wire expression in a scope; used by the builder for each new node, with
/// exactly the same rules as admission (research R17).
pub(crate) fn check_expr(
    decls: &Decls,
    derived: &BTreeMap<String, DerivedItem>,
    scope: &[Param],
    w: &WExpr,
) -> Result<Expr, AdmissionError> {
    let mut errs = Vec::new();
    let result = Ctx {
        decls,
        derived,
        scope,
        errs: &mut errs,
        queries: true,
        lambda: false,
        migration: false,
    }
    .expr(w);
    match result {
        Some(e) if errs.is_empty() => Ok(e),
        _ => Err(errs.into_iter().next().unwrap_or_else(|| {
            AdmissionError::new("TYPE_MISMATCH", "ill-typed expression", Some(&w.loc))
        })),
    }
}

/// Type-checks a migration expression (feature 009) in `scope` over the merged two-sided
/// declarations: requirements may use queries (`queries`), transforms may not.
pub(crate) fn check_migration_expr(
    decls: &Decls,
    derived: &BTreeMap<String, DerivedItem>,
    scope: &[Param],
    queries: bool,
    w: &WExpr,
) -> Result<Expr, Vec<AdmissionError>> {
    let mut errs = Vec::new();
    let result = Ctx {
        decls,
        derived,
        scope,
        errs: &mut errs,
        queries,
        lambda: false,
        migration: true,
    }
    .expr(w);
    match result {
        Some(e) if errs.is_empty() => Ok(e),
        _ if !errs.is_empty() => Err(errs),
        _ => Err(vec![AdmissionError::new(
            "TYPE_MISMATCH",
            "ill-typed expression",
            Some(&w.loc),
        )]),
    }
}

/// Whether an expression (through the derived values it uses) reads beyond one entity: a query,
/// `exists` or `referenced` (feature 009: migration transforms are entity-local).
pub(crate) fn reads_universe(e: &Expr, derived: &BTreeMap<String, DerivedItem>) -> bool {
    let (relational, existential) = universe_use(e, derived);
    relational || existential
}

/// The type of the field an effect assigns, or why the effect is not allowed.
pub(crate) fn effect_target(
    decls: &Decls,
    scope: &[Param],
    param: &str,
    field: &str,
) -> Result<Type, (&'static str, String)> {
    let Some(p) = scope.iter().find(|p| p.name == param) else {
        return Err(("UNKNOWN_PARAM", format!("unknown parameter `{param}`")));
    };
    if p.role != ParamRole::State {
        return Err((
            "EFFECT_ON_READONLY",
            format!("`{param}` is read-only; only state parameters can change"),
        ));
    }
    if field == "id" {
        return Err((
            "RESERVED_NAME",
            "an entity's `id` cannot change".to_string(),
        ));
    }
    let Type::Entity(entity) = &p.ty else {
        return Err(("TYPE_MISMATCH", format!("`{param}` is not an entity")));
    };
    decls
        .entities
        .get(entity)
        .and_then(|x| x.field_type(field))
        .cloned()
        .ok_or_else(|| {
            (
                "UNKNOWN_FIELD",
                format!("`{entity}` has no field `{field}`"),
            )
        })
}

/// Type-checks and hashes one derived value (all derived values it references must already be
/// in `derived`).
pub(crate) fn derived_item(
    decls: &Decls,
    derived: &BTreeMap<String, DerivedItem>,
    d: &crate::wire::WDerived,
    errs: &mut Vec<AdmissionError>,
) -> Option<DerivedItem> {
    let ps = params(decls, &d.params, ParamSite::Derived, &d.loc, errs)?;
    let body = {
        let mut ctx = Ctx {
            decls,
            derived,
            scope: &ps,
            errs,
            queries: true,
            lambda: false,
            migration: false,
        };
        if d.kind == DerivedKind::Rule {
            ctx.boolean(&d.body, &d.body.loc, "a rule")
        } else {
            ctx.expr(&d.body)
        }
    }?;
    let mut declared_type = None;
    if let Some(declared) = &d.declared {
        let entities: BTreeSet<String> = decls.entities.keys().cloned().collect();
        let declared = resolve_type(
            declared,
            &decls.enums,
            &decls.nominals,
            &entities,
            &d.loc,
            errs,
        )?;
        if declared != body.ty {
            let hint = if matches!(body.ty, Type::Exact(_)) && fixed_scale(&declared).is_some() {
                format!("; narrow explicitly with `rescale(value, {declared}, rounding)`")
            } else {
                String::new()
            };
            errs.push(AdmissionError::new(
                "DECLARED_TYPE_MISMATCH",
                format!(
                    "`{}` is declared `{declared}` but is `{}`{hint}",
                    d.name, body.ty
                ),
                Some(&d.loc),
            ));
            return None;
        }
        declared_type = Some(declared);
    }
    let h = hash::derived(d.kind, &param_triples(&ps), &body.hash);
    Some(DerivedItem {
        kind: d.kind,
        params: ps,
        body,
        hash: h,
        loc: d.loc.clone(),
        declared: declared_type,
    })
}
