//! Semantic content hashing v1 (contracts/hashing.md).
//!
//! `hash(node) = SHA-256(tag ‖ 0x00 ‖ body)`. Bodies use a fixed length-prefixed binary
//! encoding of semantic content only: no source locations, no names for humans, no JSON.

use sha2::{Digest, Sha256};

use crate::semantic::expr::{ExprKind, FoldOp, QueryKind, SetOp};
use crate::semantic::module::{Kind, ParamRole};
use crate::semantic::types::{ArithOp, CmpOp, Hash, Prim, Type};
use crate::semantic::value::Value;
use crate::wire::DerivedKind;

pub const TAG_ENUM: &str = "behavior.enum.v1";
pub const TAG_NOMINAL: &str = "behavior.nominal.v1";
pub const TAG_NOMINAL_FIXED: &str = "behavior.nominal.fixed.v1";
pub const TAG_ENTITY: &str = "behavior.entity.v1";
pub const TAG_EXPR: &str = "behavior.expr.v1";
pub const TAG_EFFECT: &str = "behavior.effect.v1";
pub const TAG_EFFECT_CREATE: &str = "behavior.effect.create.v1";
pub const TAG_EFFECT_REMOVE: &str = "behavior.effect.remove.v1";
pub const TAG_DERIVED: &str = "behavior.derived.v1";
pub const TAG_INVARIANT: &str = "behavior.invariant.v1";
pub const TAG_ACTION: &str = "behavior.action.v1";
pub const TAG_CONSTRAINT: &str = "behavior.constraint.v1";
pub const TAG_MODULE: &str = "behavior.module.v1";
/// The store schema (feature 009): the sorted entity declarations.
pub const TAG_STORE_SCHEMA: &str = "behavior.store_schema.v1";
/// A resolved migration (feature 009).
pub const TAG_MIGRATION: &str = "behavior.migration.v1";

/// Body encoder.
#[derive(Default)]
struct Enc(Vec<u8>);

impl Enc {
    fn u8(&mut self, b: u8) -> &mut Self {
        self.0.push(b);
        self
    }
    fn u32(&mut self, n: usize) -> &mut Self {
        // Lengths and counts beyond u32 cannot occur in admitted modules; saturate defensively.
        let n = u32::try_from(n).unwrap_or(u32::MAX);
        self.0.extend_from_slice(&n.to_be_bytes());
        self
    }
    fn i64(&mut self, n: i64) -> &mut Self {
        self.0.extend_from_slice(&n.to_be_bytes());
        self
    }
    fn str(&mut self, s: &str) -> &mut Self {
        self.u32(s.len());
        self.0.extend_from_slice(s.as_bytes());
        self
    }
    fn href(&mut self, h: &Hash) -> &mut Self {
        self.0.extend_from_slice(h);
        self
    }
    fn refs<'a>(&mut self, hs: impl ExactSizeIterator<Item = &'a Hash>) -> &mut Self {
        self.u32(hs.len());
        for h in hs {
            self.href(h);
        }
        self
    }
    fn strs(&mut self, ss: &[String]) -> &mut Self {
        self.u32(ss.len());
        for s in ss {
            self.str(s);
        }
        self
    }
    fn ty(&mut self, t: &Type) -> &mut Self {
        match t {
            Type::Bool => self.u8(0x01),
            Type::Int => self.u8(0x02),
            Type::Decimal => self.u8(0x03),
            Type::String => self.u8(0x04),
            Type::Option(inner) => self.u8(0x10).ty(inner),
            Type::Enum(e) => self.u8(0x20).href(&e.hash),
            Type::Id(entity) => self.u8(0x21).str(entity),
            Type::Nominal(n) => self.u8(0x22).href(&n.hash),
            Type::Exact(crate::semantic::types::Unit::Nominal(n)) => self.u8(0x23).href(&n.hash),
            Type::Exact(crate::semantic::types::Unit::Dimensionless) => self.u8(0x24),
            Type::Entity(entity) => self.u8(0x30).str(entity),
        }
    }
    fn value(&mut self, t: &Type, v: &Value) -> &mut Self {
        match (t, v) {
            (Type::Option(_), Value::None) => self.u8(0),
            (Type::Option(inner), v) => self.u8(1).value(inner, v),
            (_, Value::Bool(b)) => self.u8(u8::from(*b)),
            (_, Value::Int(i)) => self.i64(*i),
            (_, Value::Dec(d)) => self.str(&d.to_normalized_string()),
            (_, Value::Str(s)) => self.str(s),
            // Entities and bare `None` never appear as literals in admitted IR.
            (_, Value::None) => self.u8(0),
            (_, Value::Entity(_)) => self,
            // Exact quantities are never literals either.
            (_, Value::Exact(x)) => self.str(&x.to_text()),
        }
    }
    fn finish(&self, tag: &str) -> Hash {
        let mut h = Sha256::new();
        h.update(tag.as_bytes());
        h.update([0u8]);
        h.update(&self.0);
        h.finalize().into()
    }
}

pub fn prim_type(p: Prim) -> Type {
    p.to_type()
}

pub fn enum_decl(name: &str, values: &[String]) -> Hash {
    Enc::default().str(name).strs(values).finish(TAG_ENUM)
}

pub fn nominal_decl(name: &str, underlying: Prim, ops: u8) -> Hash {
    Enc::default()
        .str(name)
        .ty(&prim_type(underlying))
        .u8(ops)
        .finish(TAG_NOMINAL)
}

/// A fixed-scale nominal: its own tag, so nominals without scale keep their v1 hashes.
pub fn nominal_fixed_decl(name: &str, underlying: Prim, ops: u8, scale: u8) -> Hash {
    Enc::default()
        .str(name)
        .ty(&prim_type(underlying))
        .u8(ops)
        .u8(scale)
        .finish(TAG_NOMINAL_FIXED)
}

/// `fields` includes the implicit `id` field first. A reference field (`Ref<T>`, feature 006)
/// encodes its `Id<T>` with the code `0x25`; entities without references hash as before.
pub fn entity(name: &str, fields: &[(String, Type)], references: &[String]) -> Hash {
    let mut e = Enc::default();
    e.str(name).u32(fields.len());
    for (f, t) in fields {
        e.str(f);
        match t {
            Type::Id(target) if references.contains(f) => {
                e.u8(0x25).str(target);
            }
            Type::Option(inner) if references.contains(f) => {
                if let Type::Id(target) = inner.as_ref() {
                    e.u8(0x10).u8(0x25).str(target);
                }
            }
            t => {
                e.ty(t);
            }
        }
    }
    e.finish(TAG_ENTITY)
}

fn role_byte(r: ParamRole) -> u8 {
    match r {
        ParamRole::Read => 0,
        ParamRole::State => 1,
        ParamRole::Input => 2,
        ParamRole::Context => 3,
    }
}

fn params(e: &mut Enc, ps: &[(String, ParamRole, Type)]) {
    e.u32(ps.len());
    for (name, role, ty) in ps {
        e.str(name).u8(role_byte(*role)).ty(ty);
    }
}

fn cmp_code(op: CmpOp) -> u8 {
    match op {
        CmpOp::Eq => 0x10,
        CmpOp::Ne => 0x11,
        CmpOp::Lt => 0x12,
        CmpOp::Le => 0x13,
        CmpOp::Gt => 0x14,
        CmpOp::Ge => 0x15,
    }
}

fn arith_code(op: ArithOp) -> u8 {
    match op {
        ArithOp::Add => 0x20,
        ArithOp::Sub => 0x21,
        ArithOp::Mul => 0x22,
        ArithOp::Div => 0x23,
    }
}

/// Hash of an expression node from its kind, result type, and children's hashes.
pub fn expr(kind: &ExprKind, ty: &Type) -> Hash {
    let mut e = Enc::default();
    let op = |e: &mut Enc, code: u8| {
        e.u8(code).ty(ty);
    };
    match kind {
        ExprKind::Lit(v) => {
            op(&mut e, 0x01);
            e.value(ty, v);
        }
        ExprKind::Field { param, field } => {
            op(&mut e, 0x02);
            e.str(param).str(field);
        }
        ExprKind::Param(p) => {
            op(&mut e, 0x03);
            e.str(p);
        }
        ExprKind::DerivedRef { target, args, .. } => {
            op(&mut e, 0x04);
            e.href(target).strs(args);
        }
        ExprKind::Cmp(c, a, b) => {
            op(&mut e, cmp_code(*c));
            e.href(&a.hash).href(&b.hash);
        }
        ExprKind::Arith(c, a, b) => {
            op(&mut e, arith_code(*c));
            e.href(&a.hash).href(&b.hash);
        }
        ExprKind::And(xs) => {
            op(&mut e, 0x30);
            e.refs(xs.iter().map(|x| &x.hash));
        }
        ExprKind::Or(xs) => {
            op(&mut e, 0x31);
            e.refs(xs.iter().map(|x| &x.hash));
        }
        ExprKind::Not(a) => {
            op(&mut e, 0x32);
            e.href(&a.hash);
        }
        ExprKind::In(a, values) => {
            op(&mut e, 0x33);
            e.href(&a.hash).u32(values.len());
            for v in values {
                e.value(&a.ty, v);
            }
        }
        ExprKind::IsNone(a) => {
            op(&mut e, 0x40);
            e.href(&a.hash);
        }
        ExprKind::IsSome(a) => {
            op(&mut e, 0x41);
            e.href(&a.hash);
        }
        ExprKind::ValueOr(a, d) => {
            op(&mut e, 0x42);
            e.href(&a.hash).href(&d.hash);
        }
        ExprKind::Some(a) => {
            op(&mut e, 0x50);
            e.href(&a.hash);
        }
        ExprKind::ToDecimal(a) => {
            op(&mut e, 0x51);
            e.href(&a.hash);
        }
        ExprKind::Wrap(a) => {
            op(&mut e, 0x52);
            e.href(&a.hash);
        }
        ExprKind::Unwrap(a) => {
            op(&mut e, 0x53);
            e.href(&a.hash);
        }
        ExprKind::Rescale { arg, rounding } => {
            // The target type (and so its declaration hash) is `ty`.
            op(&mut e, 0x60);
            e.href(&arg.hash).u8(rounding.code());
        }
        ExprKind::Exists(a) => {
            op(&mut e, 0x70);
            e.href(&a.hash);
        }
        ExprKind::Referenced(a) => {
            op(&mut e, 0x71);
            e.href(&a.hash);
        }
        ExprKind::Count(q) => {
            op(&mut e, 0x80);
            e.href(&q.hash);
        }
        ExprKind::StrictUnwrap(a) => {
            op(&mut e, 0xa0);
            e.href(&a.hash);
        }
        ExprKind::EnumMap {
            arg,
            mapping,
            strict,
        } => {
            op(&mut e, if *strict { 0xa2 } else { 0xa1 });
            e.href(&arg.hash).u32(mapping.len());
            for (from, to) in mapping {
                e.str(from).str(to);
            }
        }
        ExprKind::Fold {
            op: fold,
            query,
            body,
            ..
        } => {
            let code = match fold {
                FoldOp::Any => 0x81,
                FoldOp::All => 0x82,
                FoldOp::Sum => 0x83,
                FoldOp::Min => 0x84,
                FoldOp::Max => 0x85,
                FoldOp::Unique => 0x86,
            };
            // The lambda parameter's source name is not hashed: the candidate is normalized.
            op(&mut e, code);
            e.href(&query.hash).href(&body.hash);
        }
    }
    e.finish(TAG_EXPR)
}

pub const TAG_QUERY: &str = "behavior.query.v1";
pub const TAG_GLOBAL_INVARIANT: &str = "behavior.invariant.global.v1";
pub const TAG_QUERY_INSTANCE: &str = "behavior.query_instance.v1";

/// The definition hash of a query node (feature 007).
pub fn query(kind: &QueryKind, entity: &str) -> Hash {
    let mut e = Enc::default();
    match kind {
        QueryKind::Select => {
            e.u8(0x90).str(entity);
        }
        QueryKind::Where { base, body, .. } => {
            e.u8(0x91).str(entity).href(&base.hash).href(&body.hash);
        }
        QueryKind::Set { op, a, b } => {
            let code = match op {
                SetOp::Union => 0x92,
                SetOp::Intersection => 0x93,
                SetOp::Difference => 0x94,
            };
            e.u8(code).str(entity).href(&a.hash).href(&b.hash);
        }
    }
    e.finish(TAG_QUERY)
}

/// A module-level invariant: its body only (it has no entity or parameter).
pub fn global_invariant(body: &Hash) -> Hash {
    Enc::default().href(body).finish(TAG_GLOBAL_INVARIANT)
}

/// A query instance: the definition hash and the canonical captured values (canonical JSON of
/// `[[read, value], …]` in canonical order).
pub fn query_instance(definition: &Hash, captures_json: &str) -> Hash {
    Enc::default()
        .href(definition)
        .str(captures_json)
        .finish(TAG_QUERY_INSTANCE)
}

pub fn effect(param: &str, field: &str, value: &Hash) -> Hash {
    Enc::default()
        .str(param)
        .str(field)
        .href(value)
        .finish(TAG_EFFECT)
}

/// A creation: entity name, identity expression, and every field's value (declaration order).
pub fn create(entity: &str, id: &Hash, fields: &[(String, Hash)]) -> Hash {
    let mut e = Enc::default();
    e.str(entity).href(id).u32(fields.len());
    for (f, h) in fields {
        e.str(f).href(h);
    }
    e.finish(TAG_EFFECT_CREATE)
}

/// A removal of the entity bound to a state parameter.
pub fn remove(param: &str) -> Hash {
    Enc::default().str(param).finish(TAG_EFFECT_REMOVE)
}

pub fn derived(kind: DerivedKind, ps: &[(String, ParamRole, Type)], body: &Hash) -> Hash {
    let mut e = Enc::default();
    e.u8(match kind {
        DerivedKind::Derived => 0,
        DerivedKind::Rule => 1,
    });
    params(&mut e, ps);
    e.href(body).finish(TAG_DERIVED)
}

pub fn invariant(entity: &str, param: &str, body: &Hash) -> Hash {
    Enc::default()
        .str(entity)
        .str(param)
        .href(body)
        .finish(TAG_INVARIANT)
}

/// An entity constraint: encoded like an invariant, under its own tag.
pub fn constraint(entity: &str, param: &str, body: &Hash) -> Hash {
    Enc::default()
        .str(entity)
        .str(param)
        .href(body)
        .finish(TAG_CONSTRAINT)
}

pub fn action(
    ps: &[(String, ParamRole, Type)],
    pre: &[Hash],
    effects: &[Hash],
    post: &[Hash],
) -> Hash {
    let mut e = Enc::default();
    params(&mut e, ps);
    e.refs(pre.iter())
        .refs(effects.iter())
        .refs(post.iter())
        .finish(TAG_ACTION)
}

/// `entries` must already be sorted by `(kind, name bytes)`.
pub fn module<'a>(entries: impl ExactSizeIterator<Item = (&'a (Kind, String), &'a Hash)>) -> Hash {
    let mut e = Enc::default();
    e.u32(entries.len());
    for ((kind, name), h) in entries {
        e.u8(*kind as u8).str(name).href(h);
    }
    e.finish(TAG_MODULE)
}

/// A store schema (feature 009, research R1): the entity declarations, sorted by name, each with
/// its declaration hash in display form.
pub fn store_schema(declarations: &std::collections::BTreeMap<String, String>) -> Hash {
    let mut e = Enc::default();
    e.u32(declarations.len());
    for (name, decl) in declarations {
        e.str(name).str(decl);
    }
    e.finish(TAG_STORE_SCHEMA)
}

/// One resolved target field of a migration transform: copied from a source field, or computed by
/// an expression (its hash).
pub enum MigrationField<'a> {
    Copy(&'a str),
    Expr(&'a Hash),
}

/// A resolved migration (feature 009, FR-010): its source and target schemas, constants (name,
/// type, value), requirements (name, body hash), transforms (entity, fields in target declaration
/// order, sorted drops), and retired types. Callers pass every list in canonical order; the
/// migration's name and source locations are not part of it.
#[allow(clippy::type_complexity)]
pub fn migration(
    source: &str,
    target: &str,
    constants: &[(&str, &Type, &Value)],
    requirements: &[(&str, &Hash)],
    transforms: &[(&str, Vec<(&str, MigrationField<'_>)>, &[String])],
    retire: &[String],
) -> Hash {
    let mut e = Enc::default();
    e.str(source).str(target).u32(constants.len());
    for (name, ty, v) in constants {
        e.str(name).ty(ty).value(ty, v);
    }
    e.u32(requirements.len());
    for (name, h) in requirements {
        e.str(name).href(h);
    }
    e.u32(transforms.len());
    for (entity, fields, drops) in transforms {
        e.str(entity).u32(fields.len());
        for (name, f) in fields {
            e.str(name);
            match f {
                MigrationField::Copy(from) => e.u8(0).str(from),
                MigrationField::Expr(h) => e.u8(1).href(h),
            };
        }
        e.strs(drops);
    }
    e.strs(retire);
    e.finish(TAG_MIGRATION)
}
