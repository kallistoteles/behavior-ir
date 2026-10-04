//! Wire IR: the untrusted exchange form (contracts/ir-encoding.md).
//!
//! Decoding is strict: unknown keys, wrong shapes, and wrong arities are `DECODE_ERROR`s with
//! the JSON path of the problem. Wire IR carries names and source locations; it is never
//! hashed or evaluated directly (see `admit`).

use serde::Serialize;
use serde_json::{Map, Value};

/// The IR version without entity constraints (feature 001).
pub const IR_VERSION: &str = "0.1";
/// The IR version with entity constraints (feature 002).
pub const IR_VERSION_CONSTRAINTS: &str = "0.2";
/// The IR version with fixed-scale decimals, exact quantities, and rescale (feature 003).
pub const IR_VERSION_FIXED_SCALE: &str = "0.3";
/// The IR version with exact arithmetic closure (feature 004): the only version this engine
/// accepts. Earlier versions had rounded decimal arithmetic; the same document text must not
/// silently change meaning, so they are rejected.
pub const IR_VERSION_EXACT: &str = "0.4";
/// The IR version with entity lifecycle (feature 006): creation and removal effects, `exists`,
/// `referenced`, and `ref` field types. Any use of one of these forms needs 0.5; 0.4 documents
/// without them are still accepted (the extension changes no existing meaning).
pub const IR_VERSION_LIFECYCLE: &str = "0.5";
/// The IR version with relational queries (feature 007): `select`, `where`, set algebra, `count`,
/// `any`, `all`, `sum`, `min`, `max`, `unique`, and module-level invariants. Any use of one of
/// them needs 0.6; earlier documents without them are still accepted.
pub const IR_VERSION_QUERIES: &str = "0.6";
/// The IR version with declared reads and read documents (feature 010). A module with a `reads`
/// section needs 0.7; earlier documents without one are still accepted.
pub const IR_VERSION_READS: &str = "0.7";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Loc {
    pub file: String,
    pub line: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WType {
    Bool,
    Int,
    Decimal,
    String,
    Option(Box<WType>),
    Enum(String),
    Nominal(String),
    /// An exact value: of a decimal nominal (`Some(name)`) or dimensionless (`None`).
    Exact(Option<String>),
    Id(String),
    /// A reference field (wire 0.5): an `Id` whose target must exist (feature 006).
    Ref(String),
    Entity(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    State,
    Input,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedKind {
    Derived,
    Rule,
}

#[derive(Debug, Clone)]
pub struct WEnum {
    pub name: String,
    pub values: Vec<String>,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WNominal {
    pub name: String,
    pub underlying: WType,
    pub ops: Vec<String>,
    /// Fixed scale (wire 0.3); range-checked at admission.
    pub scale: Option<u64>,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WField {
    pub name: String,
    pub ty: WType,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WEntity {
    pub name: String,
    pub fields: Vec<WField>,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WParam {
    pub name: String,
    pub role: Option<Role>,
    pub ty: WType,
}

#[derive(Debug, Clone)]
pub struct WDerived {
    pub name: String,
    pub kind: DerivedKind,
    pub params: Vec<WParam>,
    pub body: WExpr,
    /// Declared type (wire 0.3); checked against the inferred type, never hashed.
    pub declared: Option<WType>,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WInvariant {
    pub name: String,
    pub entity: String,
    pub param: String,
    pub body: WExpr,
    pub loc: Loc,
}

/// A module-level invariant (wire 0.6): a closed state expression over the whole state, with no
/// entity parameter.
#[derive(Debug, Clone)]
pub struct WGlobalInvariant {
    pub name: String,
    pub body: WExpr,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WConstraint {
    pub name: String,
    pub entity: String,
    pub param: String,
    pub body: WExpr,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WCond {
    pub expr: WExpr,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WEffect {
    pub param: String,
    pub field: String,
    pub value: WExpr,
    pub loc: Loc,
}

/// A lifecycle effect (wire 0.5): creation with a complete initial value, or removal of the
/// entity bound to a state parameter.
#[derive(Debug, Clone)]
pub enum WLifecycle {
    Create {
        entity: String,
        id: WExpr,
        fields: Vec<(String, WExpr)>,
        loc: Loc,
    },
    Remove {
        param: String,
        loc: Loc,
    },
}

impl WLifecycle {
    pub fn loc(&self) -> &Loc {
        match self {
            WLifecycle::Create { loc, .. } | WLifecycle::Remove { loc, .. } => loc,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WAction {
    pub name: String,
    pub params: Vec<WParam>,
    pub preconditions: Vec<WCond>,
    pub effects: Vec<WEffect>,
    /// Lifecycle effects, in document order (serialized after the field effects).
    pub lifecycle: Vec<WLifecycle>,
    pub postconditions: Vec<WCond>,
    pub loc: Loc,
}

/// A projection item (wire 0.7): a stored field of the projected entity type, or a derived value
/// over it, named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WItem {
    Field(String),
    Derived(String),
}

/// The body of a read (wire 0.7): a value expression, or a projection of named items over a query
/// or over an entity parameter (`over`), with `param` naming the member.
#[derive(Debug, Clone)]
pub enum WReadBody {
    Value(WExpr),
    Project {
        over: WExpr,
        param: String,
        items: Vec<WItem>,
    },
}

/// A read (wire 0.7): a declared read in a module's `reads` section, or the read of a read
/// document (an ad-hoc read).
#[derive(Debug, Clone)]
pub struct WRead {
    pub name: String,
    pub params: Vec<WParam>,
    pub body: WReadBody,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WModule {
    pub enums: Vec<WEnum>,
    pub nominals: Vec<WNominal>,
    pub entities: Vec<WEntity>,
    pub derived: Vec<WDerived>,
    pub invariants: Vec<WInvariant>,
    /// Module-level invariants (wire 0.6), in document order.
    pub global_invariants: Vec<WGlobalInvariant>,
    pub constraints: Vec<WConstraint>,
    pub actions: Vec<WAction>,
    /// Declared reads (wire 0.7), in document order.
    pub reads: Vec<WRead>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpName {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
    Not,
    IsNone,
    IsSome,
    Some,
    ToDecimal,
    Unwrap,
    ValueOr,
    /// `exists(id)` (wire 0.5).
    Exists,
    /// `referenced(id)` (wire 0.5).
    Referenced,
    /// `count(query)` (wire 0.6).
    Count,
    /// Set algebra over two queries of the same entity type (wire 0.6).
    Union,
    Intersection,
    Difference,
}

/// A relational operator with a lambda over the candidate (wire 0.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LambdaOp {
    Where,
    Any,
    All,
    Sum,
    Min,
    Max,
    Unique,
}

impl LambdaOp {
    pub fn as_str(self) -> &'static str {
        match self {
            LambdaOp::Where => "where",
            LambdaOp::Any => "any",
            LambdaOp::All => "all",
            LambdaOp::Sum => "sum",
            LambdaOp::Min => "min",
            LambdaOp::Max => "max",
            LambdaOp::Unique => "unique",
        }
    }

    pub fn parse(s: &str) -> Option<LambdaOp> {
        Some(match s {
            "where" => LambdaOp::Where,
            "any" => LambdaOp::Any,
            "all" => LambdaOp::All,
            "sum" => LambdaOp::Sum,
            "min" => LambdaOp::Min,
            "max" => LambdaOp::Max,
            "unique" => LambdaOp::Unique,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub enum WExprKind {
    Lit {
        ty: WType,
        value: Value,
    },
    Field {
        param: String,
        field: String,
    },
    Param(String),
    Derived {
        name: String,
        args: Vec<String>,
    },
    Op {
        op: OpName,
        args: Vec<WExpr>,
    },
    In {
        arg: Box<WExpr>,
        values: Vec<Value>,
    },
    Wrap {
        nominal: String,
        arg: Box<WExpr>,
    },
    /// `rescale(arg, nominal, rounding)` (wire 0.3); the rounding name is checked at admission.
    Rescale {
        nominal: String,
        rounding: String,
        arg: Box<WExpr>,
    },
    /// `select(T)` (wire 0.6): the existing entities of type `T`, as a query.
    Select {
        entity: String,
    },
    /// A relational operator applied to a query with a lambda `param → body` over the candidate
    /// (wire 0.6).
    Lambda {
        op: LambdaOp,
        query: Box<WExpr>,
        param: String,
        body: Box<WExpr>,
    },
    /// `strict_unwrap(x)` (migration IR only, feature 009).
    StrictUnwrap(Box<WExpr>),
    /// `enum_map` / `strict_enum_map` (migration IR only, feature 009): `to` is the target enum.
    EnumMap {
        arg: Box<WExpr>,
        to: WType,
        mapping: Vec<(String, String)>,
        strict: bool,
    },
}

/// The name prefix of a target-side named type in a migration (feature 009): never an identifier,
/// so it cannot clash with a source name.
pub const TARGET_SIDE: &str = "target:";

/// A named type's name on the given side (`"target"` prefixes it with [`TARGET_SIDE`]).
fn sided(name: String, side: Option<&Value>, path: &str) -> R<String> {
    match side {
        None => Ok(name),
        Some(Value::String(s)) if s == "source" => Ok(name),
        Some(Value::String(s)) if s == "target" => Ok(format!("{TARGET_SIDE}{name}")),
        Some(_) => fail(path, "expected \"source\" or \"target\""),
    }
}

#[derive(Debug, Clone)]
pub struct WExpr {
    pub kind: WExprKind,
    pub loc: Loc,
}

/// A failed decode: either a structural problem or an unsupported format version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Structure {
        path: String,
        message: String,
    },
    UnsupportedVersion(String),
    /// A 0.4 document uses a form introduced by wire 0.5 (named).
    NeedsLifecycleVersion(String),
    /// A document below 0.6 uses a form introduced by wire 0.6 (named).
    NeedsQueryVersion(String),
    /// A document below 0.7 uses a form introduced by wire 0.7 (named).
    NeedsReadVersion(String),
}

pub(crate) type R<T> = Result<T, DecodeError>;

pub(crate) fn fail<T>(path: &str, message: impl Into<String>) -> R<T> {
    Err(DecodeError::Structure {
        path: path.to_string(),
        message: message.into(),
    })
}

/// A JSON object being decoded; `finish` rejects keys that were not consumed.
pub(crate) struct Obj<'a> {
    map: &'a Map<String, Value>,
    path: String,
    allowed: Vec<&'static str>,
}

impl<'a> Obj<'a> {
    pub(crate) fn new(v: &'a Value, path: &str) -> R<Obj<'a>> {
        match v {
            Value::Object(map) => Ok(Obj {
                map,
                path: path.to_string(),
                allowed: Vec::new(),
            }),
            _ => fail(path, "expected an object"),
        }
    }

    pub(crate) fn sub(&self, key: &str) -> String {
        format!("{}.{key}", self.path)
    }

    pub(crate) fn get(&mut self, key: &'static str) -> R<&'a Value> {
        self.allowed.push(key);
        match self.map.get(key) {
            Some(v) => Ok(v),
            None => fail(&self.path, format!("missing key `{key}`")),
        }
    }

    pub(crate) fn opt(&mut self, key: &'static str) -> Option<&'a Value> {
        self.allowed.push(key);
        self.map.get(key)
    }

    pub(crate) fn str(&mut self, key: &'static str) -> R<String> {
        let path = self.sub(key);
        match self.get(key)? {
            Value::String(s) => Ok(s.clone()),
            _ => fail(&path, "expected a string"),
        }
    }

    pub(crate) fn ident(&mut self, key: &'static str) -> R<String> {
        let path = self.sub(key);
        let s = self.str(key)?;
        if !is_identifier(&s) {
            return fail(&path, format!("`{s}` is not an identifier"));
        }
        Ok(s)
    }

    pub(crate) fn arr(&mut self, key: &'static str) -> R<&'a Vec<Value>> {
        let path = self.sub(key);
        match self.get(key)? {
            Value::Array(a) => Ok(a),
            _ => fail(&path, "expected an array"),
        }
    }

    pub(crate) fn loc(&mut self) -> R<Loc> {
        let path = self.sub("loc");
        let v = self.get("loc")?;
        decode_loc(v, &path)
    }

    pub(crate) fn finish(self) -> R<()> {
        for key in self.map.keys() {
            if !self.allowed.contains(&key.as_str()) {
                return fail(&self.path, format!("unknown key `{key}`"));
            }
        }
        Ok(())
    }
}

pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn decode_loc(v: &Value, path: &str) -> R<Loc> {
    let mut o = Obj::new(v, path)?;
    let file = o.str("file")?;
    let line_path = o.sub("line");
    let line = match o.get("line")? {
        Value::Number(n) => match n.as_u64() {
            Some(l) if l >= 1 => l,
            _ => return fail(&line_path, "expected a positive integer"),
        },
        _ => return fail(&line_path, "expected a positive integer"),
    };
    o.finish()?;
    Ok(Loc { file, line })
}

pub fn decode_type(v: &Value, path: &str) -> R<WType> {
    decode_type_in(v, path, false)
}

/// [`decode_type`]; in a migration (feature 009) a named type may carry `"side"`.
pub(crate) fn decode_type_in(v: &Value, path: &str, migration: bool) -> R<WType> {
    let mut o = Obj::new(v, path)?;
    let tag_path = o.sub("t");
    let t = o.str("t")?;
    let ty = match t.as_str() {
        "bool" => WType::Bool,
        "int" => WType::Int,
        "decimal" => WType::Decimal,
        "string" => WType::String,
        "option" => {
            let of_path = o.sub("of");
            WType::Option(Box::new(decode_type_in(o.get("of")?, &of_path, migration)?))
        }
        "enum" | "nominal" => {
            let name = o.ident("name")?;
            let name = if migration {
                let side_path = o.sub("side");
                sided(name, o.opt("side"), &side_path)?
            } else {
                name
            };
            if t == "enum" {
                WType::Enum(name)
            } else {
                WType::Nominal(name)
            }
        }
        "exact" => {
            let name_path = o.sub("name");
            let side = if migration { o.opt("side") } else { None };
            match o.opt("name") {
                None => WType::Exact(None),
                Some(Value::String(s)) if is_identifier(s) => {
                    WType::Exact(Some(sided(s.clone(), side, &name_path)?))
                }
                Some(_) => return fail(&name_path, "expected an identifier string"),
            }
        }
        "id" => WType::Id(o.ident("entity")?),
        "ref" => WType::Ref(o.ident("entity")?),
        "entity" => WType::Entity(o.ident("name")?),
        other => return fail(&tag_path, format!("unknown type `{other}`")),
    };
    o.finish()?;
    Ok(ty)
}

pub(crate) fn decode_list<T>(
    items: &[Value],
    path: &str,
    f: impl Fn(&Value, &str) -> R<T>,
) -> R<Vec<T>> {
    items
        .iter()
        .enumerate()
        .map(|(i, v)| f(v, &format!("{path}[{i}]")))
        .collect()
}

pub(crate) fn decode_strings(v: &Value, path: &str, identifiers: bool) -> R<Vec<String>> {
    match v {
        Value::Array(items) => decode_list(items, path, |item, p| match item {
            Value::String(s) if !identifiers || is_identifier(s) => Ok(s.clone()),
            _ => fail(p, "expected an identifier string"),
        }),
        _ => fail(path, "expected an array"),
    }
}

pub fn decode_param(v: &Value, path: &str) -> R<WParam> {
    let mut o = Obj::new(v, path)?;
    let name = o.ident("name")?;
    let type_path = o.sub("type");
    let ty = decode_type(o.get("type")?, &type_path)?;
    let role_path = o.sub("role");
    let role = match o.opt("role") {
        None => None,
        Some(Value::String(r)) => Some(match r.as_str() {
            "state" => Role::State,
            "input" => Role::Input,
            "context" => Role::Context,
            other => return fail(&role_path, format!("unknown role `{other}`")),
        }),
        Some(_) => return fail(&role_path, "expected a string"),
    };
    o.finish()?;
    Ok(WParam { name, role, ty })
}

pub(crate) fn op_name(s: &str) -> Option<OpName> {
    Some(match s {
        "eq" => OpName::Eq,
        "ne" => OpName::Ne,
        "lt" => OpName::Lt,
        "le" => OpName::Le,
        "gt" => OpName::Gt,
        "ge" => OpName::Ge,
        "add" => OpName::Add,
        "sub" => OpName::Sub,
        "mul" => OpName::Mul,
        "div" => OpName::Div,
        "and" => OpName::And,
        "or" => OpName::Or,
        "not" => OpName::Not,
        "is_none" => OpName::IsNone,
        "is_some" => OpName::IsSome,
        "some" => OpName::Some,
        "to_decimal" => OpName::ToDecimal,
        "unwrap" => OpName::Unwrap,
        "value_or" => OpName::ValueOr,
        "exists" => OpName::Exists,
        "referenced" => OpName::Referenced,
        "count" => OpName::Count,
        "union" => OpName::Union,
        "intersection" => OpName::Intersection,
        "difference" => OpName::Difference,
        _ => return None,
    })
}

pub(crate) fn arity_ok(op: OpName, n: usize) -> bool {
    use OpName::*;
    match op {
        And | Or => n >= 2,
        Not | IsNone | IsSome | Some | ToDecimal | Unwrap | Exists | Referenced | Count => n == 1,
        _ => n == 2,
    }
}

fn decode_expr(v: &Value, path: &str) -> R<WExpr> {
    decode_expr_in(v, path, false)
}

/// [`decode_expr`]; a migration expression (feature 009) may also use `strict_unwrap`,
/// `enum_map`, `strict_enum_map` and side-qualified named types.
pub(crate) fn decode_expr_in(v: &Value, path: &str, migration: bool) -> R<WExpr> {
    let mut o = Obj::new(v, path)?;
    let op_path = o.sub("op");
    let op = o.str("op")?;
    let loc = o.loc()?;
    let args_of = |o: &mut Obj, path: &str| -> R<Vec<WExpr>> {
        let args_path = o.sub("args");
        let _ = path;
        let items = o.arr("args")?;
        decode_list(items, &args_path, |v, p| decode_expr_in(v, p, migration))
    };
    let kind = match op.as_str() {
        "lit" => {
            let type_path = o.sub("type");
            let ty = decode_type_in(o.get("type")?, &type_path, migration)?;
            let value = o.get("value")?.clone();
            WExprKind::Lit { ty, value }
        }
        "field" => WExprKind::Field {
            param: o.ident("param")?,
            field: o.ident("field")?,
        },
        "param" => WExprKind::Param(o.ident("param")?),
        "derived" => {
            let name = o.ident("name")?;
            let args_path = o.sub("args");
            let args = decode_strings(o.get("args")?, &args_path, true)?;
            WExprKind::Derived { name, args }
        }
        "in" => {
            let mut args = args_of(&mut o, path)?;
            if args.len() != 1 {
                return fail(&o.sub("args"), "`in` takes exactly one argument");
            }
            let values_path = o.sub("values");
            let values = match o.get("values")? {
                Value::Array(items) => items.clone(),
                _ => return fail(&values_path, "expected an array"),
            };
            WExprKind::In {
                arg: Box::new(args.remove(0)),
                values,
            }
        }
        "wrap" => {
            let nominal = o.ident("nominal")?;
            let nominal = if migration {
                let side_path = o.sub("side");
                sided(nominal, o.opt("side"), &side_path)?
            } else {
                nominal
            };
            let mut args = args_of(&mut o, path)?;
            if args.len() != 1 {
                return fail(&o.sub("args"), "`wrap` takes exactly one argument");
            }
            WExprKind::Wrap {
                nominal,
                arg: Box::new(args.remove(0)),
            }
        }
        "select" => WExprKind::Select {
            entity: o.ident("entity")?,
        },
        name if LambdaOp::parse(name).is_some() => {
            let op = LambdaOp::parse(name).ok_or_else(|| DecodeError::Structure {
                path: op_path.clone(),
                message: "unknown operator".into(),
            })?;
            let mut args = args_of(&mut o, path)?;
            if args.len() != 1 {
                return fail(
                    &o.sub("args"),
                    format!("`{name}` takes exactly one query argument"),
                );
            }
            let param = o.ident("param")?;
            let body_path = o.sub("body");
            let body = decode_expr_in(o.get("body")?, &body_path, migration)?;
            WExprKind::Lambda {
                op,
                query: Box::new(args.remove(0)),
                param,
                body: Box::new(body),
            }
        }
        "rescale" => {
            let nominal = o.ident("nominal")?;
            let nominal = if migration {
                let side_path = o.sub("side");
                sided(nominal, o.opt("side"), &side_path)?
            } else {
                nominal
            };
            let rounding = o.str("rounding")?;
            let mut args = args_of(&mut o, path)?;
            if args.len() != 1 {
                return fail(&o.sub("args"), "`rescale` takes exactly one argument");
            }
            WExprKind::Rescale {
                nominal,
                rounding,
                arg: Box::new(args.remove(0)),
            }
        }
        "strict_unwrap" if migration => {
            let mut args = args_of(&mut o, path)?;
            if args.len() != 1 {
                return fail(&o.sub("args"), "`strict_unwrap` takes exactly one argument");
            }
            WExprKind::StrictUnwrap(Box::new(args.remove(0)))
        }
        "enum_map" | "strict_enum_map" if migration => {
            let mut args = args_of(&mut o, path)?;
            if args.len() != 1 {
                return fail(&o.sub("args"), format!("`{op}` takes exactly one argument"));
            }
            let to_path = o.sub("to");
            let to = decode_type_in(o.get("to")?, &to_path, migration)?;
            let mapping_path = o.sub("mapping");
            let Value::Array(items) = o.get("mapping")? else {
                return fail(&mapping_path, "expected an array of [source, target] pairs");
            };
            let mut mapping = Vec::new();
            for (i, item) in items.iter().enumerate() {
                match item.as_array().map(Vec::as_slice) {
                    Some([Value::String(a), Value::String(b)])
                        if is_identifier(a) && is_identifier(b) =>
                    {
                        mapping.push((a.clone(), b.clone()));
                    }
                    _ => {
                        return fail(
                            &format!("{mapping_path}[{i}]"),
                            "expected a [source value, target value] pair",
                        );
                    }
                }
            }
            WExprKind::EnumMap {
                arg: Box::new(args.remove(0)),
                to,
                mapping,
                strict: op == "strict_enum_map",
            }
        }
        other => {
            let Some(name) = op_name(other) else {
                return fail(&op_path, format!("unknown operator `{other}`"));
            };
            let args = args_of(&mut o, path)?;
            if !arity_ok(name, args.len()) {
                return fail(
                    &o.sub("args"),
                    format!("wrong number of arguments for `{other}`"),
                );
            }
            WExprKind::Op { op: name, args }
        }
    };
    o.finish()?;
    Ok(WExpr { kind, loc })
}

fn decode_cond(v: &Value, path: &str) -> R<WCond> {
    let mut o = Obj::new(v, path)?;
    let expr_path = o.sub("expr");
    let expr = decode_expr(o.get("expr")?, &expr_path)?;
    let loc = o.loc()?;
    o.finish()?;
    Ok(WCond { expr, loc })
}

/// A decoded entry of an action's `effects` array.
enum Decoded {
    Set(WEffect),
    Lifecycle(WLifecycle),
}

fn decode_effect(v: &Value, path: &str) -> R<Decoded> {
    if let Value::Object(map) = v {
        if map.contains_key("create") {
            let mut o = Obj::new(v, path)?;
            let entity = o.ident("create")?;
            let id_path = o.sub("id");
            let id = decode_expr(o.get("id")?, &id_path)?;
            let fields_path = o.sub("fields");
            let fields = match o.get("fields")? {
                Value::Object(m) => m
                    .iter()
                    .map(|(k, fv)| {
                        let fp = format!("{fields_path}.{k}");
                        if !is_identifier(k) {
                            return fail(&fp, format!("`{k}` is not an identifier"));
                        }
                        Ok((k.clone(), decode_expr(fv, &fp)?))
                    })
                    .collect::<R<Vec<_>>>()?,
                _ => return fail(&fields_path, "expected an object"),
            };
            let loc = o.loc()?;
            o.finish()?;
            return Ok(Decoded::Lifecycle(WLifecycle::Create {
                entity,
                id,
                fields,
                loc,
            }));
        }
        if map.contains_key("remove") {
            let mut o = Obj::new(v, path)?;
            let param = o.ident("remove")?;
            let loc = o.loc()?;
            o.finish()?;
            return Ok(Decoded::Lifecycle(WLifecycle::Remove { param, loc }));
        }
    }
    let mut o = Obj::new(v, path)?;
    let target_path = o.sub("target");
    let mut t = Obj::new(o.get("target")?, &target_path)?;
    let param = t.ident("param")?;
    let field = t.ident("field")?;
    t.finish()?;
    let value_path = o.sub("value");
    let value = decode_expr(o.get("value")?, &value_path)?;
    let loc = o.loc()?;
    o.finish()?;
    Ok(Decoded::Set(WEffect {
        param,
        field,
        value,
        loc,
    }))
}

/// A read (wire 0.7): `{name, params, body, loc}`.
fn decode_read(v: &Value, path: &str) -> R<WRead> {
    let mut o = Obj::new(v, path)?;
    let name = o.ident("name")?;
    let params_path = o.sub("params");
    let params = decode_list(o.arr("params")?, &params_path, decode_param)?;
    let body_path = o.sub("body");
    let body = decode_read_body(o.get("body")?, &body_path)?;
    let loc = o.loc()?;
    o.finish()?;
    Ok(WRead {
        name,
        params,
        body,
        loc,
    })
}

/// A read body: exactly one of `value` (an expression) and `project`.
fn decode_read_body(v: &Value, path: &str) -> R<WReadBody> {
    let mut b = Obj::new(v, path)?;
    let value = b.opt("value");
    let project = b.opt("project");
    b.finish()?;
    match (value, project) {
        (Some(e), None) => Ok(WReadBody::Value(decode_expr(e, &format!("{path}.value"))?)),
        (None, Some(p)) => {
            let project_path = format!("{path}.project");
            let mut o = Obj::new(p, &project_path)?;
            let over_path = o.sub("over");
            let over = decode_expr(o.get("over")?, &over_path)?;
            let param = o.ident("param")?;
            let items_path = o.sub("items");
            let items = decode_list(o.arr("items")?, &items_path, decode_item)?;
            o.finish()?;
            Ok(WReadBody::Project { over, param, items })
        }
        _ => fail(path, "a read body has exactly one of `value` and `project`"),
    }
}

/// A projection item: exactly one of `field` and `derived`, with a non-empty name.
fn decode_item(v: &Value, path: &str) -> R<WItem> {
    let mut o = Obj::new(v, path)?;
    let field = o.opt("field");
    let derived = o.opt("derived");
    o.finish()?;
    // Names are checked by admission: a reference path such as `customer.name` is refused there
    // as an unknown projection item, with guidance.
    match (field, derived) {
        (Some(Value::String(f)), None) if !f.is_empty() => Ok(WItem::Field(f.clone())),
        (None, Some(Value::String(d))) if !d.is_empty() => Ok(WItem::Derived(d.clone())),
        _ => fail(
            path,
            "a projection item is exactly one of `field` and `derived`, naming an item",
        ),
    }
}

/// Decodes a read document (wire 0.7): `{"ir_version": "0.7", "read": {...}}`, an ad-hoc read
/// admitted against a module on use.
pub fn decode_read_document(text: &str) -> R<WRead> {
    let root: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return fail("$", format!("invalid JSON: {e}")),
    };
    let mut o = Obj::new(&root, "$")?;
    match o.get("ir_version")? {
        Value::String(v) if v == IR_VERSION_READS => {}
        Value::String(v) => return Err(DecodeError::UnsupportedVersion(v.clone())),
        _ => return fail("$.ir_version", "expected a string"),
    }
    let read = decode_read(o.get("read")?, "$.read")?;
    o.finish()?;
    Ok(read)
}

/// Decodes a wire IR document. The version is checked before anything else.
pub fn decode_module(text: &str) -> R<WModule> {
    let root: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return fail("$", format!("invalid JSON: {e}")),
    };
    let mut o = Obj::new(&root, "$")?;
    let (with_constraints, fixed_scale, lifecycle, queries, reads_ok) = match o.get("ir_version")? {
        Value::String(v) if v == IR_VERSION_EXACT => (true, true, false, false, false),
        Value::String(v) if v == IR_VERSION_LIFECYCLE => (true, true, true, false, false),
        Value::String(v) if v == IR_VERSION_QUERIES => (true, true, true, true, false),
        Value::String(v) if v == IR_VERSION_READS => (true, true, true, true, true),
        Value::String(v) => return Err(DecodeError::UnsupportedVersion(v.clone())),
        _ => return fail("$.ir_version", "expected a string"),
    };

    let enums = decode_list(o.arr("enums")?, "$.enums", |v, p| {
        let mut e = Obj::new(v, p)?;
        let name = e.ident("name")?;
        let values_path = e.sub("values");
        let values = decode_strings(e.get("values")?, &values_path, false)?;
        let loc = e.loc()?;
        e.finish()?;
        Ok(WEnum { name, values, loc })
    })?;

    let nominals = decode_list(o.arr("nominals")?, "$.nominals", |v, p| {
        let mut n = Obj::new(v, p)?;
        let name = n.ident("name")?;
        let u_path = n.sub("underlying");
        let underlying = decode_type(n.get("underlying")?, &u_path)?;
        let ops_path = n.sub("ops");
        let ops = decode_strings(n.get("ops")?, &ops_path, true)?;
        let scale_path = n.sub("scale");
        let scale = match n.opt("scale") {
            None => None,
            Some(Value::Number(x)) if x.as_u64().is_some() => x.as_u64(),
            Some(_) => return fail(&scale_path, "expected a non-negative integer"),
        };
        let loc = n.loc()?;
        n.finish()?;
        Ok(WNominal {
            name,
            underlying,
            ops,
            scale,
            loc,
        })
    })?;

    let entities = decode_list(o.arr("entities")?, "$.entities", |v, p| {
        let mut e = Obj::new(v, p)?;
        let name = e.ident("name")?;
        let fields_path = e.sub("fields");
        let fields = decode_list(e.arr("fields")?, &fields_path, |fv, fp| {
            let mut f = Obj::new(fv, fp)?;
            let name = f.ident("name")?;
            let type_path = f.sub("type");
            let ty = decode_type(f.get("type")?, &type_path)?;
            let loc = f.loc()?;
            f.finish()?;
            Ok(WField { name, ty, loc })
        })?;
        let loc = e.loc()?;
        e.finish()?;
        Ok(WEntity { name, fields, loc })
    })?;

    let derived = decode_list(o.arr("derived")?, "$.derived", |v, p| {
        let mut d = Obj::new(v, p)?;
        let name = d.ident("name")?;
        let kind_path = d.sub("kind");
        let kind = match d.str("kind")?.as_str() {
            "derived" => DerivedKind::Derived,
            "rule" => DerivedKind::Rule,
            other => return fail(&kind_path, format!("unknown kind `{other}`")),
        };
        let params_path = d.sub("params");
        let params = decode_list(d.arr("params")?, &params_path, decode_param)?;
        let body_path = d.sub("body");
        let body = decode_expr(d.get("body")?, &body_path)?;
        let declared_path = d.sub("type");
        let declared = match d.opt("type") {
            None => None,
            Some(t) => Some(decode_type(t, &declared_path)?),
        };
        let loc = d.loc()?;
        d.finish()?;
        Ok(WDerived {
            name,
            kind,
            params,
            body,
            declared,
            loc,
        })
    })?;

    // An invariant without `entity` and `param` is a module-level invariant (wire 0.6).
    let mut invariants = Vec::new();
    let mut global_invariants = Vec::new();
    for d in decode_list(o.arr("invariants")?, "$.invariants", |v, p| {
        let mut i = Obj::new(v, p)?;
        let name = i.ident("name")?;
        let global = i.opt("entity").is_none() && i.opt("param").is_none();
        let (entity, param) = if global {
            (String::new(), String::new())
        } else {
            (i.ident("entity")?, i.ident("param")?)
        };
        let body_path = i.sub("body");
        let body = decode_expr(i.get("body")?, &body_path)?;
        let loc = i.loc()?;
        i.finish()?;
        Ok(if global {
            Err(WGlobalInvariant { name, body, loc })
        } else {
            Ok(WInvariant {
                name,
                entity,
                param,
                body,
                loc,
            })
        })
    })? {
        match d {
            Ok(i) => invariants.push(i),
            Err(g) => global_invariants.push(g),
        }
    }

    let constraints = if with_constraints {
        decode_list(o.arr("constraints")?, "$.constraints", |v, p| {
            let mut c = Obj::new(v, p)?;
            let name = c.ident("name")?;
            let entity = c.ident("entity")?;
            let param = c.ident("param")?;
            let body_path = c.sub("body");
            let body = decode_expr(c.get("body")?, &body_path)?;
            let loc = c.loc()?;
            c.finish()?;
            Ok(WConstraint {
                name,
                entity,
                param,
                body,
                loc,
            })
        })?
    } else {
        Vec::new()
    };

    let actions = decode_list(o.arr("actions")?, "$.actions", |v, p| {
        let mut a = Obj::new(v, p)?;
        let name = a.ident("name")?;
        let params_path = a.sub("params");
        let params = decode_list(a.arr("params")?, &params_path, decode_param)?;
        let pre_path = a.sub("preconditions");
        let preconditions = decode_list(a.arr("preconditions")?, &pre_path, decode_cond)?;
        let eff_path = a.sub("effects");
        let mut effects = Vec::new();
        let mut lifecycle = Vec::new();
        for d in decode_list(a.arr("effects")?, &eff_path, decode_effect)? {
            match d {
                Decoded::Set(e) => effects.push(e),
                Decoded::Lifecycle(l) => lifecycle.push(l),
            }
        }
        let post_path = a.sub("postconditions");
        let postconditions = decode_list(a.arr("postconditions")?, &post_path, decode_cond)?;
        let loc = a.loc()?;
        a.finish()?;
        Ok(WAction {
            name,
            params,
            preconditions,
            effects,
            lifecycle,
            postconditions,
            loc,
        })
    })?;

    let reads_present = o.opt("reads").is_some();
    let reads = match o.opt("reads") {
        None => Vec::new(),
        Some(Value::Array(items)) => decode_list(items, "$.reads", decode_read)?,
        Some(_) => return fail("$.reads", "expected an array"),
    };

    o.finish()?;
    if reads_present && !reads_ok {
        return Err(DecodeError::NeedsReadVersion("reads".into()));
    }
    let module = WModule {
        enums,
        nominals,
        entities,
        derived,
        invariants,
        global_invariants,
        constraints,
        actions,
        reads,
    };
    if !fixed_scale && uses_fixed_scale(&module) {
        return fail(
            "$.ir_version",
            "fixed-scale nominals, exact types, rescale, and declared derived types need ir_version \"0.3\"",
        );
    }
    if !queries && let Some(form) = query_form(&module) {
        return Err(DecodeError::NeedsQueryVersion(form.to_string()));
    }
    if !lifecycle && let Some(form) = lifecycle_form(&module) {
        return Err(DecodeError::NeedsLifecycleVersion(form.to_string()));
    }
    Ok(module)
}

/// Every expression of a module, for form detection.
fn module_exprs(m: &WModule) -> Vec<&WExpr> {
    let mut out: Vec<&WExpr> = Vec::new();
    out.extend(m.derived.iter().map(|d| &d.body));
    out.extend(m.invariants.iter().map(|i| &i.body));
    out.extend(m.global_invariants.iter().map(|i| &i.body));
    out.extend(m.constraints.iter().map(|c| &c.body));
    for a in &m.actions {
        out.extend(
            a.preconditions
                .iter()
                .chain(&a.postconditions)
                .map(|c| &c.expr),
        );
        out.extend(a.effects.iter().map(|e| &e.value));
        for l in &a.lifecycle {
            if let WLifecycle::Create { id, fields, .. } = l {
                out.push(id);
                out.extend(fields.iter().map(|(_, e)| e));
            }
        }
    }
    out
}

fn expr_query_form(e: &WExpr) -> Option<&'static str> {
    match &e.kind {
        WExprKind::Select { .. } => Some("select"),
        WExprKind::Lambda { op, .. } => Some(op.as_str()),
        WExprKind::Op {
            op: OpName::Count, ..
        } => Some("count"),
        WExprKind::Op {
            op: OpName::Union, ..
        } => Some("union"),
        WExprKind::Op {
            op: OpName::Intersection,
            ..
        } => Some("intersection"),
        WExprKind::Op {
            op: OpName::Difference,
            ..
        } => Some("difference"),
        WExprKind::Op { args, .. } => args.iter().find_map(expr_query_form),
        WExprKind::In { arg, .. }
        | WExprKind::Wrap { arg, .. }
        | WExprKind::Rescale { arg, .. }
        | WExprKind::StrictUnwrap(arg)
        | WExprKind::EnumMap { arg, .. } => expr_query_form(arg),
        WExprKind::Lit { .. }
        | WExprKind::Field { .. }
        | WExprKind::Param(_)
        | WExprKind::Derived { .. } => None,
    }
}

/// The first form introduced by wire 0.6 (feature 007) that a module uses, if any: such a module
/// needs, and serializes as, `"0.6"`.
pub fn query_form(m: &WModule) -> Option<&'static str> {
    if !m.global_invariants.is_empty() {
        return Some("module invariant");
    }
    module_exprs(m).into_iter().find_map(expr_query_form)
}

fn type_lifecycle_form(t: &WType) -> Option<&'static str> {
    match t {
        WType::Ref(_) => Some("ref"),
        WType::Option(inner) => type_lifecycle_form(inner),
        _ => None,
    }
}

fn expr_lifecycle_form(e: &WExpr) -> Option<&'static str> {
    match &e.kind {
        WExprKind::Op {
            op: OpName::Exists, ..
        } => Some("exists"),
        WExprKind::Op {
            op: OpName::Referenced,
            ..
        } => Some("referenced"),
        WExprKind::Op { args, .. } => args.iter().find_map(expr_lifecycle_form),
        WExprKind::Lit { ty, .. } => type_lifecycle_form(ty),
        WExprKind::In { arg, .. }
        | WExprKind::Wrap { arg, .. }
        | WExprKind::Rescale { arg, .. }
        | WExprKind::StrictUnwrap(arg)
        | WExprKind::EnumMap { arg, .. } => expr_lifecycle_form(arg),
        WExprKind::Lambda { query, body, .. } => {
            expr_lifecycle_form(query).or_else(|| expr_lifecycle_form(body))
        }
        WExprKind::Field { .. }
        | WExprKind::Param(_)
        | WExprKind::Derived { .. }
        | WExprKind::Select { .. } => None,
    }
}

/// The first form introduced by wire 0.5 (feature 006) that a module uses, if any: such a module
/// needs, and serializes as, `"0.5"`.
pub fn lifecycle_form(m: &WModule) -> Option<&'static str> {
    let params = |ps: &[WParam]| ps.iter().find_map(|p| type_lifecycle_form(&p.ty));
    fn exprs(xs: &mut dyn Iterator<Item = &WExpr>) -> Option<&'static str> {
        xs.map(expr_lifecycle_form).find(Option::is_some).flatten()
    }
    m.entities
        .iter()
        .flat_map(|e| &e.fields)
        .find_map(|f| type_lifecycle_form(&f.ty))
        .or_else(|| {
            m.derived.iter().find_map(|d| {
                params(&d.params)
                    .or_else(|| d.declared.as_ref().and_then(type_lifecycle_form))
                    .or_else(|| expr_lifecycle_form(&d.body))
            })
        })
        .or_else(|| exprs(&mut m.invariants.iter().map(|i| &i.body)))
        .or_else(|| exprs(&mut m.constraints.iter().map(|c| &c.body)))
        .or_else(|| {
            m.actions.iter().find_map(|a| {
                a.lifecycle
                    .first()
                    .map(|l| match l {
                        WLifecycle::Create { .. } => "create",
                        WLifecycle::Remove { .. } => "remove",
                    })
                    .or_else(|| params(&a.params))
                    .or_else(|| {
                        exprs(
                            &mut a
                                .preconditions
                                .iter()
                                .chain(&a.postconditions)
                                .map(|c| &c.expr)
                                .chain(a.effects.iter().map(|e| &e.value)),
                        )
                    })
            })
        })
}

fn type_uses_fixed_scale(t: &WType) -> bool {
    match t {
        WType::Exact(_) => true,
        WType::Option(inner) => type_uses_fixed_scale(inner),
        _ => false,
    }
}

fn expr_uses_fixed_scale(e: &WExpr) -> bool {
    match &e.kind {
        WExprKind::Rescale { .. } => true,
        WExprKind::Lit { ty, .. } => type_uses_fixed_scale(ty),
        WExprKind::Op { args, .. } => args.iter().any(expr_uses_fixed_scale),
        WExprKind::In { arg, .. }
        | WExprKind::Wrap { arg, .. }
        | WExprKind::StrictUnwrap(arg)
        | WExprKind::EnumMap { arg, .. } => expr_uses_fixed_scale(arg),
        WExprKind::Lambda { query, body, .. } => {
            expr_uses_fixed_scale(query) || expr_uses_fixed_scale(body)
        }
        WExprKind::Field { .. }
        | WExprKind::Param(_)
        | WExprKind::Derived { .. }
        | WExprKind::Select { .. } => false,
    }
}

/// Whether a module uses any wire 0.3 feature (it then needs, and serializes as, `"0.3"`).
pub fn uses_fixed_scale(m: &WModule) -> bool {
    let params = |ps: &[WParam]| ps.iter().any(|p| type_uses_fixed_scale(&p.ty));
    m.nominals.iter().any(|n| n.scale.is_some())
        || m.entities
            .iter()
            .any(|e| e.fields.iter().any(|f| type_uses_fixed_scale(&f.ty)))
        || m.derived
            .iter()
            .any(|d| d.declared.is_some() || params(&d.params) || expr_uses_fixed_scale(&d.body))
        || m.invariants.iter().any(|i| expr_uses_fixed_scale(&i.body))
        || m.global_invariants
            .iter()
            .any(|i| expr_uses_fixed_scale(&i.body))
        || m.constraints.iter().any(|c| expr_uses_fixed_scale(&c.body))
        || m.actions.iter().any(|a| {
            params(&a.params)
                || a.preconditions
                    .iter()
                    .chain(&a.postconditions)
                    .any(|c| expr_uses_fixed_scale(&c.expr))
                || a.effects.iter().any(|e| expr_uses_fixed_scale(&e.value))
                || a.lifecycle.iter().any(|l| match l {
                    WLifecycle::Create { id, fields, .. } => {
                        expr_uses_fixed_scale(id)
                            || fields.iter().any(|(_, e)| expr_uses_fixed_scale(e))
                    }
                    WLifecycle::Remove { .. } => false,
                })
        })
}
