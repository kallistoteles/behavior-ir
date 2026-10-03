//! The migration document, migration IR 0.1 (contracts/migration-wire.md): decoding into its wire
//! form. Expressions use the module wire IR plus the migration operators and side-qualified named
//! types.

use serde_json::Value;

use crate::wire::{
    DecodeError, Loc, Obj, R, WExpr, WType, decode_expr_in, decode_list, decode_loc,
    decode_strings, decode_type_in, fail,
};

/// The migration IR version this engine reads and writes.
pub const MIGRATION_IR_VERSION: &str = "0.1";

/// A named, typed literal usable in transforms and requirements.
#[derive(Debug, Clone)]
pub struct WConstant {
    pub name: String,
    pub ty: WType,
    pub value: Value,
    pub loc: Loc,
}

/// A source requirement: a closed Bool expression over the source state.
#[derive(Debug, Clone)]
pub struct WRequirement {
    pub name: String,
    pub body: WExpr,
    pub loc: Loc,
}

/// How one target field is computed.
#[derive(Debug, Clone)]
pub enum WFieldSpec {
    Expr(WExpr),
    /// `{"copy": f}`: the source field `f`, unchanged.
    Copy(String),
}

/// The transform of one entity type.
#[derive(Debug, Clone)]
pub struct WTransform {
    pub entity: String,
    pub fields: Vec<(String, WFieldSpec)>,
    pub drops: Vec<String>,
    pub loc: Option<Loc>,
}

/// A decoded migration document.
#[derive(Debug, Clone)]
pub struct WMigration {
    pub name: String,
    pub source: String,
    pub target: String,
    pub constants: Vec<WConstant>,
    pub requirements: Vec<WRequirement>,
    pub transforms: Vec<WTransform>,
    pub retire: Vec<String>,
}

fn schema_hash(o: &mut Obj<'_>, key: &'static str) -> R<String> {
    let path = o.sub(key);
    let s = o.str(key)?;
    let hex = s.strip_prefix("sha256:").unwrap_or_default();
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return fail(
            &path,
            format!("`{s}` is not a SchemaHash (sha256:<64 hex digits>)"),
        );
    }
    Ok(s)
}

fn decode_constant(v: &Value, path: &str) -> R<WConstant> {
    let mut o = Obj::new(v, path)?;
    let name = o.ident("name")?;
    let type_path = o.sub("type");
    let ty = decode_type_in(o.get("type")?, &type_path, true)?;
    let value = o.get("value")?.clone();
    let loc = o.loc()?;
    o.finish()?;
    Ok(WConstant {
        name,
        ty,
        value,
        loc,
    })
}

fn decode_requirement(v: &Value, path: &str) -> R<WRequirement> {
    let mut o = Obj::new(v, path)?;
    let name = o.ident("name")?;
    let body_path = o.sub("body");
    let body = decode_expr_in(o.get("body")?, &body_path, true)?;
    let loc = o.loc()?;
    o.finish()?;
    Ok(WRequirement { name, body, loc })
}

fn decode_field(v: &Value, path: &str) -> R<WFieldSpec> {
    match v {
        Value::Object(m) if m.contains_key("copy") => {
            let mut o = Obj::new(v, path)?;
            let from = o.ident("copy")?;
            o.finish()?;
            Ok(WFieldSpec::Copy(from))
        }
        _ => Ok(WFieldSpec::Expr(decode_expr_in(v, path, true)?)),
    }
}

fn decode_transform(v: &Value, path: &str) -> R<WTransform> {
    let mut o = Obj::new(v, path)?;
    let entity = o.ident("entity")?;
    let fields_path = o.sub("fields");
    let Value::Object(map) = o.get("fields")? else {
        return fail(
            &fields_path,
            "expected an object of target field → expression or copy",
        );
    };
    let mut fields = Vec::new();
    for (name, spec) in map {
        if !crate::wire::is_identifier(name) {
            return fail(&fields_path, format!("`{name}` is not an identifier"));
        }
        fields.push((
            name.clone(),
            decode_field(spec, &format!("{fields_path}.{name}"))?,
        ));
    }
    let drops_path = o.sub("drops");
    let drops = match o.opt("drops") {
        None => Vec::new(),
        Some(d) => decode_strings(d, &drops_path, true)?,
    };
    let loc_path = o.sub("loc");
    let loc = match o.opt("loc") {
        None => None,
        Some(l) => Some(decode_loc(l, &loc_path)?),
    };
    o.finish()?;
    Ok(WTransform {
        entity,
        fields,
        drops,
        loc,
    })
}

/// Decodes a migration document (migration IR 0.1).
pub fn decode_migration(text: &str) -> R<WMigration> {
    let v: Value = serde_json::from_str(text).map_err(|e| DecodeError::Structure {
        path: "$".into(),
        message: format!("not JSON: {e}"),
    })?;
    let mut o = Obj::new(&v, "$")?;
    let version_path = o.sub("migration_ir");
    let version = o.str("migration_ir")?;
    if version != MIGRATION_IR_VERSION {
        return fail(
            &version_path,
            format!(
                "unsupported migration_ir `{version}`; this engine reads `{MIGRATION_IR_VERSION}`"
            ),
        );
    }
    let name = o.ident("name")?;
    let source = schema_hash(&mut o, "source")?;
    let target = schema_hash(&mut o, "target")?;
    let list = |o: &mut Obj<'_>, key: &'static str| -> R<Vec<Value>> {
        Ok(match o.opt(key) {
            None => Vec::new(),
            Some(Value::Array(a)) => a.clone(),
            Some(_) => return fail(&o.sub(key), "expected an array"),
        })
    };
    let constants = list(&mut o, "constants")?;
    let constants = decode_list(&constants, "$.constants", decode_constant)?;
    let requirements = list(&mut o, "requirements")?;
    let requirements = decode_list(&requirements, "$.requirements", decode_requirement)?;
    let transforms = o.arr("transforms")?.clone();
    let transforms = decode_list(&transforms, "$.transforms", decode_transform)?;
    let retire = match o.opt("retire") {
        None => Vec::new(),
        Some(r) => decode_strings(r, "$.retire", true)?,
    };
    o.finish()?;
    Ok(WMigration {
        name,
        source,
        target,
        constants,
        requirements,
        transforms,
        retire,
    })
}
