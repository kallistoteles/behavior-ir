//! Type checking: turns wire expressions into typed, hashed semantic expressions, making every
//! implicit conversion explicit, and assembles the semantic module.

use std::collections::{BTreeMap, BTreeSet};

use crate::admit::resolve::{Decls, ParamSite, params, resolve_type};
use crate::admit::{AdmissionError, hash};
use crate::decimal::Dec;
use crate::exact::Rounding;
use crate::semantic::expr::{Expr, ExprKind};
use crate::semantic::module::{
    ActionItem, Condition, ConstraintItem, DerivedItem, Effect, InvariantItem, Kind, Module, Param,
    ParamRole,
};
use crate::semantic::types::{
    ArithOp, CmpOp, Conv, OpSig, Type, TypeCode, Unit, coerce, fixed_scale, type_of_op,
};
use crate::semantic::value::{Value, decode_scalar};
use crate::wire::{DerivedKind, Loc, OpName, WExpr, WExprKind, WModule};

struct Ctx<'a> {
    decls: &'a Decls,
    derived: &'a BTreeMap<String, DerivedItem>,
    scope: &'a [Param],
    errs: &'a mut Vec<AdmissionError>,
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
                let kind = ExprKind::DerivedRef {
                    name: name.clone(),
                    target: d.hash,
                    args: args.clone(),
                };
                Some(Expr::new(kind, d.body.ty.clone(), loc.clone()))
            }
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
            },
        );
    }

    let mut actions = BTreeMap::new();
    for a in &w.actions {
        let Some(ps) = params(&decls, &a.params, ParamSite::Action, &a.loc, errs) else {
            continue;
        };
        let mut ctx = Ctx {
            decls: &decls,
            derived: &derived,
            scope: &ps,
            errs,
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
        let h = hash::action(
            &param_triples(&ps),
            &pre.iter().map(|c| c.expr.hash).collect::<Vec<_>>(),
            &effects.iter().map(|e| e.hash).collect::<Vec<_>>(),
            &post.iter().map(|c| c.expr.hash).collect::<Vec<_>>(),
        );
        actions.insert(
            a.name.clone(),
            ActionItem {
                params: ps,
                preconditions: pre,
                effects,
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
    for (n, x) in &constraints {
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
    }
    .expr(w);
    match result {
        Some(e) if errs.is_empty() => Ok(e),
        _ => Err(errs.into_iter().next().unwrap_or_else(|| {
            AdmissionError::new("TYPE_MISMATCH", "ill-typed expression", Some(&w.loc))
        })),
    }
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
