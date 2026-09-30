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
    /// `exists(id)`: `Id<T>` or `Option<Id<T>>` (an absent value gives false); on S before the
    /// effects, on S' after them (feature 006).
    Exists(Box<Expr>),
    /// `referenced(id)`: some surviving `Ref` field points at the identity (feature 006).
    Referenced(Box<Expr>),
    /// `count(query)` (feature 007).
    Count(Box<QueryNode>),
    /// A relational operator with a lambda over the candidate (feature 007). `param` is the
    /// source name of the lambda parameter (display and serialization only); in `body` the
    /// candidate is the parameter [`CANDIDATE`].
    Fold {
        op: FoldOp,
        query: Box<QueryNode>,
        param: String,
        body: Box<Expr>,
    },
}

/// The name of the candidate inside lambda bodies (not an identifier, so it cannot clash).
pub const CANDIDATE: &str = "$c";

/// Relational operators with a lambda (feature 007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FoldOp {
    Any,
    All,
    Sum,
    Min,
    Max,
    Unique,
}

impl FoldOp {
    pub fn as_str(self) -> &'static str {
        match self {
            FoldOp::Any => "any",
            FoldOp::All => "all",
            FoldOp::Sum => "sum",
            FoldOp::Min => "min",
            FoldOp::Max => "max",
            FoldOp::Unique => "unique",
        }
    }
}

/// Set algebra over two queries of the same entity type (feature 007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetOp {
    Union,
    Intersection,
    Difference,
}

impl SetOp {
    pub fn as_str(self) -> &'static str {
        match self {
            SetOp::Union => "union",
            SetOp::Intersection => "intersection",
            SetOp::Difference => "difference",
        }
    }
}

#[derive(Debug, Clone)]
pub enum QueryKind {
    /// Every existing entity of the type.
    Select,
    /// The members of `base` whose candidate satisfies `body` (`param` as in [`ExprKind::Fold`]).
    Where {
        base: Box<QueryNode>,
        param: String,
        body: Box<Expr>,
    },
    Set {
        op: SetOp,
        a: Box<QueryNode>,
        b: Box<QueryNode>,
    },
}

/// A query over one entity type: its membership is a pure predicate over a candidate, its
/// definition hash covers the type, the normalized predicate and the operations (feature 007).
#[derive(Debug, Clone)]
pub struct QueryNode {
    pub(crate) kind: QueryKind,
    pub(crate) entity: String,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

impl QueryNode {
    pub(crate) fn new(kind: QueryKind, entity: String, loc: Loc) -> QueryNode {
        let hash = hash::query(&kind, &entity);
        QueryNode {
            kind,
            entity,
            hash,
            loc,
        }
    }

    pub fn kind(&self) -> &QueryKind {
        &self.kind
    }

    pub fn entity(&self) -> &str {
        &self.entity
    }

    /// The definition hash.
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    pub fn loc(&self) -> &Loc {
        &self.loc
    }
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

impl Expr {
    /// The direct subexpressions, including the lambda bodies of relational forms and of the
    /// queries they range over.
    pub fn children(&self) -> Vec<&Expr> {
        match &self.kind {
            ExprKind::Lit(_)
            | ExprKind::Field { .. }
            | ExprKind::Param(_)
            | ExprKind::DerivedRef { .. } => Vec::new(),
            ExprKind::Cmp(_, a, b) | ExprKind::Arith(_, a, b) | ExprKind::ValueOr(a, b) => {
                vec![a, b]
            }
            ExprKind::And(xs) | ExprKind::Or(xs) => xs.iter().collect(),
            ExprKind::Not(a)
            | ExprKind::In(a, _)
            | ExprKind::IsNone(a)
            | ExprKind::IsSome(a)
            | ExprKind::Some(a)
            | ExprKind::ToDecimal(a)
            | ExprKind::Wrap(a)
            | ExprKind::Unwrap(a)
            | ExprKind::Rescale { arg: a, .. }
            | ExprKind::Exists(a)
            | ExprKind::Referenced(a) => vec![a],
            ExprKind::Count(q) => q.bodies(),
            ExprKind::Fold { query, body, .. } => {
                let mut out = query.bodies();
                out.push(body);
                out
            }
        }
    }
}

impl QueryNode {
    /// The lambda bodies of this query (its `where` predicates), outermost last.
    pub fn bodies(&self) -> Vec<&Expr> {
        match &self.kind {
            QueryKind::Select => Vec::new(),
            QueryKind::Where { base, body, .. } => {
                let mut out = base.bodies();
                out.push(body);
                out
            }
            QueryKind::Set { a, b, .. } => {
                let mut out = a.bodies();
                out.extend(b.bodies());
                out
            }
        }
    }
}
