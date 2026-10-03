//! The construction API behind the Python binding (research R17).
//!
//! The builder records declarations and expression trees and checks every new node with the
//! same type checker as admission (`admit::typecheck`), so authors get errors where they write
//! an expression. `finish` hands the collected module to `admit_wire`, the same entry point JSON
//! decoding uses: there is one way into the semantic IR, whichever frontend is used.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde_json::Value as Json;

use crate::admit::resolve::{Decls, ParamSite, declarations, params};
use crate::admit::typecheck::{check_expr, derived_item, effect_target};
use crate::admit::{AdmissionError, AdmissionResult, admit_wire};
use crate::semantic::module::{DerivedItem, Module, Param, ParamRole};
use crate::semantic::types::{Type, coerce};
use crate::wire::{
    DerivedKind, Loc, WAction, WCond, WConstraint, WDerived, WEffect, WEntity, WEnum, WExpr,
    WExprKind, WField, WGlobalInvariant, WInvariant, WLifecycle, WModule, WNominal, WParam, WType,
    arity_ok, op_name,
};

/// A construction error: the same codes as admission errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildError {
    pub code: String,
    pub message: String,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl From<AdmissionError> for BuildError {
    fn from(e: AdmissionError) -> Self {
        BuildError {
            code: e.code,
            message: e.message,
        }
    }
}

fn err(code: &str, message: impl Into<String>) -> BuildError {
    BuildError {
        code: code.to_string(),
        message: message.into(),
    }
}

type R<T> = Result<T, BuildError>;

/// Where a parameter scope comes from (invariants use `Derived`: one read parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeSite {
    Derived,
    Action,
    /// A module invariant (feature 007): no parameters at all.
    Closed,
}

/// A typed expression node. `ty` is `None` for a node that depends on a derived value that is
/// still being traced (a cycle); such nodes skip the immediate check and `finish` reports.
#[derive(Debug, Clone)]
pub struct Node {
    pub(crate) w: WExpr,
    pub(crate) ty: Option<Type>,
    pub(crate) role: Option<ParamRole>,
    /// The entity type of a query node (feature 007): a set of entities, not a value.
    pub(crate) query: Option<String>,
}

impl Node {
    /// The node's type in wire JSON form, or `None` if it is untyped (below a cycle reference).
    pub fn type_wire_json(&self) -> Option<Json> {
        self.ty.as_ref().map(Type::to_wire_json)
    }

    /// The node's type in wire form, or `None` if it is untyped.
    pub fn wire_type(&self) -> Option<WType> {
        self.ty.as_ref().map(Type::to_wire_type)
    }

    /// The node's wire expression (for lifecycle effects built from nodes, feature 006).
    pub fn into_wire(self) -> WExpr {
        self.w
    }

    /// The entity type of a query node (feature 007), or `None` for a value.
    pub fn query_entity(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// The parameter role of `field`/`param` nodes: `state`, `input`, `context`, or `read`.
    pub fn role(&self) -> Option<&'static str> {
        self.role.map(|r| match r {
            ParamRole::State => "state",
            ParamRole::Input => "input",
            ParamRole::Context => "context",
            ParamRole::Read => "read",
        })
    }
}

#[derive(Default)]
pub struct Builder {
    enums: Vec<WEnum>,
    nominals: Vec<WNominal>,
    entities: Vec<WEntity>,
    derived: Vec<WDerived>,
    invariants: Vec<WInvariant>,
    global_invariants: Vec<WGlobalInvariant>,
    constraints: Vec<WConstraint>,
    actions: Vec<WAction>,
    decls: Option<Decls>,
    typed_derived: BTreeMap<String, DerivedItem>,
    scopes: Vec<Vec<Param>>,
    /// Set when the builder builds a migration (feature 009) instead of a module.
    migration: Option<Box<MigrationParts>>,
}

/// A migration being built (feature 009): its modules and the parts collected so far.
struct MigrationParts {
    source: Module,
    target: Module,
    transforms: BTreeMap<String, crate::migration::wire::WTransform>,
    requirements: Vec<crate::migration::wire::WRequirement>,
    retire: Vec<String>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    /// A builder for a migration from `source` to `target` (feature 009): expressions are typed
    /// over both schemas (target-side named types under [`crate::wire::TARGET_SIDE`]) and the
    /// source module's derived values.
    pub fn for_migration(source: &Module, target: &Module) -> Self {
        Builder {
            decls: Some(crate::migration::merged_decls(source, target)),
            typed_derived: source.derived.clone(),
            migration: Some(Box::new(MigrationParts {
                source: source.clone(),
                target: target.clone(),
                transforms: BTreeMap::new(),
                requirements: Vec::new(),
                retire: Vec::new(),
            })),
            ..Self::default()
        }
    }

    fn parts(&mut self) -> R<&mut MigrationParts> {
        self.migration
            .as_deref_mut()
            .ok_or_else(|| err("DECODE_ERROR", "this builder does not build a migration"))
    }

    /// Opens the scope of a transform of `entity`: the old entity `old` (feature 009).
    pub fn push_transform(&mut self, entity: &str, loc: Loc) -> R<()> {
        let parts = self.parts()?;
        if parts.source.entity(entity).is_none() {
            return Err(err(
                "INVALID_TRANSFORM",
                format!("`{entity}` is not an entity type of the source schema"),
            ));
        }
        parts
            .transforms
            .entry(entity.to_string())
            .or_insert_with(|| crate::migration::wire::WTransform {
                entity: entity.to_string(),
                fields: Vec::new(),
                drops: Vec::new(),
                loc: Some(loc),
            });
        self.scopes.push(vec![Param {
            name: crate::migration::OLD.into(),
            role: ParamRole::Read,
            ty: Type::Entity(entity.into()),
        }]);
        Ok(())
    }

    /// Opens the scope of a source requirement: a closed expression (feature 009).
    pub fn push_requirement(&mut self) -> R<()> {
        self.parts()?;
        self.scopes.push(Vec::new());
        Ok(())
    }

    /// `strict_unwrap(arg)` (migrations only, feature 009).
    pub fn strict_unwrap(&mut self, arg: Node, loc: Loc) -> R<Node> {
        self.parts()?;
        let w = WExpr {
            kind: WExprKind::StrictUnwrap(Box::new(arg.w.clone())),
            loc,
        };
        self.make(w, &[&arg], None)
    }

    /// `enum_map` / `strict_enum_map` (migrations only, feature 009).
    pub fn enum_map(
        &mut self,
        arg: Node,
        to: WType,
        mapping: Vec<(String, String)>,
        strict: bool,
        loc: Loc,
    ) -> R<Node> {
        self.parts()?;
        let w = WExpr {
            kind: WExprKind::EnumMap {
                arg: Box::new(arg.w.clone()),
                to,
                mapping,
                strict,
            },
            loc,
        };
        self.make(w, &[&arg], None)
    }

    /// Assigns a target field of `entity` in its transform.
    pub fn set_field(&mut self, entity: &str, field: &str, value: Node) -> R<()> {
        let t = self.transform_mut(entity)?;
        t.fields.retain(|(f, _)| f != field);
        t.fields.push((
            field.to_string(),
            crate::migration::wire::WFieldSpec::Expr(value.w),
        ));
        Ok(())
    }

    /// Acknowledges that the source field `field` of `entity` is dropped.
    pub fn drop_field(&mut self, entity: &str, field: &str) -> R<()> {
        let t = self.transform_mut(entity)?;
        if !t.drops.iter().any(|d| d == field) {
            t.drops.push(field.to_string());
        }
        Ok(())
    }

    fn transform_mut(&mut self, entity: &str) -> R<&mut crate::migration::wire::WTransform> {
        let parts = self.parts()?;
        Ok(parts
            .transforms
            .entry(entity.to_string())
            .or_insert_with(|| crate::migration::wire::WTransform {
                entity: entity.to_string(),
                fields: Vec::new(),
                drops: Vec::new(),
                loc: None,
            }))
    }

    /// Adds a source requirement (feature 009).
    pub fn add_requirement(&mut self, name: &str, body: Node, loc: Loc) -> R<()> {
        self.check_condition(&body, &format!("requirement `{name}`"))?;
        self.parts()?
            .requirements
            .push(crate::migration::wire::WRequirement {
                name: name.to_string(),
                body: body.w,
                loc,
            });
        Ok(())
    }

    /// Retires an entity type of the source schema (feature 009).
    pub fn retire(&mut self, entity: &str) -> R<()> {
        let parts = self.parts()?;
        if !parts.retire.iter().any(|r| r == entity) {
            parts.retire.push(entity.to_string());
        }
        Ok(())
    }

    /// Admits the collected migration through the same pipeline as migration documents.
    pub fn finish_migration(
        &self,
        name: &str,
    ) -> Result<crate::migration::Migration, AdmissionResult> {
        let Some(parts) = self.migration.as_deref() else {
            return Err(AdmissionResult::failed(vec![AdmissionError::new(
                "DECODE_ERROR",
                "this builder does not build a migration",
                None,
            )]));
        };
        let w = crate::migration::wire::WMigration {
            name: name.to_string(),
            source: crate::schema(&parts.source).hash,
            target: crate::schema(&parts.target).hash,
            constants: Vec::new(),
            requirements: parts.requirements.clone(),
            transforms: parts.transforms.values().cloned().collect(),
            retire: parts.retire.clone(),
        };
        crate::migration::admit_migration_wire(&parts.source, &parts.target, &w)
    }

    // --- declarations ------------------------------------------------------------------

    /// Declares an enum; declaring the same enum again is a no-op.
    pub fn declare_enum(&mut self, name: &str, values: Vec<String>, loc: Loc) -> R<()> {
        if let Some(existing) = self.enums.iter().find(|e| e.name == name) {
            return if existing.values == values {
                Ok(())
            } else {
                Err(err(
                    "DUPLICATE_NAME",
                    format!("enum `{name}` is declared twice"),
                ))
            };
        }
        self.enums.push(WEnum {
            name: name.into(),
            values,
            loc,
        });
        self.decls = None;
        Ok(())
    }

    /// Declares a nominal type; declaring the same nominal type again is a no-op.
    /// Declares a nominal type; `scale` makes a decimal-based nominal fixed-scale (0–28).
    pub fn declare_nominal(
        &mut self,
        name: &str,
        underlying: WType,
        mut ops: Vec<String>,
        scale: Option<u64>,
        loc: Loc,
    ) -> R<()> {
        ops.sort();
        ops.dedup();
        if let Some(existing) = self.nominals.iter().find(|n| n.name == name) {
            return if existing.underlying == underlying && existing.ops == ops {
                Ok(())
            } else {
                Err(err(
                    "DUPLICATE_NAME",
                    format!("nominal type `{name}` is declared twice"),
                ))
            };
        }
        self.nominals.push(WNominal {
            name: name.into(),
            underlying,
            ops,
            scale,
            loc,
        });
        self.decls = None;
        Ok(())
    }

    /// Declares an entity. Declarations are checked together on first use.
    pub fn declare_entity(&mut self, name: &str, fields: Vec<WField>, loc: Loc) -> R<()> {
        if self.entities.iter().any(|e| e.name == name) {
            return Err(err(
                "DUPLICATE_NAME",
                format!("entity `{name}` is declared twice"),
            ));
        }
        self.entities.push(WEntity {
            name: name.into(),
            fields,
            loc,
        });
        self.decls = None;
        Ok(())
    }

    fn ensure_decls(&mut self) -> R<()> {
        if self.decls.is_some() {
            return Ok(());
        }
        let model = WModule {
            enums: self.enums.clone(),
            nominals: self.nominals.clone(),
            entities: self.entities.clone(),
            derived: Vec::new(),
            invariants: Vec::new(),
            global_invariants: Vec::new(),
            constraints: Vec::new(),
            actions: Vec::new(),
        };
        let mut errors = Vec::new();
        let decls = declarations(&model, &mut errors);
        if let Some(first) = errors.into_iter().next() {
            return Err(first.into());
        }
        self.decls = Some(decls);
        Ok(())
    }

    // --- scopes ------------------------------------------------------------------------

    pub fn push_scope(&mut self, site: ScopeSite, ps: Vec<WParam>, loc: Loc) -> R<()> {
        self.ensure_decls()?;
        if site == ScopeSite::Closed {
            if !ps.is_empty() {
                return Err(err(
                    "ARITY_MISMATCH",
                    "a module invariant has no parameters",
                ));
            }
            self.scopes.push(Vec::new());
            return Ok(());
        }
        let Some(decls) = &self.decls else {
            return Err(err("DECODE_ERROR", "no declarations"));
        };
        let site = match site {
            ScopeSite::Derived => ParamSite::Derived,
            // Whether the action creates is known only when it is added (`add_action` checks).
            ScopeSite::Action => ParamSite::CreatingAction,
            ScopeSite::Closed => ParamSite::Derived,
        };
        let mut errors = Vec::new();
        match params(decls, &ps, site, &loc, &mut errors) {
            Some(resolved) => {
                self.scopes.push(resolved);
                Ok(())
            }
            None => Err(errors
                .into_iter()
                .next()
                .map(Into::into)
                .unwrap_or_else(|| err("DECODE_ERROR", "invalid parameters"))),
        }
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn scope(&self) -> &[Param] {
        self.scopes.last().map(Vec::as_slice).unwrap_or(&[])
    }

    // --- nodes -------------------------------------------------------------------------

    fn make(&mut self, w: WExpr, children: &[&Node], role: Option<ParamRole>) -> R<Node> {
        if children.iter().any(|c| c.ty.is_none() && c.query.is_none()) {
            return Ok(Node {
                w,
                ty: None,
                role,
                query: None,
            });
        }
        self.ensure_decls()?;
        let Some(decls) = &self.decls else {
            return Err(err("DECODE_ERROR", "no declarations"));
        };
        let scope = self.scopes.last().map(Vec::as_slice).unwrap_or(&[]);
        let typed = if self.migration.is_some() {
            crate::admit::typecheck::check_migration_expr(
                decls,
                &self.typed_derived,
                scope,
                true,
                &w,
            )
            .map_err(|es| {
                es.into_iter()
                    .next()
                    .map(BuildError::from)
                    .unwrap_or_else(|| err("TYPE_MISMATCH", "ill-typed expression"))
            })?
        } else {
            check_expr(decls, &self.typed_derived, scope, &w)?
        };
        Ok(Node {
            ty: Some(typed.ty().clone()),
            w,
            role,
            query: None,
        })
    }

    /// A query node (feature 007): checked as the argument of `count`, which applies every rule
    /// a query must satisfy (candidate-local filters, same-type set algebra).
    fn make_query(&mut self, w: WExpr, entity: String, children: &[&Node]) -> R<Node> {
        let untyped = children.iter().any(|c| c.ty.is_none() && c.query.is_none());
        if !untyped {
            let probe = WExpr {
                loc: w.loc.clone(),
                kind: WExprKind::Op {
                    op: crate::wire::OpName::Count,
                    args: vec![w.clone()],
                },
            };
            self.make(probe, children, None)?;
        }
        Ok(Node {
            w,
            ty: None,
            role: None,
            query: Some(entity),
        })
    }

    /// `select(T)`: every existing entity of type `entity` (feature 007).
    pub fn select(&mut self, entity: &str, loc: Loc) -> R<Node> {
        let w = WExpr {
            kind: WExprKind::Select {
                entity: entity.into(),
            },
            loc,
        };
        self.make_query(w, entity.into(), &[])
    }

    /// Opens the scope of a lambda body: the current scope plus the candidate `param` of type
    /// `entity` (feature 007). Close it with `pop_scope`.
    pub fn push_lambda(&mut self, param: &str, entity: &str) -> R<()> {
        let mut scope = self.scope().to_vec();
        if scope.iter().any(|p| p.name() == param) {
            return Err(err(
                "DUPLICATE_NAME",
                format!("the lambda parameter `{param}` shadows a parameter"),
            ));
        }
        scope.push(Param {
            name: param.into(),
            role: ParamRole::Read,
            ty: Type::Entity(entity.into()),
        });
        self.scopes.push(scope);
        Ok(())
    }

    /// A relational operator with a lambda `param → body` over the candidates of `query`:
    /// `where` gives a query; `any`, `all`, `sum`, `min`, `max`, `unique` a value.
    pub fn lambda(&mut self, op: &str, query: Node, param: &str, body: Node, loc: Loc) -> R<Node> {
        let Some(lop) = crate::wire::LambdaOp::parse(op) else {
            return Err(err("DECODE_ERROR", format!("unknown operator `{op}`")));
        };
        let Some(entity) = query.query.clone() else {
            return Err(err(
                "TYPE_MISMATCH",
                format!("`{op}` needs a query as its first argument"),
            ));
        };
        let w = WExpr {
            kind: WExprKind::Lambda {
                op: lop,
                query: Box::new(query.w.clone()),
                param: param.into(),
                body: Box::new(body.w.clone()),
            },
            loc,
        };
        if lop == crate::wire::LambdaOp::Where {
            self.make_query(w, entity, &[&query, &body])
        } else {
            self.make(w, &[&query, &body], None)
        }
    }

    pub fn lit(&mut self, ty: WType, value: Json, loc: Loc) -> R<Node> {
        self.make(
            WExpr {
                kind: WExprKind::Lit { ty, value },
                loc,
            },
            &[],
            None,
        )
    }

    pub fn field(&mut self, param: &str, field: &str, loc: Loc) -> R<Node> {
        let role = self
            .scope()
            .iter()
            .find(|p| p.name() == param)
            .map(Param::role);
        let kind = WExprKind::Field {
            param: param.into(),
            field: field.into(),
        };
        self.make(WExpr { kind, loc }, &[], role)
    }

    pub fn param(&mut self, name: &str, loc: Loc) -> R<Node> {
        let role = self
            .scope()
            .iter()
            .find(|p| p.name() == name)
            .map(Param::role);
        self.make(
            WExpr {
                kind: WExprKind::Param(name.into()),
                loc,
            },
            &[],
            role,
        )
    }

    /// A reference to a derived value. If the derived value has not been added yet (it is still
    /// being traced: a cycle), the node is untyped and `finish` reports the cycle.
    pub fn derived_ref(&mut self, name: &str, args: Vec<String>, loc: Loc) -> R<Node> {
        let w = WExpr {
            kind: WExprKind::Derived {
                name: name.into(),
                args,
            },
            loc,
        };
        if !self.typed_derived.contains_key(name) {
            return Ok(Node {
                w,
                ty: None,
                role: None,
                query: None,
            });
        }
        self.make(w, &[], None)
    }

    pub fn op(&mut self, op: &str, args: Vec<Node>, loc: Loc) -> R<Node> {
        let Some(name) = op_name(op) else {
            return Err(err("DECODE_ERROR", format!("unknown operator `{op}`")));
        };
        if !arity_ok(name, args.len()) {
            return Err(err(
                "ARITY_MISMATCH",
                format!("wrong number of arguments for `{op}`"),
            ));
        }
        let w = WExpr {
            kind: WExprKind::Op {
                op: name,
                args: args.iter().map(|a| a.w.clone()).collect(),
            },
            loc,
        };
        let children: Vec<&Node> = args.iter().collect();
        if matches!(
            name,
            crate::wire::OpName::Union
                | crate::wire::OpName::Intersection
                | crate::wire::OpName::Difference
        ) {
            let entity = args
                .first()
                .and_then(|a| a.query.clone())
                .ok_or_else(|| err("TYPE_MISMATCH", format!("`{op}` combines queries")))?;
            return self.make_query(w, entity, &children);
        }
        if children.iter().any(|c| c.query.is_some()) && name != crate::wire::OpName::Count {
            return Err(err(
                "TYPE_MISMATCH",
                format!("a query is a set of entities, not a value, and cannot be used in `{op}`"),
            ));
        }
        self.make(w, &children, None)
    }

    pub fn in_(&mut self, arg: Node, values: Vec<Json>, loc: Loc) -> R<Node> {
        let w = WExpr {
            kind: WExprKind::In {
                arg: Box::new(arg.w.clone()),
                values,
            },
            loc,
        };
        self.make(w, &[&arg], None)
    }

    pub fn wrap(&mut self, nominal: &str, arg: Node, loc: Loc) -> R<Node> {
        let w = WExpr {
            kind: WExprKind::Wrap {
                nominal: nominal.into(),
                arg: Box::new(arg.w.clone()),
            },
            loc,
        };
        self.make(w, &[&arg], None)
    }

    /// `rescale(arg, nominal, rounding)`: the explicit narrowing to a fixed-scale type.
    pub fn rescale(&mut self, arg: Node, nominal: &str, rounding: &str, loc: Loc) -> R<Node> {
        let w = WExpr {
            kind: WExprKind::Rescale {
                nominal: nominal.into(),
                rounding: rounding.into(),
                arg: Box::new(arg.w.clone()),
            },
            loc,
        };
        self.make(w, &[&arg], None)
    }

    // --- statements ----------------------------------------------------------------------

    /// A pre/postcondition, invariant, or rule body must be Bool.
    pub fn check_condition(&self, node: &Node, what: &str) -> R<()> {
        match &node.ty {
            Some(t) if *t != Type::Bool => Err(err(
                "NOT_BOOLEAN",
                format!("{what} must be Bool, found `{t}`"),
            )),
            _ => Ok(()),
        }
    }

    /// Checks `set_(param.field, value)` in the current action scope.
    pub fn check_effect(&mut self, param: &str, field: &str, value: &Node) -> R<()> {
        self.ensure_decls()?;
        let Some(decls) = &self.decls else {
            return Err(err("DECODE_ERROR", "no declarations"));
        };
        let target = effect_target(decls, self.scope(), param, field)
            .map_err(|(code, message)| err(code, message))?;
        if let Some(t) = &value.ty
            && coerce(&target, t).is_none()
            && !crate::admit::typecheck::exact_store(t, &target)
        {
            let lossy = matches!(t, Type::Exact(_))
                || (crate::semantic::types::fixed_scale(&target).is_some() && *t == Type::Decimal);
            return Err(if lossy {
                err(
                    "LOSSY_CONVERSION",
                    format!(
                        "assigning `{t}` to `{param}.{field}: {target}` would lose information; \
                         use `rescale(value, {target}, rounding)`"
                    ),
                )
            } else {
                err(
                    "TYPE_MISMATCH",
                    format!("cannot assign `{t}` to `{param}.{field}: {target}`"),
                )
            });
        }
        Ok(())
    }

    fn claim(&self, name: &str) -> R<()> {
        let taken = self.derived.iter().any(|d| d.name == name)
            || self.invariants.iter().any(|i| i.name == name)
            || self.constraints.iter().any(|c| c.name == name)
            || self.global_invariants.iter().any(|g| g.name == name)
            || self.actions.iter().any(|a| a.name == name);
        if taken {
            return Err(err(
                "DUPLICATE_NAME",
                format!("`{name}` is declared more than once"),
            ));
        }
        Ok(())
    }

    pub fn add_derived(
        &mut self,
        name: &str,
        kind: DerivedKind,
        ps: Vec<WParam>,
        body: Node,
        declared: Option<WType>,
        loc: Loc,
    ) -> R<()> {
        self.claim(name)?;
        let wd = WDerived {
            name: name.into(),
            kind,
            params: ps,
            body: body.w.clone(),
            declared,
            loc,
        };
        if body.ty.is_some() {
            self.ensure_decls()?;
            let Some(decls) = &self.decls else {
                return Err(err("DECODE_ERROR", "no declarations"));
            };
            let mut errors = Vec::new();
            match derived_item(decls, &self.typed_derived, &wd, &mut errors) {
                Some(item) => {
                    self.typed_derived.insert(name.into(), item);
                }
                None => {
                    return Err(errors
                        .into_iter()
                        .next()
                        .map(Into::into)
                        .unwrap_or_else(|| err("TYPE_MISMATCH", "ill-typed derived value")));
                }
            }
        }
        self.derived.push(wd);
        Ok(())
    }

    pub fn add_invariant(
        &mut self,
        name: &str,
        entity: &str,
        param: &str,
        body: Node,
        loc: Loc,
    ) -> R<()> {
        self.claim(name)?;
        self.check_condition(&body, "an invariant")?;
        self.invariants.push(WInvariant {
            name: name.into(),
            entity: entity.into(),
            param: param.into(),
            body: body.w,
            loc,
        });
        Ok(())
    }

    /// Adds a module invariant (feature 007): a closed state expression over entity sets.
    pub fn add_global_invariant(&mut self, name: &str, body: Node, loc: Loc) -> R<()> {
        self.claim(name)?;
        self.check_condition(&body, "a module invariant")?;
        self.global_invariants.push(WGlobalInvariant {
            name: name.into(),
            body: body.w,
            loc,
        });
        Ok(())
    }

    /// Adds an entity constraint: a validity rule over one entity of type `entity`.
    pub fn add_constraint(
        &mut self,
        name: &str,
        entity: &str,
        param: &str,
        body: Node,
        loc: Loc,
    ) -> R<()> {
        self.claim(name)?;
        self.check_condition(&body, "an entity constraint")?;
        self.constraints.push(WConstraint {
            name: name.into(),
            entity: entity.into(),
            param: param.into(),
            body: body.w,
            loc,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_action(
        &mut self,
        name: &str,
        ps: Vec<WParam>,
        preconditions: Vec<(Node, Loc)>,
        effects: Vec<(String, String, Node, Loc)>,
        lifecycle: Vec<WLifecycle>,
        postconditions: Vec<(Node, Loc)>,
        loc: Loc,
    ) -> R<()> {
        self.claim(name)?;
        let cond = |(n, l): (Node, Loc)| WCond { expr: n.w, loc: l };
        let creates = lifecycle
            .iter()
            .any(|l| matches!(l, WLifecycle::Create { .. }));
        if !creates {
            // Without a creation, an action needs a state parameter (checked as at admission).
            self.ensure_decls()?;
            if let Some(decls) = &self.decls {
                let mut errors = Vec::new();
                if params(decls, &ps, ParamSite::Action, &loc, &mut errors).is_none() {
                    return Err(errors
                        .into_iter()
                        .next()
                        .map(Into::into)
                        .unwrap_or_else(|| err("ARITY_MISMATCH", "invalid parameters")));
                }
            }
        }
        self.actions.push(WAction {
            name: name.into(),
            params: ps,
            preconditions: preconditions.into_iter().map(cond).collect(),
            effects: effects
                .into_iter()
                .map(|(param, field, n, l)| WEffect {
                    param,
                    field,
                    value: n.w,
                    loc: l,
                })
                .collect(),
            lifecycle,
            postconditions: postconditions.into_iter().map(cond).collect(),
            loc,
        });
        Ok(())
    }

    /// Relativizes source locations (to `root`, or to the common directory of all files) and
    /// admits the module through the same pipeline as wire JSON.
    pub fn finish(&self, root: Option<&str>) -> Result<Module, AdmissionResult> {
        let mut w = WModule {
            enums: self.enums.clone(),
            nominals: self.nominals.clone(),
            entities: self.entities.clone(),
            derived: self.derived.clone(),
            invariants: self.invariants.clone(),
            global_invariants: self.global_invariants.clone(),
            constraints: self.constraints.clone(),
            actions: self.actions.clone(),
        };
        let mut files = Vec::new();
        for_each_loc(&mut w, &mut |l| files.push(PathBuf::from(&l.file)));
        let root = match root {
            Some(r) => PathBuf::from(r),
            None => common_dir(&files),
        };
        for_each_loc(&mut w, &mut |l| l.file = relative(&l.file, &root));
        admit_wire(&w)
    }
}

fn common_dir(files: &[PathBuf]) -> PathBuf {
    let mut dirs = files
        .iter()
        .map(|f| f.parent().map(Path::to_path_buf).unwrap_or_default());
    let Some(mut common) = dirs.next() else {
        return PathBuf::new();
    };
    for d in dirs {
        let shared: PathBuf = common
            .components()
            .zip(d.components())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a)
            .collect();
        common = shared;
    }
    common
}

fn relative(file: &str, root: &Path) -> String {
    let path = Path::new(file);
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            Component::ParentDir => Some("..".to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn expr_locs(e: &mut WExpr, f: &mut dyn FnMut(&mut Loc)) {
    f(&mut e.loc);
    match &mut e.kind {
        WExprKind::Op { args, .. } => args.iter_mut().for_each(|a| expr_locs(a, f)),
        WExprKind::In { arg, .. }
        | WExprKind::Wrap { arg, .. }
        | WExprKind::StrictUnwrap(arg)
        | WExprKind::EnumMap { arg, .. } => expr_locs(arg, f),
        WExprKind::Lambda { query, body, .. } => {
            expr_locs(query, f);
            expr_locs(body, f);
        }
        _ => {}
    }
}

fn for_each_loc(w: &mut WModule, f: &mut dyn FnMut(&mut Loc)) {
    w.enums.iter_mut().for_each(|e| f(&mut e.loc));
    w.nominals.iter_mut().for_each(|n| f(&mut n.loc));
    for e in &mut w.entities {
        f(&mut e.loc);
        e.fields.iter_mut().for_each(|x| f(&mut x.loc));
    }
    for d in &mut w.derived {
        f(&mut d.loc);
        expr_locs(&mut d.body, f);
    }
    for i in &mut w.invariants {
        f(&mut i.loc);
        expr_locs(&mut i.body, f);
    }
    for c in &mut w.constraints {
        f(&mut c.loc);
        expr_locs(&mut c.body, f);
    }
    for g in &mut w.global_invariants {
        f(&mut g.loc);
        expr_locs(&mut g.body, f);
    }
    for a in &mut w.actions {
        f(&mut a.loc);
        for c in a
            .preconditions
            .iter_mut()
            .chain(a.postconditions.iter_mut())
        {
            f(&mut c.loc);
            expr_locs(&mut c.expr, f);
        }
        for e in &mut a.effects {
            f(&mut e.loc);
            expr_locs(&mut e.value, f);
        }
        for l in &mut a.lifecycle {
            match l {
                WLifecycle::Create {
                    id, fields, loc, ..
                } => {
                    f(loc);
                    expr_locs(id, f);
                    fields.iter_mut().for_each(|(_, e)| expr_locs(e, f));
                }
                WLifecycle::Remove { loc, .. } => f(loc),
            }
        }
    }
}
