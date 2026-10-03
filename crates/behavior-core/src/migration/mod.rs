//! Migrations (feature 009): explicit, verifiable transformations of a valid state under one store
//! schema into a valid state under another. A migration is its own document (migration IR 0.1),
//! admitted against its source and target modules, content-addressed by its resolved form, and
//! applied entity by entity with global validation.
//!
//! Principles: migrations change representation; behavior changes information. Broaden freely;
//! narrow only after proving the data already fits.

mod admit;
mod apply;
pub mod wire;

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::schema::StoreSchema;
use crate::semantic::expr::Expr;
use crate::semantic::types::{Hash, Type};
use crate::semantic::value::Value;
use crate::wire::Loc;

pub(crate) use admit::merged_decls;
pub use admit::{OLD, admit_migration, admit_migration_wire};
pub use apply::{
    Migrated, MigratedEntity, MigrationRefusal, RequirementOutcome, SourceEntity, apply_migration,
    outcome_json, transform_one,
};
pub use wire::{MIGRATION_IR_VERSION, WMigration, decode_migration};

/// A named, typed literal of a migration.
#[derive(Debug, Clone)]
pub struct Constant {
    pub(crate) name: String,
    pub(crate) ty: Type,
    pub(crate) value: Value,
    pub(crate) loc: Loc,
}

impl Constant {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn ty(&self) -> &Type {
        &self.ty
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// A source requirement: a closed Bool predicate over the immutable source state, an
/// applicability condition of the migration (FR-007d).
#[derive(Debug, Clone)]
pub struct Requirement {
    pub(crate) name: String,
    pub(crate) body: Expr,
    pub(crate) loc: Loc,
}

impl Requirement {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn body(&self) -> &Expr {
        &self.body
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

/// How one target field is computed in a resolved transform.
#[derive(Debug, Clone)]
pub enum FieldSource {
    /// The source field, unchanged (an automatic or explicit copy: equal type identity).
    Copy(String),
    /// An expression over the old entity (`old`), literals and constants.
    Expr(Expr),
}

/// The resolved transform of one changed entity type: every target field, in target declaration
/// order, and the acknowledged dropped source fields.
#[derive(Debug, Clone)]
pub struct Transform {
    pub(crate) fields: Vec<(String, FieldSource)>,
    pub(crate) drops: Vec<String>,
    pub(crate) loc: Option<Loc>,
}

impl Transform {
    pub fn fields(&self) -> &[(String, FieldSource)] {
        &self.fields
    }
    pub fn drops(&self) -> &[String] {
        &self.drops
    }
}

/// A narrowing site (FR-007e): a `strict_unwrap` or `strict_enum_map` in the transform of
/// `entity.field`; a verification obligation and a runtime check.
#[derive(Debug, Clone)]
pub struct NarrowingSite {
    pub entity: String,
    pub field: String,
    pub kind: &'static str,
    pub expr: Expr,
}

/// An admitted migration.
#[derive(Debug, Clone)]
pub struct Migration {
    pub(crate) name: String,
    pub(crate) source: StoreSchema,
    pub(crate) target: StoreSchema,
    pub(crate) constants: Vec<Constant>,
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) transforms: BTreeMap<String, Transform>,
    pub(crate) retire: Vec<String>,
    pub(crate) narrowing: Vec<NarrowingSite>,
    pub(crate) hash: Hash,
    pub(crate) resolved: Json,
    pub(crate) summary: Json,
}

impl Migration {
    /// The migration's identity: the hash of its resolved form (`sha256:…`).
    pub fn hash(&self) -> String {
        admit::display(&self.hash)
    }
    /// The author's name for it (metadata, not part of the identity).
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn source_schema(&self) -> &str {
        &self.source.hash
    }
    pub fn target_schema(&self) -> &str {
        &self.target.hash
    }
    /// The source and target store schemas.
    pub fn schemas(&self) -> (&StoreSchema, &StoreSchema) {
        (&self.source, &self.target)
    }
    pub fn requirements(&self) -> &[Requirement] {
        &self.requirements
    }
    pub fn constants(&self) -> &[Constant] {
        &self.constants
    }
    /// The resolved transforms of the changed entity types, by entity name.
    pub fn transforms(&self) -> &BTreeMap<String, Transform> {
        &self.transforms
    }
    pub fn retired(&self) -> &[String] {
        &self.retire
    }
    pub fn narrowing_sites(&self) -> &[NarrowingSite] {
        &self.narrowing
    }
    /// The constants as evaluation values, by name.
    pub(crate) fn constant_values(&self) -> BTreeMap<String, Value> {
        self.constants
            .iter()
            .map(|c| (c.name.clone(), c.value.clone()))
            .collect()
    }
    /// The resolved document (migration IR 0.1): every target field explicit, automatic copies
    /// as `{"copy": f}`, drops listed. Admitting it again gives the same migration.
    pub fn resolved(&self) -> Json {
        self.resolved.clone()
    }
    /// The reviewable summary (FR-007c).
    pub fn summary(&self) -> Json {
        self.summary.clone()
    }
}
