//! Semantic content hashing v1 (contracts/hashing.md).
//!
//! `hash(node) = SHA-256(tag ‖ 0x00 ‖ body)`. Bodies use a fixed length-prefixed binary
//! encoding of semantic content only: no source locations, no names for humans, no JSON.

use sha2::{Digest, Sha256};

use crate::semantic::expr::ExprKind;
use crate::semantic::module::{Kind, ParamRole};
use crate::semantic::types::{ArithOp, CmpOp, Hash, Prim, Type};
use crate::semantic::value::Value;
use crate::wire::DerivedKind;

pub const TAG_ENUM: &str = "behavior.enum.v1";
pub const TAG_NOMINAL: &str = "behavior.nominal.v1";
pub const TAG_ENTITY: &str = "behavior.entity.v1";
pub const TAG_EXPR: &str = "behavior.expr.v1";
pub const TAG_EFFECT: &str = "behavior.effect.v1";
pub const TAG_DERIVED: &str = "behavior.derived.v1";
pub const TAG_INVARIANT: &str = "behavior.invariant.v1";
pub const TAG_ACTION: &str = "behavior.action.v1";
pub const TAG_CONSTRAINT: &str = "behavior.constraint.v1";
pub const TAG_MODULE: &str = "behavior.module.v1";

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

/// `fields` includes the implicit `id` field first.
pub fn entity(name: &str, fields: &[(String, Type)]) -> Hash {
    let mut e = Enc::default();
    e.str(name).u32(fields.len());
    for (f, t) in fields {
        e.str(f).ty(t);
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
    }
    e.finish(TAG_EXPR)
}

pub fn effect(param: &str, field: &str, value: &Hash) -> Hash {
    Enc::default()
        .str(param)
        .str(field)
        .href(value)
        .finish(TAG_EFFECT)
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
