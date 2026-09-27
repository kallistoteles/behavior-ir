//! Semantic expressions: typed, resolved, and hashed at construction.

use crate::admit::hash;
use crate::semantic::types::{ArithOp, CmpOp, Hash, Type};
use crate::semantic::value::Value;
use crate::wire::Loc;

#[derive(Debug, Clone)]
pub enum ExprKind {
    Lit(Value),
    Field {
        param: String,
        field: String,
    },
    Param(String),
    /// Reference to a derived value by hash; `name` is kept for evaluation and display only.
    DerivedRef {
        name: String,
        target: Hash,
        args: Vec<String>,
    },
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    Arith(ArithOp, Box<Expr>, Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
    In(Box<Expr>, Vec<Value>),
    IsNone(Box<Expr>),
    IsSome(Box<Expr>),
    ValueOr(Box<Expr>, Box<Expr>),
    Some(Box<Expr>),
    ToDecimal(Box<Expr>),
    Wrap(Box<Expr>),
    Unwrap(Box<Expr>),
    /// Narrows an exact value to the fixed-scale type `ty` with an explicit rounding.
    Rescale {
        arg: Box<Expr>,
        rounding: crate::exact::Rounding,
    },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub(crate) kind: ExprKind,
    pub(crate) ty: Type,
    pub(crate) hash: Hash,
    /// Source metadata; never part of the hash.
    pub(crate) loc: Loc,
}

impl Expr {
    pub(crate) fn new(kind: ExprKind, ty: Type, loc: Loc) -> Expr {
        let hash = hash::expr(&kind, &ty);
        Expr {
            kind,
            ty,
            hash,
            loc,
        }
    }

    pub fn kind(&self) -> &ExprKind {
        &self.kind
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}
