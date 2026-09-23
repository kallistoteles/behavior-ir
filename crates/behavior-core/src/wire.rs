//! Wire IR: the untrusted exchange form (contracts/ir-encoding.md).
//!
//! Decoding is strict: unknown keys, wrong shapes, and wrong arities are `DECODE_ERROR`s with
//! the JSON path of the problem. Wire IR carries names and source locations; it is never
//! hashed or evaluated directly (see `admit`).

use serde::Serialize;
use serde_json::{Map, Value};

pub const IR_VERSION: &str = "0.1";

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
    Id(String),
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

#[derive(Debug, Clone)]
pub struct WAction {
    pub name: String,
    pub params: Vec<WParam>,
    pub preconditions: Vec<WCond>,
    pub effects: Vec<WEffect>,
    pub postconditions: Vec<WCond>,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct WModule {
    pub enums: Vec<WEnum>,
    pub nominals: Vec<WNominal>,
    pub entities: Vec<WEntity>,
    pub derived: Vec<WDerived>,
    pub invariants: Vec<WInvariant>,
    pub actions: Vec<WAction>,
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
}

#[derive(Debug, Clone)]
pub enum WExprKind {
    Lit { ty: WType, value: Value },
    Field { param: String, field: String },
    Param(String),
    Derived { name: String, args: Vec<String> },
    Op { op: OpName, args: Vec<WExpr> },
    In { arg: Box<WExpr>, values: Vec<Value> },
    Wrap { nominal: String, arg: Box<WExpr> },
}

#[derive(Debug, Clone)]
pub struct WExpr {
    pub kind: WExprKind,
    pub loc: Loc,
}

/// A failed decode: either a structural problem or an unsupported format version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Structure { path: String, message: String },
    UnsupportedVersion(String),
}

type R<T> = Result<T, DecodeError>;

fn fail<T>(path: &str, message: impl Into<String>) -> R<T> {
    Err(DecodeError::Structure {
        path: path.to_string(),
        message: message.into(),
    })
}

/// A JSON object being decoded; `finish` rejects keys that were not consumed.
struct Obj<'a> {
    map: &'a Map<String, Value>,
    path: String,
    allowed: Vec<&'static str>,
}

impl<'a> Obj<'a> {
    fn new(v: &'a Value, path: &str) -> R<Obj<'a>> {
        match v {
            Value::Object(map) => Ok(Obj {
                map,
                path: path.to_string(),
                allowed: Vec::new(),
            }),
            _ => fail(path, "expected an object"),
        }
    }

    fn sub(&self, key: &str) -> String {
        format!("{}.{key}", self.path)
    }

    fn get(&mut self, key: &'static str) -> R<&'a Value> {
        self.allowed.push(key);
        match self.map.get(key) {
            Some(v) => Ok(v),
            None => fail(&self.path, format!("missing key `{key}`")),
        }
    }

    fn opt(&mut self, key: &'static str) -> Option<&'a Value> {
        self.allowed.push(key);
        self.map.get(key)
    }

    fn str(&mut self, key: &'static str) -> R<String> {
        let path = self.sub(key);
        match self.get(key)? {
            Value::String(s) => Ok(s.clone()),
            _ => fail(&path, "expected a string"),
        }
    }

    fn ident(&mut self, key: &'static str) -> R<String> {
        let path = self.sub(key);
        let s = self.str(key)?;
        if !is_identifier(&s) {
            return fail(&path, format!("`{s}` is not an identifier"));
        }
        Ok(s)
    }

    fn arr(&mut self, key: &'static str) -> R<&'a Vec<Value>> {
        let path = self.sub(key);
        match self.get(key)? {
            Value::Array(a) => Ok(a),
            _ => fail(&path, "expected an array"),
        }
    }

    fn loc(&mut self) -> R<Loc> {
        let path = self.sub("loc");
        let v = self.get("loc")?;
        decode_loc(v, &path)
    }

    fn finish(self) -> R<()> {
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

fn decode_loc(v: &Value, path: &str) -> R<Loc> {
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
            WType::Option(Box::new(decode_type(o.get("of")?, &of_path)?))
        }
        "enum" => WType::Enum(o.ident("name")?),
        "nominal" => WType::Nominal(o.ident("name")?),
        "id" => WType::Id(o.ident("entity")?),
        "entity" => WType::Entity(o.ident("name")?),
        other => return fail(&tag_path, format!("unknown type `{other}`")),
    };
    o.finish()?;
    Ok(ty)
}

fn decode_list<T>(items: &[Value], path: &str, f: impl Fn(&Value, &str) -> R<T>) -> R<Vec<T>> {
    items
        .iter()
        .enumerate()
        .map(|(i, v)| f(v, &format!("{path}[{i}]")))
        .collect()
}

fn decode_strings(v: &Value, path: &str, identifiers: bool) -> R<Vec<String>> {
    match v {
        Value::Array(items) => decode_list(items, path, |item, p| match item {
            Value::String(s) if !identifiers || is_identifier(s) => Ok(s.clone()),
            _ => fail(p, "expected an identifier string"),
        }),
        _ => fail(path, "expected an array"),
    }
}

fn decode_param(v: &Value, path: &str) -> R<WParam> {
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

fn op_name(s: &str) -> Option<OpName> {
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
        _ => return None,
    })
}

fn arity_ok(op: OpName, n: usize) -> bool {
    use OpName::*;
    match op {
        And | Or => n >= 2,
        Not | IsNone | IsSome | Some | ToDecimal | Unwrap => n == 1,
        _ => n == 2,
    }
}

fn decode_expr(v: &Value, path: &str) -> R<WExpr> {
    let mut o = Obj::new(v, path)?;
    let op_path = o.sub("op");
    let op = o.str("op")?;
    let loc = o.loc()?;
    let args_of = |o: &mut Obj, path: &str| -> R<Vec<WExpr>> {
        let args_path = o.sub("args");
        let _ = path;
        let items = o.arr("args")?;
        decode_list(items, &args_path, decode_expr)
    };
    let kind = match op.as_str() {
        "lit" => {
            let type_path = o.sub("type");
            let ty = decode_type(o.get("type")?, &type_path)?;
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
            let mut args = args_of(&mut o, path)?;
            if args.len() != 1 {
                return fail(&o.sub("args"), "`wrap` takes exactly one argument");
            }
            WExprKind::Wrap {
                nominal,
                arg: Box::new(args.remove(0)),
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

fn decode_effect(v: &Value, path: &str) -> R<WEffect> {
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
    Ok(WEffect {
        param,
        field,
        value,
        loc,
    })
}

/// Decodes a wire IR document. The version is checked before anything else.
pub fn decode_module(text: &str) -> R<WModule> {
    let root: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return fail("$", format!("invalid JSON: {e}")),
    };
    let mut o = Obj::new(&root, "$")?;
    match o.get("ir_version")? {
        Value::String(v) if v == IR_VERSION => {}
        Value::String(v) => return Err(DecodeError::UnsupportedVersion(v.clone())),
        _ => return fail("$.ir_version", "expected a string"),
    }

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
        let loc = n.loc()?;
        n.finish()?;
        Ok(WNominal {
            name,
            underlying,
            ops,
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
        let loc = d.loc()?;
        d.finish()?;
        Ok(WDerived {
            name,
            kind,
            params,
            body,
            loc,
        })
    })?;

    let invariants = decode_list(o.arr("invariants")?, "$.invariants", |v, p| {
        let mut i = Obj::new(v, p)?;
        let name = i.ident("name")?;
        let entity = i.ident("entity")?;
        let param = i.ident("param")?;
        let body_path = i.sub("body");
        let body = decode_expr(i.get("body")?, &body_path)?;
        let loc = i.loc()?;
        i.finish()?;
        Ok(WInvariant {
            name,
            entity,
            param,
            body,
            loc,
        })
    })?;

    let actions = decode_list(o.arr("actions")?, "$.actions", |v, p| {
        let mut a = Obj::new(v, p)?;
        let name = a.ident("name")?;
        let params_path = a.sub("params");
        let params = decode_list(a.arr("params")?, &params_path, decode_param)?;
        let pre_path = a.sub("preconditions");
        let preconditions = decode_list(a.arr("preconditions")?, &pre_path, decode_cond)?;
        let eff_path = a.sub("effects");
        let effects = decode_list(a.arr("effects")?, &eff_path, decode_effect)?;
        let post_path = a.sub("postconditions");
        let postconditions = decode_list(a.arr("postconditions")?, &post_path, decode_cond)?;
        let loc = a.loc()?;
        a.finish()?;
        Ok(WAction {
            name,
            params,
            preconditions,
            effects,
            postconditions,
            loc,
        })
    })?;

    o.finish()?;
    Ok(WModule {
        enums,
        nominals,
        entities,
        derived,
        invariants,
        actions,
    })
}
