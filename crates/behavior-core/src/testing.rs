//! Support for the shared typing table (`tests/fixtures/typing_cases.json`).
//!
//! The same table is run against the Python DSL checker, so both checkers agree.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use crate::admit::hash;
use crate::semantic::types::{
    ArithOp, CmpOp, EnumInfo, NominalInfo, OpSig, Prim, Type, check_type, ops, type_of_op,
};
use crate::wire::{WType, decode_type};

struct Env {
    enums: BTreeMap<String, Arc<EnumInfo>>,
    nominals: BTreeMap<String, Arc<NominalInfo>>,
}

fn prim(t: &WType) -> Option<Prim> {
    Some(match t {
        WType::Bool => Prim::Bool,
        WType::Int => Prim::Int,
        WType::Decimal => Prim::Decimal,
        WType::String => Prim::String,
        _ => return None,
    })
}

fn env(v: &Value) -> Result<Env, String> {
    let mut enums = BTreeMap::new();
    for e in v["enums"].as_array().into_iter().flatten() {
        let name = e["name"].as_str().ok_or("enum name")?.to_string();
        let values: Vec<String> = e["values"]
            .as_array()
            .ok_or("enum values")?
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
        let h = hash::enum_decl(&name, &values);
        enums.insert(
            name.clone(),
            Arc::new(EnumInfo {
                name,
                values,
                hash: h,
            }),
        );
    }
    let mut nominals = BTreeMap::new();
    for n in v["nominals"].as_array().into_iter().flatten() {
        let name = n["name"].as_str().ok_or("nominal name")?.to_string();
        let underlying = decode_type(&n["underlying"], "$").map_err(|e| format!("{e:?}"))?;
        let underlying = prim(&underlying).ok_or("nominal underlying")?;
        let mut bits = 0;
        for op in n["ops"].as_array().into_iter().flatten() {
            bits |= ops::parse(op.as_str().unwrap_or_default()).ok_or("nominal op")?;
        }
        let h = hash::nominal_decl(&name, underlying, bits);
        nominals.insert(
            name.clone(),
            Arc::new(NominalInfo {
                name,
                underlying,
                ops: bits,
                scale: None,
                hash: h,
            }),
        );
    }
    Ok(Env { enums, nominals })
}

fn ty(env: &Env, t: &WType) -> Result<Type, String> {
    Ok(match t {
        WType::Bool => Type::Bool,
        WType::Int => Type::Int,
        WType::Decimal => Type::Decimal,
        WType::String => Type::String,
        WType::Option(inner) => Type::Option(Box::new(ty(env, inner)?)),
        WType::Enum(n) => Type::Enum(env.enums.get(n).ok_or("unknown enum")?.clone()),
        WType::Nominal(n) => Type::Nominal(env.nominals.get(n).ok_or("unknown nominal")?.clone()),
        WType::Exact(None) => Type::Exact(crate::semantic::types::Unit::Dimensionless),
        WType::Exact(Some(n)) => Type::Exact(crate::semantic::types::Unit::Nominal(
            env.nominals.get(n).ok_or("unknown nominal")?.clone(),
        )),
        WType::Id(e) => Type::Id(e.clone()),
        WType::Entity(e) => Type::Entity(e.clone()),
    })
}

/// Evaluates one typing case: `Ok(result type as wire JSON)` or `Err(error code)`.
pub fn typing_case(env_json: &Value, case: &Value) -> Result<Value, String> {
    let env = env(env_json)?;
    let operands: Vec<Type> = case["operands"]
        .as_array()
        .ok_or("operands")?
        .iter()
        .map(|o| {
            decode_type(o, "$")
                .map_err(|e| format!("{e:?}"))
                .and_then(|w| ty(&env, &w))
        })
        .collect::<Result<_, _>>()?;
    let op = case["op"].as_str().ok_or("op")?;
    if op == "check_type" {
        let t = operands.first().ok_or("operand")?;
        return check_type(t)
            .map(|_| t.to_wire_json())
            .map_err(|c| c.as_str().to_string());
    }
    let sig = match op {
        "eq" => OpSig::Cmp(CmpOp::Eq),
        "ne" => OpSig::Cmp(CmpOp::Ne),
        "lt" => OpSig::Cmp(CmpOp::Lt),
        "le" => OpSig::Cmp(CmpOp::Le),
        "gt" => OpSig::Cmp(CmpOp::Gt),
        "ge" => OpSig::Cmp(CmpOp::Ge),
        "add" => OpSig::Arith(ArithOp::Add),
        "sub" => OpSig::Arith(ArithOp::Sub),
        "mul" => OpSig::Arith(ArithOp::Mul),
        "div" => OpSig::Arith(ArithOp::Div),
        "and" => OpSig::And,
        "or" => OpSig::Or,
        "not" => OpSig::Not,
        "in" => OpSig::In {
            count: case["values"].as_u64().unwrap_or(0) as usize,
        },
        "is_none" => OpSig::IsNone,
        "is_some" => OpSig::IsSome,
        "value_or" => OpSig::ValueOr,
        "some" => OpSig::Some,
        "to_decimal" => OpSig::ToDecimal,
        "unwrap" => OpSig::Unwrap,
        "wrap" => {
            let target = case["target"].as_str().ok_or("target")?;
            OpSig::Wrap(env.nominals.get(target).ok_or("unknown target")?.clone())
        }
        other => return Err(format!("unknown op {other}")),
    };
    type_of_op(&sig, &operands)
        .map(|(t, _)| t.to_wire_json())
        .map_err(|c| c.as_str().to_string())
}
