//! Semantic items and the module. Constructed only by `admit`.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::semantic::expr::Expr;
use crate::semantic::types::{EnumInfo, Hash, NominalInfo, Type};
use crate::wire::{DerivedKind, Loc, Role};

/// Item kinds in the module name table (values are the hash encoding, contracts/hashing.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Enum = 1,
    Nominal = 2,
    Entity = 3,
    Derived = 4,
    Invariant = 5,
    Action = 6,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Enum => "enum",
            Kind::Nominal => "nominal",
            Kind::Entity => "entity",
            Kind::Derived => "derived",
            Kind::Invariant => "invariant",
            Kind::Action => "action",
        }
    }
}

/// Parameter role; derived values and invariants have `Read` parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamRole {
    Read,
    State,
    Input,
    Context,
}

impl From<Role> for ParamRole {
    fn from(r: Role) -> Self {
        match r {
            Role::State => ParamRole::State,
            Role::Input => ParamRole::Input,
            Role::Context => ParamRole::Context,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub(crate) name: String,
    pub(crate) role: ParamRole,
    pub(crate) ty: Type,
}

impl Param {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn role(&self) -> ParamRole {
        self.role
    }
    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[derive(Debug, Clone)]
pub struct EntityItem {
    pub(crate) name: String,
    /// Fields in declaration order, with the implicit `id: Id<Self>` first.
    pub(crate) fields: Vec<(String, Type)>,
    pub(crate) hash: Hash,
    /// Source metadata (outside the hash): the entity and each declared field (not `id`).
    pub(crate) loc: Loc,
    pub(crate) field_locs: Vec<Loc>,
}

impl EntityItem {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn fields(&self) -> &[(String, Type)] {
        &self.fields
    }
    pub fn field_type(&self, name: &str) -> Option<&Type> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }
}

#[derive(Debug, Clone)]
pub struct DerivedItem {
    pub(crate) kind: DerivedKind,
    pub(crate) params: Vec<Param>,
    pub(crate) body: Expr,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

impl DerivedItem {
    pub fn params(&self) -> &[Param] {
        &self.params
    }
    pub fn body(&self) -> &Expr {
        &self.body
    }
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn is_rule(&self) -> bool {
        self.kind == DerivedKind::Rule
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

#[derive(Debug, Clone)]
pub struct InvariantItem {
    pub(crate) entity: String,
    pub(crate) param: String,
    pub(crate) body: Expr,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

impl InvariantItem {
    pub fn entity(&self) -> &str {
        &self.entity
    }
    pub fn param(&self) -> &str {
        &self.param
    }
    pub fn body(&self) -> &Expr {
        &self.body
    }
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub(crate) param: String,
    pub(crate) field: String,
    pub(crate) value: Expr,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub(crate) expr: Expr,
    pub(crate) loc: Loc,
}

#[derive(Debug, Clone)]
pub struct ActionItem {
    pub(crate) params: Vec<Param>,
    pub(crate) preconditions: Vec<Condition>,
    pub(crate) effects: Vec<Effect>,
    pub(crate) postconditions: Vec<Condition>,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

impl ActionItem {
    pub fn params(&self) -> &[Param] {
        &self.params
    }
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

/// An admitted behavior module. Its hash is the behavior version.
#[derive(Debug, Clone)]
pub struct Module {
    pub(crate) enums: BTreeMap<String, Arc<EnumInfo>>,
    pub(crate) nominals: BTreeMap<String, Arc<NominalInfo>>,
    pub(crate) entities: BTreeMap<String, EntityItem>,
    pub(crate) derived: BTreeMap<String, DerivedItem>,
    pub(crate) invariants: BTreeMap<String, InvariantItem>,
    pub(crate) actions: BTreeMap<String, ActionItem>,
    pub(crate) name_table: BTreeMap<(Kind, String), Hash>,
    pub(crate) evaluation_order: Vec<String>,
    /// Source metadata (outside the hash).
    pub(crate) enum_locs: BTreeMap<String, Loc>,
    pub(crate) nominal_locs: BTreeMap<String, Loc>,
    pub(crate) hash: Hash,
}

impl Module {
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn behavior_version(&self) -> String {
        crate::semantic::types::hash_display(&self.hash)
    }
    pub fn evaluation_order(&self) -> &[String] {
        &self.evaluation_order
    }
    pub fn name_table(&self) -> &BTreeMap<(Kind, String), Hash> {
        &self.name_table
    }
    pub fn enums(&self) -> &BTreeMap<String, Arc<EnumInfo>> {
        &self.enums
    }
    pub fn nominals(&self) -> &BTreeMap<String, Arc<NominalInfo>> {
        &self.nominals
    }
    pub fn entity(&self, name: &str) -> Option<&EntityItem> {
        self.entities.get(name)
    }
    pub fn action(&self, name: &str) -> Option<&ActionItem> {
        self.actions.get(name)
    }
    pub fn derived(&self, name: &str) -> Option<&DerivedItem> {
        self.derived.get(name)
    }
    /// Invariants that constrain the given entity, in name order.
    pub fn invariants_for<'a>(
        &'a self,
        entity: &'a str,
    ) -> impl Iterator<Item = (&'a String, &'a InvariantItem)> + 'a {
        self.invariants
            .iter()
            .filter(move |(_, i)| i.entity == entity)
    }
}
