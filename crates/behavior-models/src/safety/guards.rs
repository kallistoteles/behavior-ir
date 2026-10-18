//! Canonical expression probes built and admitted exclusively through public Core APIs.
//! This adapter neither rewrites the original module nor certifies a dependency.
use super::SafetyError;
use behavior_engine::{
    builder::{Builder, Node, ScopeSite, SemanticProfile},
    semantic::{Module, types::hash_display},
    wire::{DerivedKind, Loc, Role, WField, WLifecycle, WParam, WType},
};
use serde_json::json;
use std::collections::BTreeSet;

fn loc() -> Loc {
    Loc {
        file: "<safety-probe>".into(),
        line: 1,
    }
}
fn unsupported(e: impl std::fmt::Debug) -> SafetyError {
    SafetyError::new(
        "SAFETY_UNSUPPORTED_PROBE",
        format!("Public Core probe could not be established: {e:?}"),
    )
}

#[derive(Debug, Clone)]
pub enum CanonicalExpression {
    EnumLiteral {
        entity: String,
        field: String,
        member: String,
    },
    BoundId {
        binding: String,
    },
    Membership {
        binding: String,
        field: String,
        values: Vec<String>,
        negated: bool,
    },
    OwnerEquality {
        child: String,
        owner_field: String,
        parent: String,
    },
    NoNonterminalChildren {
        parent: String,
        child: String,
        owner_field: String,
        child_state: String,
        terminal: Vec<String>,
    },
}
#[derive(Debug, Clone)]
struct NominalDeclaration {
    name: String,
    underlying: WType,
    operations: Vec<String>,
    scale: Option<u64>,
}
#[derive(Debug, Clone)]
struct DeclarationContext {
    profile: SemanticProfile,
    enums: Vec<(String, Vec<String>)>,
    nominals: Vec<NominalDeclaration>,
    entities: Vec<(String, Vec<WField>)>,
}
impl DeclarationContext {
    fn builder(&self) -> Result<Builder, SafetyError> {
        let mut b = Builder::with_semantic_profile(self.profile);
        for (name, values) in &self.enums {
            b.declare_enum(name, values.clone(), loc())
                .map_err(unsupported)?;
        }
        for n in &self.nominals {
            b.declare_nominal(
                &n.name,
                n.underlying.clone(),
                n.operations.clone(),
                n.scale,
                loc(),
            )
            .map_err(unsupported)?;
        }
        for (name, fields) in &self.entities {
            b.declare_entity(name, fields.clone(), loc())
                .map_err(unsupported)?;
        }
        Ok(b)
    }
    fn extract(module: &Module) -> Result<Self, SafetyError> {
        let profile = module.semantic_profile();
        let enums = module
            .enums()
            .iter()
            .map(|(name, e)| (name.clone(), e.values.clone()))
            .collect();
        let mut nominals = Vec::new();
        for (name, n) in module.nominals() {
            let underlying = n.underlying.to_type().to_wire_type();
            let scale = n.scale.map(u64::from);
            // The facade exposes the nominal's identity, but not operation-bit
            // constants. Reconstruct by public declaration/admission and identity
            // comparison; never interpret its private bit encoding.
            let names = ["order", "add", "scale", "ratio"];
            let mut found = None;
            for subset in 0..(1_u8 << names.len()) {
                let operations = names
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| subset & (1 << i) != 0)
                    .map(|(_, s)| s.to_string())
                    .collect::<Vec<_>>();
                let mut probe = Builder::with_semantic_profile(profile);
                if probe
                    .declare_nominal(name, underlying.clone(), operations.clone(), scale, loc())
                    .is_err()
                {
                    continue;
                }
                if let Ok(admitted) = probe.finish(None)
                    && admitted
                        .nominals()
                        .get(name)
                        .is_some_and(|candidate| candidate.hash == n.hash)
                {
                    found = Some(operations);
                    break;
                }
            }
            let operations = found.ok_or_else(|| {
                unsupported(format!("No public nominal declaration reproduces {name}"))
            })?;
            nominals.push(NominalDeclaration {
                name: name.clone(),
                underlying,
                operations,
                scale,
            });
        }
        let mut entities = Vec::new();
        for (name, e) in module.entities() {
            let fields = e
                .fields()
                .iter()
                .filter(|(f, _)| f != "id")
                .map(|(name, t)| {
                    let mut ty = t.to_wire_type();
                    if e.is_reference(name) {
                        ty = reference(ty)?;
                    }
                    Ok(WField {
                        name: name.clone(),
                        ty,
                        loc: loc(),
                    })
                })
                .collect::<Result<Vec<_>, SafetyError>>()?;
            entities.push((name.clone(), fields));
        }
        Ok(Self {
            profile,
            enums,
            nominals,
            entities,
        })
    }
}
fn reference(ty: WType) -> Result<WType, SafetyError> {
    match ty {
        WType::Id(entity) => Ok(WType::Ref(entity)),
        WType::Option(t) => Ok(WType::Option(Box::new(reference(*t)?))),
        other => Err(unsupported(format!(
            "Reference marker on unexpected type {other:?}"
        ))),
    }
}

#[derive(Debug, Clone)]
pub struct CanonicalProbes {
    original: Module,
    context: DeclarationContext,
    declarations: Module,
}
impl CanonicalProbes {
    pub fn new(module: &Module) -> Result<Self, SafetyError> {
        let context = DeclarationContext::extract(module)?;
        let declarations = context.builder()?.finish(None).map_err(unsupported)?;
        for ((kind, name), hash) in module.name_table() {
            if ["entity", "enum", "nominal"].contains(&kind.as_str())
                && declarations.name_table().get(&(*kind, name.clone())) != Some(hash)
            {
                return Err(unsupported(format!(
                    "Reconstructed {name} declaration differs"
                )));
            }
        }
        for (name, entity) in module.entities() {
            let rebuilt = declarations.entity(name).ok_or_else(|| unsupported(name))?;
            for (field, ty) in entity.fields() {
                if rebuilt.field_type(field) != Some(ty)
                    || rebuilt.is_reference(field) != entity.is_reference(field)
                {
                    return Err(unsupported(format!(
                        "Reconstructed {name}.{field} type/reference marker differs"
                    )));
                }
            }
        }
        Ok(Self {
            original: module.clone(),
            context,
            declarations,
        })
    }
    pub fn declarations(&self) -> &Module {
        &self.declarations
    }
    pub fn mandatory_root(&self, action: &str, hash: &str) -> Result<bool, SafetyError> {
        let a = self
            .original
            .action(action)
            .ok_or_else(|| unsupported(format!("Unknown action {action}")))?;
        Ok(a.preconditions()
            .iter()
            .any(|p| hash_display(p.expr().hash()) == hash))
    }
    pub fn is_state_binding(
        &self,
        action: &str,
        param: &str,
        entity: &str,
    ) -> Result<bool, SafetyError> {
        let a = self
            .original
            .action(action)
            .ok_or_else(|| unsupported(format!("Unknown action {action}")))?;
        let Some(p) = a.params().iter().find(|p| p.name() == param) else {
            return Ok(false);
        };
        if p.ty().to_wire_type() != WType::Entity(entity.into()) {
            return Ok(false);
        }
        let mut b = self.context.builder()?;
        let params = vec![WParam {
            name: "_safety_state".into(),
            role: Some(Role::State),
            ty: WType::Entity(entity.into()),
        }];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .map_err(unsupported)?;
        let yes = b
            .lit(WType::Bool, json!(true), loc())
            .map_err(unsupported)?;
        b.add_action(
            "_safety_anchor",
            params,
            vec![(yes, loc())],
            vec![],
            vec![WLifecycle::Remove {
                param: "_safety_state".into(),
                loc: loc(),
            }],
            vec![],
            loc(),
        )
        .map_err(unsupported)?;
        b.pop_scope();
        let anchor = b.finish(None).map_err(unsupported)?;
        let state = anchor
            .action("_safety_anchor")
            .and_then(|a| a.params().first())
            .ok_or_else(|| unsupported("Missing State role anchor"))?;
        Ok(p.role() == state.role())
    }
    fn enum_field(&self, entity: &str, field: &str) -> Result<String, SafetyError> {
        let ty = self
            .declarations
            .entity(entity)
            .and_then(|e| e.field_type(field))
            .ok_or_else(|| unsupported(format!("Unknown {entity}.{field}")))?
            .to_wire_type();
        if let WType::Enum(name) = ty {
            Ok(name)
        } else {
            Err(unsupported("Lifecycle field is not an enum"))
        }
    }
    fn membership(
        &self,
        b: &mut Builder,
        entity: &str,
        binding: &str,
        field: &str,
        values: &[String],
        negated: bool,
    ) -> Result<Node, SafetyError> {
        let name = self.enum_field(entity, field)?;
        let enumeration = self
            .declarations
            .enums()
            .get(&name)
            .ok_or_else(|| unsupported("Missing enum declaration"))?;
        if values.iter().any(|v| !enumeration.values.contains(v)) {
            return Err(unsupported("Unknown lifecycle enum member"));
        }
        if values.is_empty() {
            return b
                .lit(WType::Bool, json!(negated), loc())
                .map_err(unsupported);
        }
        let values = values
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|v| json!(v))
            .collect();
        let value = b.field(binding, field, loc()).map_err(unsupported)?;
        let member = b.in_(value, values, loc()).map_err(unsupported)?;
        if negated {
            b.op("not", vec![member], loc()).map_err(unsupported)
        } else {
            Ok(member)
        }
    }
    pub fn expression(
        &self,
        captures: &[(String, String)],
        expression: &CanonicalExpression,
    ) -> Result<String, SafetyError> {
        let mut b = self.context.builder()?;
        let params = captures
            .iter()
            .map(|(name, entity)| WParam {
                name: name.clone(),
                role: None,
                ty: WType::Entity(entity.clone()),
            })
            .collect::<Vec<_>>();
        b.push_scope(ScopeSite::Derived, params.clone(), loc())
            .map_err(unsupported)?;
        let capture = |name: &str| {
            captures
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, e)| e.as_str())
                .ok_or_else(|| unsupported(format!("Unknown capture {name}")))
        };
        let body = match expression {
            CanonicalExpression::BoundId { binding } => {
                b.field(binding, "id", loc()).map_err(unsupported)?
            }
            CanonicalExpression::EnumLiteral {
                entity,
                field,
                member,
            } => {
                let name = self.enum_field(entity, field)?;
                b.lit(WType::Enum(name), json!(member), loc())
                    .map_err(unsupported)?
            }
            CanonicalExpression::Membership {
                binding,
                field,
                values,
                negated,
            } => self.membership(&mut b, capture(binding)?, binding, field, values, *negated)?,
            CanonicalExpression::OwnerEquality {
                child,
                owner_field,
                parent,
            } => {
                let owner = b.field(child, owner_field, loc()).map_err(unsupported)?;
                let id = b.field(parent, "id", loc()).map_err(unsupported)?;
                b.op("eq", vec![owner, id], loc()).map_err(unsupported)?
            }
            CanonicalExpression::NoNonterminalChildren {
                parent,
                child,
                owner_field,
                child_state,
                terminal,
            } => {
                let mut candidate = "_safety_candidate".to_owned();
                while captures.iter().any(|(name, _)| name == &candidate) {
                    candidate.push('_');
                }
                let query = b.select(child, loc()).map_err(unsupported)?;
                b.push_lambda(&candidate, child).map_err(unsupported)?;
                let owner = b
                    .field(&candidate, owner_field, loc())
                    .map_err(unsupported)?;
                let id = b.field(parent, "id", loc()).map_err(unsupported)?;
                let belongs = b.op("eq", vec![owner, id], loc()).map_err(unsupported)?;
                let nonterminal =
                    self.membership(&mut b, child, &candidate, child_state, terminal, true)?;
                let body = b
                    .op("and", vec![belongs, nonterminal], loc())
                    .map_err(unsupported)?;
                b.pop_scope();
                let any = b
                    .lambda("any", query, &candidate, body, loc())
                    .map_err(unsupported)?;
                b.op("not", vec![any], loc()).map_err(unsupported)?
            }
        };
        b.add_derived(
            "_safety_probe",
            DerivedKind::Derived,
            params,
            body,
            None,
            loc(),
        )
        .map_err(unsupported)?;
        b.pop_scope();
        let admitted = b.finish(None).map_err(unsupported)?;
        let item = admitted
            .derived("_safety_probe")
            .ok_or_else(|| unsupported("Missing admitted probe"))?;
        Ok(hash_display(item.body().hash()))
    }
}
