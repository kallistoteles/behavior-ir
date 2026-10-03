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
    Constraint = 7,
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
            Kind::Constraint => "constraint",
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
    /// A parameter (feature 009: the verifier binds a migration's old entity as one).
    pub fn new(name: impl Into<String>, role: ParamRole, ty: Type) -> Param {
        Param {
            name: name.into(),
            role,
            ty,
        }
    }
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
    /// Fields declared `Ref<T>` (feature 006): `Id<T>` fields whose target must exist.
    pub(crate) references: Vec<String>,
}

impl EntityItem {
    /// Whether `field` is a reference (`Ref<T>`), not a plain identity.
    pub fn is_reference(&self, field: &str) -> bool {
        self.references.iter().any(|f| f == field)
    }
    /// Reference fields with their target entity type, in declaration order.
    pub fn reference_fields(&self) -> impl Iterator<Item = (&str, &str)> {
        self.fields.iter().filter_map(|(n, t)| {
            let target = match t {
                Type::Id(e) => e,
                Type::Option(inner) => match inner.as_ref() {
                    Type::Id(e) => e,
                    _ => return None,
                },
                _ => return None,
            };
            self.is_reference(n)
                .then_some((n.as_str(), target.as_str()))
        })
    }
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
    /// The author's declared type (equal to the body's type; not hashed).
    pub(crate) declared: Option<Type>,
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

/// What a module-level invariant depends on (feature 007): the entity types it queries and, per
/// type, the fields its lambdas read (`None`: possibly every field).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Signature {
    pub types: BTreeMap<String, Option<std::collections::BTreeSet<String>>>,
}

/// A module-level invariant (feature 007): a closed state expression over the whole state.
#[derive(Debug, Clone)]
pub struct GlobalInvariantItem {
    pub(crate) body: Expr,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
    pub(crate) signature: Signature,
}

impl GlobalInvariantItem {
    pub fn body(&self) -> &Expr {
        &self.body
    }
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

/// An entity constraint: what a valid instance of an entity type is (feature 002).
#[derive(Debug, Clone)]
pub struct ConstraintItem {
    pub(crate) entity: String,
    pub(crate) param: String,
    pub(crate) body: Expr,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
    /// The reference field of a constraint synthesized from `Ref<T>` (feature 006); such
    /// constraints are carried by the entity's hash and never serialized.
    pub(crate) reference: Option<String>,
}

impl ConstraintItem {
    /// The field of a synthesized reference constraint (`exists(field)`), if this is one.
    pub fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }
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

/// `create(T, id, {fields})` (feature 006): a new entity with a complete initial value.
#[derive(Debug, Clone)]
pub struct CreateEffect {
    pub(crate) entity: String,
    pub(crate) id: Expr,
    /// Every declared field except `id`, in declaration order, converted to the field's type.
    pub(crate) fields: Vec<(String, Expr)>,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

impl CreateEffect {
    pub fn entity(&self) -> &str {
        &self.entity
    }
    pub fn id(&self) -> &Expr {
        &self.id
    }
    pub fn fields(&self) -> &[(String, Expr)] {
        &self.fields
    }
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

/// `remove(p)` (feature 006): the entity bound to state parameter `p` is absent from S'.
#[derive(Debug, Clone)]
pub struct RemoveEffect {
    pub(crate) param: String,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

impl RemoveEffect {
    pub fn param(&self) -> &str {
        &self.param
    }
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub(crate) expr: Expr,
    pub(crate) loc: Loc,
}

impl Condition {
    pub fn expr(&self) -> &Expr {
        &self.expr
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

impl Effect {
    pub fn param(&self) -> &str {
        &self.param
    }
    pub fn field(&self) -> &str {
        &self.field
    }
    pub fn value(&self) -> &Expr {
        &self.value
    }
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
    pub fn loc(&self) -> &Loc {
        &self.loc
    }
}

#[derive(Debug, Clone)]
pub struct ActionItem {
    pub(crate) params: Vec<Param>,
    pub(crate) preconditions: Vec<Condition>,
    pub(crate) effects: Vec<Effect>,
    pub(crate) creates: Vec<CreateEffect>,
    pub(crate) removes: Vec<RemoveEffect>,
    pub(crate) postconditions: Vec<Condition>,
    pub(crate) hash: Hash,
    pub(crate) loc: Loc,
}

impl ActionItem {
    pub fn params(&self) -> &[Param] {
        &self.params
    }
    pub fn preconditions(&self) -> &[Condition] {
        &self.preconditions
    }
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    pub fn postconditions(&self) -> &[Condition] {
        &self.postconditions
    }
    pub fn creates(&self) -> &[CreateEffect] {
        &self.creates
    }
    pub fn removes(&self) -> &[RemoveEffect] {
        &self.removes
    }
    /// Whether the action changes the entity universe (feature 006).
    pub fn has_lifecycle(&self) -> bool {
        !self.creates.is_empty() || !self.removes.is_empty()
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
    pub(crate) global_invariants: BTreeMap<String, GlobalInvariantItem>,
    pub(crate) constraints: BTreeMap<String, ConstraintItem>,
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
    pub fn entities(&self) -> &BTreeMap<String, EntityItem> {
        &self.entities
    }
    pub fn actions(&self) -> &BTreeMap<String, ActionItem> {
        &self.actions
    }
    pub fn derived_items(&self) -> &BTreeMap<String, DerivedItem> {
        &self.derived
    }
    pub fn invariants(&self) -> &BTreeMap<String, InvariantItem> {
        &self.invariants
    }
    pub fn constraints(&self) -> &BTreeMap<String, ConstraintItem> {
        &self.constraints
    }
    /// Module-level invariants (feature 007), in name order.
    pub fn global_invariants(&self) -> &BTreeMap<String, GlobalInvariantItem> {
        &self.global_invariants
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
    /// Entity constraints of the given entity type, in name order.
    pub fn constraints_for<'a>(
        &'a self,
        entity: &'a str,
    ) -> impl Iterator<Item = (&'a String, &'a ConstraintItem)> + 'a {
        self.constraints
            .iter()
            .filter(move |(_, c)| c.entity == entity)
    }
    /// Reference fields of every entity that point at `target`: `(entity, field)`, sorted.
    pub fn references_to(&self, target: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (name, e) in &self.entities {
            for (field, t) in e.reference_fields() {
                if t == target {
                    out.push((name.clone(), field.to_string()));
                }
            }
        }
        out
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
