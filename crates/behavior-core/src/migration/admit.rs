//! Migration admission (FR-006–FR-010, research R4, R5): two-sided typing, field resolution
//! (automatic copies become explicit), explicit drops and retirements, and the identity of the
//! resolved migration.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as Json, json};

use super::wire::{MIGRATION_IR_VERSION, WFieldSpec, WMigration, WTransform, decode_migration};
use super::{Constant, FieldSource, Migration, NarrowingSite, Requirement, Transform};
use crate::admit::hash::{self, MigrationField};
use crate::admit::resolve::Decls;
use crate::admit::typecheck::{check_migration_expr, reads_universe, store_value};
use crate::admit::{AdmissionError, AdmissionResult};
use crate::schema::schema;
use crate::semantic::expr::{Expr, ExprKind};
use crate::semantic::module::{DerivedItem, Module, Param, ParamRole};
use crate::semantic::types::{Type, hash_display};
use crate::semantic::value::encode;
use crate::serialize::{Sides, migration_expr, migration_type};
use crate::wire::{DecodeError, Loc, TARGET_SIDE, WExpr, WExprKind};

/// One resolved transform as it is hashed: entity, fields, drops.
type HashedTransform<'a> = (&'a str, Vec<(&'a str, MigrationField<'a>)>, &'a [String]);

/// The name of the source entity inside a transform.
pub const OLD: &str = "old";

fn err(code: &str, message: impl Into<String>, loc: Option<&Loc>) -> AdmissionError {
    AdmissionError::new(code, message, loc)
}

fn decode_error(e: DecodeError) -> AdmissionError {
    let message = match e {
        DecodeError::Structure { path, message } => format!("at {path}: {message}"),
        other => format!("{other:?}"),
    };
    err("DECODE_ERROR", message, None)
}

/// Admits a migration document (migration IR 0.1) between `source` and `target`.
pub fn admit_migration(
    source: &Module,
    target: &Module,
    text: &str,
) -> Result<Migration, AdmissionResult> {
    let w = decode_migration(text).map_err(|e| AdmissionResult::failed(vec![decode_error(e)]))?;
    admit_migration_wire(source, target, &w)
}

/// The two-sided declaration table (research R5): the source module's names as they are, the
/// target module's enums and nominals under [`TARGET_SIDE`], and every entity type of either side.
pub(crate) fn merged_decls(source: &Module, target: &Module) -> Decls {
    let mut enums = source.enums.clone();
    for (n, e) in &target.enums {
        enums.insert(format!("{TARGET_SIDE}{n}"), e.clone());
    }
    let mut nominals = source.nominals.clone();
    for (n, x) in &target.nominals {
        nominals.insert(format!("{TARGET_SIDE}{n}"), x.clone());
    }
    let mut entities = source.entities.clone();
    for (n, e) in &target.entities {
        entities.entry(n.clone()).or_insert_with(|| e.clone());
    }
    Decls {
        enums,
        nominals,
        entities,
        reads: BTreeSet::new(),
    }
}

/// The source fields an expression reads from `param`, through the derived values it uses.
fn fields_read(
    e: &Expr,
    param: &str,
    derived: &BTreeMap<String, DerivedItem>,
    out: &mut BTreeSet<String>,
) {
    match &e.kind {
        ExprKind::Field { param: p, field } if p == param => {
            out.insert(field.clone());
        }
        ExprKind::DerivedRef { name, args, .. } => {
            if let Some(d) = derived.get(name) {
                for (p, a) in d.params.iter().zip(args) {
                    if a == param {
                        fields_read(&d.body, &p.name, derived, out);
                    }
                }
            }
        }
        _ => {}
    }
    for c in e.children() {
        fields_read(c, param, derived, out);
    }
}

fn narrowing(e: &Expr, entity: &str, field: &str, out: &mut Vec<NarrowingSite>) {
    let kind = match &e.kind {
        ExprKind::StrictUnwrap(_) => Some("strict_unwrap"),
        ExprKind::EnumMap { strict: true, .. } => Some("strict_enum_map"),
        _ => None,
    };
    if let Some(kind) = kind {
        out.push(NarrowingSite {
            entity: entity.to_string(),
            field: field.to_string(),
            kind,
            expr: e.clone(),
        });
    }
    for c in e.children() {
        narrowing(c, entity, field, out);
    }
}

/// A clearer message when a type mismatch is between two same-named types of different schemas.
fn mismatch_message(msg: String, from: &Type, to: &Type) -> String {
    if from != to && from.to_string() == to.to_string() {
        format!(
            "{msg}: the source and target `{to}` are different types (their declarations differ); \
             convert explicitly"
        )
    } else {
        msg
    }
}

struct Admission<'a> {
    source: &'a Module,
    target: &'a Module,
    decls: Decls,
    errors: Vec<AdmissionError>,
}

impl Admission<'_> {
    fn constants(&mut self, w: &WMigration) -> (Vec<Constant>, Vec<Param>) {
        let mut out = Vec::new();
        let mut scope = Vec::new();
        let mut seen = BTreeSet::new();
        for c in &w.constants {
            if c.name == OLD || !seen.insert(c.name.clone()) {
                let msg = format!("constant `{}` is declared twice or shadows `old`", c.name);
                self.errors.push(err("DUPLICATE_NAME", msg, Some(&c.loc)));
                continue;
            }
            let lit = WExpr {
                kind: WExprKind::Lit {
                    ty: c.ty.clone(),
                    value: c.value.clone(),
                },
                loc: c.loc.clone(),
            };
            match check_migration_expr(&self.decls, &self.source.derived, &[], false, &lit) {
                Ok(Expr {
                    kind: ExprKind::Lit(v),
                    ty,
                    ..
                }) => {
                    scope.push(Param {
                        name: c.name.clone(),
                        role: ParamRole::Read,
                        ty: ty.clone(),
                    });
                    out.push(Constant {
                        name: c.name.clone(),
                        ty,
                        value: v,
                        loc: c.loc.clone(),
                    });
                }
                Ok(_) => self.errors.push(err(
                    "INVALID_LITERAL",
                    format!("constant `{}` is not a literal", c.name),
                    Some(&c.loc),
                )),
                Err(es) => self.errors.extend(es),
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        (out, scope)
    }

    fn requirements(&mut self, w: &WMigration, scope: &[Param]) -> Vec<Requirement> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for r in &w.requirements {
            if !seen.insert(r.name.clone()) {
                let msg = format!("requirement `{}` is declared twice", r.name);
                self.errors.push(err("DUPLICATE_NAME", msg, Some(&r.loc)));
                continue;
            }
            match check_migration_expr(&self.decls, &self.source.derived, scope, true, &r.body) {
                Ok(e) if e.ty == Type::Bool => out.push(Requirement {
                    name: r.name.clone(),
                    body: e,
                    loc: r.loc.clone(),
                }),
                Ok(e) => self.errors.push(err(
                    "TYPE_MISMATCH",
                    format!("requirement `{}` is `{}`, not Bool", r.name, e.ty),
                    Some(&r.loc),
                )),
                Err(es) => self.errors.extend(es),
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn retire(&mut self, w: &WMigration) -> Vec<String> {
        let mut out = BTreeSet::new();
        for r in &w.retire {
            let declared_by_source = self.source.entities.contains_key(r);
            let declared_by_target = self.target.entities.contains_key(r);
            if !declared_by_source || declared_by_target || !out.insert(r.clone()) {
                self.errors.push(err(
                    "INVALID_RETIREMENT",
                    format!(
                        "`{r}` cannot be retired: only entity types of the source schema that the \
                         target schema no longer declares are retired, once each"
                    ),
                    None,
                ));
            }
        }
        for name in self.source.entities.keys() {
            if !self.target.entities.contains_key(name) && !out.contains(name) {
                self.errors.push(err(
                    "MISSING_RETIREMENT",
                    format!(
                        "the target schema no longer declares `{name}`; retire it explicitly (it \
                         must hold no entities when the migration is applied)"
                    ),
                    None,
                ));
            }
        }
        out.into_iter().collect()
    }

    /// The resolved transform of one changed entity type (FR-007a, FR-007b).
    fn transform(
        &mut self,
        entity: &str,
        w: Option<&WTransform>,
        constants: &[Param],
    ) -> Option<Transform> {
        let source_item = self.source.entities.get(entity)?.clone();
        let target_item = self.target.entities.get(entity)?.clone();
        let loc = w.and_then(|t| t.loc.clone());
        let mut scope = vec![Param {
            name: OLD.to_string(),
            role: ParamRole::Read,
            ty: Type::Entity(entity.to_string()),
        }];
        scope.extend(constants.iter().cloned());
        let explicit: BTreeMap<&str, &WFieldSpec> = w
            .map(|t| t.fields.iter().map(|(n, s)| (n.as_str(), s)).collect())
            .unwrap_or_default();
        for name in explicit.keys() {
            if target_item.field_type(name).is_none() {
                self.errors.push(err(
                    "INVALID_TRANSFORM",
                    format!("the target `{entity}` has no field `{name}`"),
                    loc.as_ref(),
                ));
            }
        }
        let mut fields = Vec::new();
        let mut read = BTreeSet::new();
        let mut ok = true;
        for (name, fty) in &target_item.fields {
            let what = format!("{entity}.{name}");
            match explicit.get(name.as_str()) {
                Some(spec) if name == "id" && !matches!(spec, WFieldSpec::Copy(f) if f == "id") => {
                    self.errors.push(err(
                        "MIGRATION_TYPE_MISMATCH",
                        format!(
                            "an entity keeps its identity: `{what}` is copied and cannot be assigned"
                        ),
                        loc.as_ref(),
                    ));
                    ok = false;
                }
                Some(WFieldSpec::Copy(from)) => match source_item.field_type(from) {
                    Some(st) if st == fty => {
                        read.insert(from.clone());
                        fields.push((name.clone(), FieldSource::Copy(from.clone())));
                    }
                    Some(st) => {
                        let msg = mismatch_message(
                            format!(
                                "`{entity}.{from}: {st}` cannot be copied into `{what}: {fty}`"
                            ),
                            st,
                            fty,
                        );
                        self.errors
                            .push(err("MIGRATION_TYPE_MISMATCH", msg, loc.as_ref()));
                        ok = false;
                    }
                    None => {
                        self.errors.push(err(
                            "INVALID_TRANSFORM",
                            format!("the source `{entity}` has no field `{from}` to copy"),
                            loc.as_ref(),
                        ));
                        ok = false;
                    }
                },
                Some(WFieldSpec::Expr(wexpr)) => {
                    match check_migration_expr(
                        &self.decls,
                        &self.source.derived,
                        &scope,
                        false,
                        wexpr,
                    ) {
                        Ok(e) if reads_universe(&e, &self.source.derived) => {
                            self.errors.push(err(
                                "NON_LOCAL_TRANSFORM",
                                format!(
                                    "`{what}` reads other entities; a transform reads only the \
                                     entity being migrated, literals and constants (fill such \
                                     values with an ordinary action between migrations)"
                                ),
                                Some(&wexpr.loc),
                            ));
                            ok = false;
                        }
                        Ok(e) => {
                            let from = e.ty.clone();
                            match store_value(e, fty, &what) {
                                Ok(e) => {
                                    fields_read(&e, OLD, &self.source.derived, &mut read);
                                    fields.push((name.clone(), FieldSource::Expr(e)));
                                }
                                Err((_, msg)) => {
                                    self.errors.push(err(
                                        "MIGRATION_TYPE_MISMATCH",
                                        mismatch_message(msg, &from, fty),
                                        Some(&wexpr.loc),
                                    ));
                                    ok = false;
                                }
                            }
                        }
                        Err(es) => {
                            self.errors.extend(es);
                            ok = false;
                        }
                    }
                }
                None => match source_item.field_type(name) {
                    Some(st) if st == fty => {
                        read.insert(name.clone());
                        fields.push((name.clone(), FieldSource::Copy(name.clone())));
                    }
                    _ => {
                        self.errors.push(err(
                            "MISSING_MIGRATION_FIELD",
                            format!(
                                "`{what}: {fty}` has no source field with the same name and type; \
                                 assign it explicitly"
                            ),
                            loc.as_ref(),
                        ));
                        ok = false;
                    }
                },
            }
        }
        let mut drops = BTreeSet::new();
        for d in w.map(|t| t.drops.as_slice()).unwrap_or_default() {
            if source_item.field_type(d).is_none() || target_item.field_type(d).is_some() {
                self.errors.push(err(
                    "INVALID_TRANSFORM",
                    format!(
                        "`{entity}.{d}` cannot be dropped: only source fields the target no \
                         longer declares are dropped"
                    ),
                    loc.as_ref(),
                ));
                ok = false;
            }
            drops.insert(d.clone());
        }
        // Only once every assignment is admitted: a refused one reads nothing that counts.
        let assignments_ok = ok;
        for (f, _) in source_item.fields.iter().filter(|_| assignments_ok) {
            if target_item.field_type(f).is_none() && !drops.contains(f) && !read.contains(f) {
                self.errors.push(err(
                    "UNACKNOWLEDGED_FIELD_DROP",
                    format!(
                        "`{entity}.{f}` is not in the target schema and no assignment reads it: \
                         its information would be lost; acknowledge that with an explicit drop"
                    ),
                    loc.as_ref(),
                ));
                ok = false;
            }
        }
        ok.then(|| Transform {
            fields,
            drops: drops.into_iter().collect(),
            loc,
        })
    }
}

/// Admits a decoded migration between `source` and `target` (the single entry point, shared by
/// JSON decoding and the builder).
pub fn admit_migration_wire(
    source: &Module,
    target: &Module,
    w: &WMigration,
) -> Result<Migration, AdmissionResult> {
    let (sa, ta) = (schema(source), schema(target));
    if w.source != sa.hash || w.target != ta.hash {
        return Err(AdmissionResult::failed(vec![err(
            "MIGRATION_SCHEMA_MISMATCH",
            format!(
                "the migration relates schema {} to {}; the modules declare {} and {}",
                w.source, w.target, sa.hash, ta.hash
            ),
            None,
        )]));
    }
    if sa.hash == ta.hash {
        return Err(AdmissionResult::failed(vec![err(
            "MIGRATION_SCHEMA_MISMATCH",
            "the source and target schemas are the same: there is nothing to migrate (behavior-only \
             changes need no migration)",
            None,
        )]));
    }
    let mut a = Admission {
        source,
        target,
        decls: merged_decls(source, target),
        errors: Vec::new(),
    };
    let (constants, scope) = a.constants(w);
    let requirements = a.requirements(w, &scope);
    let retire = a.retire(w);
    let mut by_entity: BTreeMap<&str, &WTransform> = BTreeMap::new();
    for t in &w.transforms {
        if by_entity.insert(t.entity.as_str(), t).is_some() {
            a.errors.push(err(
                "INVALID_TRANSFORM",
                format!("`{}` has more than one transform", t.entity),
                t.loc.as_ref(),
            ));
        }
        let in_both =
            source.entities.contains_key(&t.entity) && target.entities.contains_key(&t.entity);
        if !in_both {
            a.errors.push(err(
                "INVALID_TRANSFORM",
                format!(
                    "`{}` is not an entity type of both schemas; types new in the target start \
                     empty and removed ones are retired",
                    t.entity
                ),
                t.loc.as_ref(),
            ));
        } else if sa.declarations.get(&t.entity) == ta.declarations.get(&t.entity) {
            a.errors.push(err(
                "INVALID_TRANSFORM",
                format!(
                    "`{}` is declared identically by both schemas; it is carried over unchanged",
                    t.entity
                ),
                t.loc.as_ref(),
            ));
        }
    }
    let mut transforms = BTreeMap::new();
    for (name, decl) in &sa.declarations {
        let changed = ta.declarations.get(name).is_some_and(|t| t != decl);
        if changed && let Some(t) = a.transform(name, by_entity.get(name.as_str()).copied(), &scope)
        {
            transforms.insert(name.clone(), t);
        }
    }
    if !a.errors.is_empty() {
        return Err(AdmissionResult::failed(a.errors));
    }
    let mut sites = Vec::new();
    for (entity, t) in &transforms {
        for (field, src) in &t.fields {
            if let FieldSource::Expr(e) = src {
                narrowing(e, entity, field, &mut sites);
            }
        }
    }
    let identity = {
        let cs: Vec<(&str, &Type, &crate::semantic::value::Value)> = constants
            .iter()
            .map(|c| (c.name.as_str(), &c.ty, &c.value))
            .collect();
        let rs: Vec<(&str, &crate::semantic::types::Hash)> = requirements
            .iter()
            .map(|r| (r.name.as_str(), &r.body.hash))
            .collect();
        let ts: Vec<HashedTransform<'_>> = transforms
            .iter()
            .map(|(entity, t)| {
                let fields = t
                    .fields
                    .iter()
                    .map(|(n, s)| {
                        let f = match s {
                            FieldSource::Copy(from) => MigrationField::Copy(from.as_str()),
                            FieldSource::Expr(e) => MigrationField::Expr(&e.hash),
                        };
                        (n.as_str(), f)
                    })
                    .collect();
                (entity.as_str(), fields, t.drops.as_slice())
            })
            .collect();
        hash::migration(&sa.hash, &ta.hash, &cs, &rs, &ts, &retire)
    };
    let sides = Sides { source };
    let summary = summary(&transforms, &retire, &source.derived);
    let resolved = json!({
        "migration_ir": MIGRATION_IR_VERSION,
        "name": w.name,
        "source": sa.hash,
        "target": ta.hash,
        "constants": constants.iter().map(|c| json!({
            "name": c.name, "type": migration_type(&c.ty, &sides),
            "value": encode(&c.ty, &c.value), "loc": loc_json(&c.loc),
        })).collect::<Vec<_>>(),
        "requirements": requirements.iter().map(|r| json!({
            "name": r.name, "body": migration_expr(&r.body, &sides), "loc": loc_json(&r.loc),
        })).collect::<Vec<_>>(),
        "transforms": transforms.iter().map(|(entity, t)| {
            let fields: serde_json::Map<String, Json> = t.fields.iter().map(|(n, s)| {
                let v = match s {
                    FieldSource::Copy(from) => json!({"copy": from}),
                    FieldSource::Expr(e) => migration_expr(e, &sides),
                };
                (n.clone(), v)
            }).collect();
            let mut o = json!({"entity": entity, "fields": fields, "drops": t.drops});
            if let (Some(l), Json::Object(m)) = (&t.loc, &mut o) {
                m.insert("loc".into(), loc_json(l));
            }
            o
        }).collect::<Vec<_>>(),
        "retire": retire,
    });
    Ok(Migration {
        name: w.name.clone(),
        source: sa,
        target: ta,
        constants,
        requirements,
        transforms,
        retire,
        narrowing: sites,
        hash: identity,
        resolved,
        summary,
    })
}

fn loc_json(l: &Loc) -> Json {
    json!({"file": l.file, "line": l.line})
}

/// The reviewable summary (FR-007c): per migrated type, its target fields classified as copied,
/// transformed (computed from the old entity) or new (computed from literals and constants only),
/// and its dropped source fields; plus the retired types.
fn summary(
    transforms: &BTreeMap<String, Transform>,
    retire: &[String],
    derived: &BTreeMap<String, DerivedItem>,
) -> Json {
    let mut types = serde_json::Map::new();
    for (entity, t) in transforms {
        let (mut copied, mut transformed, mut new) = (Vec::new(), Vec::new(), Vec::new());
        for (name, src) in &t.fields {
            match src {
                FieldSource::Copy(_) => copied.push(name.clone()),
                FieldSource::Expr(e) => {
                    let mut read = BTreeSet::new();
                    fields_read(e, OLD, derived, &mut read);
                    if read.is_empty() {
                        new.push(name.clone());
                    } else {
                        transformed.push(name.clone());
                    }
                }
            }
        }
        copied.sort();
        transformed.sort();
        new.sort();
        types.insert(
            entity.clone(),
            json!({"copied": copied, "transformed": transformed, "new": new, "dropped": t.drops}),
        );
    }
    json!({"types": types, "retired": retire})
}

/// The display form of a migration's identity.
pub(crate) fn display(h: &crate::semantic::types::Hash) -> String {
    hash_display(h)
}
