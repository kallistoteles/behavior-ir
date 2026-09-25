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
    DerivedKind, Loc, WAction, WCond, WDerived, WEffect, WEntity, WEnum, WExpr, WExprKind, WField,
    WInvariant, WModule, WNominal, WParam, WType, arity_ok, op_name,
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
}

/// A typed expression node. `ty` is `None` for a node that depends on a derived value that is
/// still being traced (a cycle); such nodes skip the immediate check and `finish` reports.
#[derive(Debug, Clone)]
pub struct Node {
    pub(crate) w: WExpr,
    pub(crate) ty: Option<Type>,
    pub(crate) role: Option<ParamRole>,
}

impl Node {
    /// The node's type in wire form, or `None` if it is untyped (below a cycle reference).
    pub fn type_wire_json(&self) -> Option<Json> {
        self.ty.as_ref().map(Type::to_wire_json)
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
    actions: Vec<WAction>,
    decls: Option<Decls>,
    typed_derived: BTreeMap<String, DerivedItem>,
    scopes: Vec<Vec<Param>>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
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
    pub fn declare_nominal(
        &mut self,
        name: &str,
        underlying: WType,
        mut ops: Vec<String>,
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
        let Some(decls) = &self.decls else {
            return Err(err("DECODE_ERROR", "no declarations"));
        };
        let site = match site {
            ScopeSite::Derived => ParamSite::Derived,
            ScopeSite::Action => ParamSite::Action,
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
        if children.iter().any(|c| c.ty.is_none()) {
            return Ok(Node { w, ty: None, role });
        }
        self.ensure_decls()?;
        let Some(decls) = &self.decls else {
            return Err(err("DECODE_ERROR", "no declarations"));
        };
        let scope = self.scopes.last().map(Vec::as_slice).unwrap_or(&[]);
        let typed = check_expr(decls, &self.typed_derived, scope, &w)?;
        Ok(Node {
            ty: Some(typed.ty().clone()),
            w,
            role,
        })
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
        {
            return Err(err(
                "TYPE_MISMATCH",
                format!("cannot assign `{t}` to `{param}.{field}: {target}`"),
            ));
        }
        Ok(())
    }

    fn claim(&self, name: &str) -> R<()> {
        let taken = self.derived.iter().any(|d| d.name == name)
            || self.invariants.iter().any(|i| i.name == name)
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
        loc: Loc,
    ) -> R<()> {
        self.claim(name)?;
        let wd = WDerived {
            name: name.into(),
            kind,
            params: ps,
            body: body.w.clone(),
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

    #[allow(clippy::too_many_arguments)]
    pub fn add_action(
        &mut self,
        name: &str,
        ps: Vec<WParam>,
        preconditions: Vec<(Node, Loc)>,
        effects: Vec<(String, String, Node, Loc)>,
        postconditions: Vec<(Node, Loc)>,
        loc: Loc,
    ) -> R<()> {
        self.claim(name)?;
        let cond = |(n, l): (Node, Loc)| WCond { expr: n.w, loc: l };
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
        WExprKind::In { arg, .. } | WExprKind::Wrap { arg, .. } => expr_locs(arg, f),
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
    }
}
