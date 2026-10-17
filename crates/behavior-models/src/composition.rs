//! Composition: the model set, ownership, lowering order and the final pass over the admitted
//! module (contract `composition-and-ownership.md`).

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_engine::semantic::Module;
use behavior_engine::semantic::types::hash_display;
use behavior_engine::wire::{DerivedKind, Loc, Role, WParam, WType};
use serde_json::json;

use crate::error::{ModelError, sorted_unique};
use crate::state_machine::resolve::ResolvedStateMachine;

/// A model-crate failure reported in the Core admission error shape (`code`, `message`).
pub(crate) fn admission_failed_message(code: &str, message: String) -> ModelError {
    ModelError::CoreAdmissionFailed {
        errors: vec![(code.to_string(), message)],
    }
}

/// Core admission errors as model errors, verbatim.
pub(crate) fn admission_failed(report: behavior_engine::AdmissionResult) -> ModelError {
    ModelError::CoreAdmissionFailed {
        errors: report
            .errors
            .into_iter()
            .map(|e| (e.code, e.message))
            .collect(),
    }
}

/// `(entity, field) → owning model id` (data-model §8).
pub type OwnershipTable = BTreeMap<(String, String), String>;

/// A state machine of the internal composition set after lowering, with the actions it
/// generates. `supplied` is false for a machine derived from another model (feature 502): such
/// a machine is checked and lowered like any other, but gets no receipt of its own.
pub struct Composed {
    pub machine: ResolvedStateMachine,
    /// `action name → transition id` of every generated action.
    pub generated: BTreeMap<String, String>,
    pub supplied: bool,
}

/// An entity declaration a model contributes to the declaration environment (feature 502,
/// declaration-staging contract §3): its name and fields in canonical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredSchema {
    pub model_id: String,
    pub entity: String,
    pub fields: Vec<(String, WType)>,
    pub loc: Loc,
}

/// An enum declaration a model contributes (feature 503): its name and cases, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredEnum {
    pub model_id: String,
    pub name: String,
    pub cases: Vec<String>,
    pub loc: Loc,
}

/// A declaration a model contributes to the declaration environment (contracts
/// `declaration-dag.md` §2): an enum or an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contribution {
    Enum(DeclaredEnum),
    Entity(DeclaredSchema),
}

impl Contribution {
    pub fn model_id(&self) -> &str {
        match self {
            Contribution::Enum(e) => &e.model_id,
            Contribution::Entity(s) => &s.model_id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Contribution::Enum(e) => &e.name,
            Contribution::Entity(s) => &s.entity,
        }
    }

    /// The type names the declaration references (`Needs`): every enum, nominal and entity a
    /// field type names.
    pub fn references(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if let Contribution::Entity(s) = self {
            for (_, ty) in &s.fields {
                type_names(ty, &mut out);
            }
        }
        out
    }
}

/// Every declared type name `ty` mentions.
pub(crate) fn type_names(ty: &WType, out: &mut BTreeSet<String>) {
    match ty {
        WType::Option(inner) => type_names(inner, out),
        WType::Enum(n) | WType::Nominal(n) | WType::Id(n) | WType::Ref(n) | WType::Entity(n) => {
            out.insert(n.clone());
        }
        WType::Exact(Some(n)) => {
            out.insert(n.clone());
        }
        WType::Bool | WType::Int | WType::Decimal | WType::String | WType::Exact(None) => {}
    }
}

/// The deterministic topological order of a dependency graph (Kahn's algorithm; contracts
/// `declaration-dag.md` §2, laws D7, D8, D10). `(a, b)` in `edges` means `b` needs `a`. Ready
/// nodes are taken smallest first, so the order is a function of the node and edge *sets*,
/// independent of any insertion order. Edges naming unknown nodes and self-edges are ignored.
/// A cycle returns the nodes that could not be ordered, sorted.
pub fn topo_order<K: Ord + Clone>(
    nodes: BTreeSet<K>,
    edges: BTreeSet<(K, K)>,
) -> Result<Vec<K>, Vec<K>> {
    let mut indegree: BTreeMap<K, usize> = nodes.iter().map(|n| (n.clone(), 0)).collect();
    let mut out_edges: BTreeMap<K, BTreeSet<K>> = BTreeMap::new();
    for (from, to) in edges {
        if from == to || !nodes.contains(&from) || !nodes.contains(&to) {
            continue;
        }
        if out_edges.entry(from).or_default().insert(to.clone())
            && let Some(d) = indegree.get_mut(&to)
        {
            *d += 1;
        }
    }
    let mut ready: BTreeSet<K> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| n.clone())
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(next) = ready.pop_first() {
        indegree.remove(&next);
        for to in out_edges.remove(&next).unwrap_or_default() {
            if let Some(d) = indegree.get_mut(&to) {
                *d -= 1;
                if *d == 0 {
                    ready.insert(to);
                }
            }
        }
        order.push(next);
    }
    if indegree.is_empty() {
        Ok(order)
    } else {
        Err(indegree.into_keys().collect())
    }
}

/// Builds a definition's body in a derived scope with its parameters pushed.
type DefinitionBody = Box<dyn Fn(&mut Builder) -> Result<Node, BuildError>>;

/// A derived value a model contributes (contracts `declaration-dag.md` §2): its signature, the
/// other definitions its body references (`deps`), and how to build its body in a derived scope
/// with `params` already pushed.
pub struct DefinitionSpec {
    pub model_id: String,
    /// The model's own id of the definition (for example `approved`); `name` is its Core name.
    pub local: String,
    pub name: String,
    pub kind: DerivedKind,
    pub params: Vec<WParam>,
    pub result: WType,
    pub deps: BTreeSet<String>,
    pub loc: Loc,
    pub body: DefinitionBody,
}

impl std::fmt::Debug for DefinitionSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefinitionSpec")
            .field("model_id", &self.model_id)
            .field("name", &self.name)
            .field("deps", &self.deps)
            .finish_non_exhaustive()
    }
}

/// The signature of a contributed definition: fixed before any body is built, and returned to
/// the binding so that typed handles can be checked without a body (D10).
#[derive(Debug, Clone)]
pub struct DefinitionSignature {
    pub model_id: String,
    pub local: String,
    pub name: String,
    pub kind: DerivedKind,
    pub params: Vec<WParam>,
    pub result: WType,
}

impl DefinitionSpec {
    pub fn signature(&self) -> DefinitionSignature {
        DefinitionSignature {
            model_id: self.model_id.clone(),
            local: self.local.clone(),
            name: self.name.clone(),
            kind: self.kind,
            params: self.params.clone(),
            result: self.result.clone(),
        }
    }
}

/// The order in which definition bodies are added (law D10): every body after all its `deps`;
/// ready definitions by `(model id, name)`. A dependency that is not one of `specs` is external
/// (already in the builder) and adds no edge. A cycle is `DEFINITION_CYCLE` (names sorted). A
/// definition that depends on itself is a cycle too: [`topo_order`] ignores self-edges (a
/// model's declarations may name each other), so direct recursion is refused here.
pub fn definition_order(specs: &[DefinitionSpec]) -> Result<Vec<usize>, ModelError> {
    let recursive: BTreeSet<String> = specs
        .iter()
        .filter(|s| s.deps.contains(&s.name))
        .map(|s| s.name.clone())
        .collect();
    let key = |s: &DefinitionSpec| (s.model_id.clone(), s.name.clone());
    let by_name: BTreeMap<&str, (String, String)> =
        specs.iter().map(|s| (s.name.as_str(), key(s))).collect();
    let nodes: BTreeSet<(String, String)> = specs.iter().map(key).collect();
    let edges: BTreeSet<_> = specs
        .iter()
        .flat_map(|s| {
            s.deps
                .iter()
                .filter_map(|d| by_name.get(d.as_str()).map(|from| (from.clone(), key(s))))
                .collect::<Vec<_>>()
        })
        .collect();
    let index: BTreeMap<(String, String), usize> =
        specs.iter().enumerate().map(|(i, s)| (key(s), i)).collect();
    let cycle = match topo_order(nodes, edges) {
        Ok(order) if recursive.is_empty() => {
            return Ok(order.iter().filter_map(|k| index.get(k).copied()).collect());
        }
        Ok(_) => Vec::new(),
        Err(cycle) => cycle,
    };
    let names: BTreeSet<String> = cycle.into_iter().map(|(_, n)| n).chain(recursive).collect();
    Err(ModelError::DefinitionCycle {
        names: names.into_iter().collect(),
    })
}

/// Adds every definition to `builder` (contracts `declaration-dag.md` §2): all names are claimed
/// first, then the bodies are built and added in [`definition_order`]. Nothing is added when a
/// name is taken or the definitions form a cycle.
pub(crate) fn add_definitions(
    builder: &mut Builder,
    specs: &[DefinitionSpec],
) -> Result<(), Vec<ModelError>> {
    if specs.is_empty() {
        return Ok(());
    }
    let current = builder
        .finish(None)
        .map_err(|r| vec![admission_failed(r)])?;
    let taken: BTreeSet<&str> = current
        .name_table()
        .keys()
        .map(|(_, name)| name.as_str())
        .collect();
    let mut errors = Vec::new();
    let mut claimed = BTreeSet::new();
    for s in specs {
        if taken.contains(s.name.as_str()) || !claimed.insert(s.name.as_str()) {
            errors.push(ModelError::GeneratedNameCollision {
                model_ref: s.model_id.clone(),
                name: s.name.clone(),
            });
        }
    }
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }
    let order = definition_order(specs).map_err(|e| vec![e])?;
    for i in order {
        let s = &specs[i];
        let core = |e: BuildError| {
            vec![ModelError::FragmentInvalid {
                model_ref: format!("{}/{}", s.model_id, s.name),
                core_code: e.code,
                core_message: e.message,
            }]
        };
        builder
            .push_scope(ScopeSite::Derived, s.params.clone(), s.loc.clone())
            .map_err(core)?;
        let body = (s.body)(builder);
        builder.pop_scope();
        builder
            .add_derived(
                &s.name,
                s.kind,
                s.params.clone(),
                body.map_err(core)?,
                None,
                s.loc.clone(),
            )
            .map_err(core)?;
    }
    Ok(())
}

/// Law D4′: the module's derived item under a contributed name is exactly the contributed
/// definition. Core's derived items are compared by their semantic hash, through two ad-hoc
/// reads (Core's public probe of a module): one referring to the module's item, one to the
/// re-derived definition in its place. A derived reference is hashed by its target's hash, so
/// the two reads are equal exactly when the items are.
pub(crate) fn check_definition(module: &Module, spec: &DefinitionSpec) -> Option<ModelError> {
    let mismatch = || {
        Some(ModelError::DefinitionMismatch {
            name: spec.name.clone(),
            model_ref: spec.model_id.clone(),
        })
    };
    if !module.derived_items().contains_key(&spec.name) {
        return mismatch();
    }
    let read_params: Vec<WParam> = spec
        .params
        .iter()
        .map(|p| WParam {
            name: p.name.clone(),
            role: Some(Role::State),
            ty: p.ty.clone(),
        })
        .collect();
    let args: Vec<String> = spec.params.iter().map(|p| p.name.clone()).collect();
    let probe = |b: &mut Builder| -> Option<String> {
        b.push_scope(ScopeSite::Read, read_params.clone(), spec.loc.clone())
            .ok()?;
        let r = b.derived_ref(&spec.name, args.clone(), spec.loc.clone());
        b.pop_scope();
        let read = Builder::read_value(
            "__definition_probe",
            read_params.clone(),
            r.ok()?,
            spec.loc.clone(),
        );
        b.admit_read(&read)
            .ok()
            .map(|item| hash_display(item.hash()))
    };
    let actual = probe(&mut Builder::for_module(module));
    let mut expected_builder = Builder::for_module(module);
    let added = expected_builder
        .push_scope(ScopeSite::Derived, spec.params.clone(), spec.loc.clone())
        .ok()
        .and_then(|()| {
            let body = (spec.body)(&mut expected_builder);
            expected_builder.pop_scope();
            body.ok()
        })
        .and_then(|body| {
            expected_builder
                .add_derived(
                    &spec.name,
                    spec.kind,
                    spec.params.clone(),
                    body,
                    None,
                    spec.loc.clone(),
                )
                .ok()
        });
    let expected = added.and_then(|()| probe(&mut expected_builder));
    match (actual, expected) {
        (Some(a), Some(e)) if a == e => None,
        _ => mismatch(),
    }
}

/// How the declarations of `module` relate to a contributed schema.
pub(crate) enum Existing {
    /// No item has the schema's name.
    Absent,
    /// An entity with exactly the schema's fields (names, order, types and reference markers).
    Identical,
    /// Some other item has the name: its kind.
    Other(String),
}

/// Compares a contributed schema with what `module` declares (laws D4, D6).
pub(crate) fn existing(module: &Module, schema: &DeclaredSchema) -> Existing {
    if let Some(entity) = module.entities().get(&schema.entity) {
        // Core gives every entity its own identity field `id: Id(Self)`; it is not declared.
        let own_id = WType::Id(schema.entity.clone());
        let fields: Vec<(String, WType)> = entity
            .fields()
            .iter()
            .map(|(n, t)| (n.clone(), t.to_wire_type()))
            .filter(|(n, t)| !(n == "id" && *t == own_id))
            .collect();
        // Core admits `Ref(T)` as `Id(T)` and marks the field as a reference (`Option` too), so
        // the schema is compared in that normalized form, with its reference markers.
        let wanted: Vec<(String, WType)> = schema
            .fields
            .iter()
            .map(|(n, t)| (n.clone(), admitted_type(t)))
            .collect();
        let references: BTreeSet<&str> = fields
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| entity.is_reference(n))
            .collect();
        let wanted_references: BTreeSet<&str> = schema
            .fields
            .iter()
            .filter(|(_, t)| is_reference_type(t))
            .map(|(n, _)| n.as_str())
            .collect();
        return if fields == wanted && references == wanted_references {
            Existing::Identical
        } else {
            Existing::Other("an entity with other fields".into())
        };
    }
    match module
        .name_table()
        .keys()
        .find(|(_, name)| name == &schema.entity)
    {
        Some((kind, _)) => Existing::Other(kind.as_str().to_string()),
        None => Existing::Absent,
    }
}

/// A declared field type as Core admits it: every `Ref(T)` is the identity `Id(T)`.
fn admitted_type(t: &WType) -> WType {
    match t {
        WType::Ref(e) => WType::Id(e.clone()),
        WType::Option(inner) => WType::Option(Box::new(admitted_type(inner))),
        other => other.clone(),
    }
}

/// Whether Core marks a field of this declared type as a reference: `Ref(T)`, possibly optional.
fn is_reference_type(t: &WType) -> bool {
    match t {
        WType::Ref(_) => true,
        WType::Option(inner) => is_reference_type(inner),
        _ => false,
    }
}

/// How the declarations of `module` relate to a contributed enum (laws D4, D6).
pub(crate) fn existing_enum(module: &Module, e: &DeclaredEnum) -> Existing {
    if let Some(info) = module.enums().get(&e.name) {
        return if info.values == e.cases {
            Existing::Identical
        } else {
            Existing::Other("an enum with other cases".into())
        };
    }
    match module.name_table().keys().find(|(_, name)| name == &e.name) {
        Some((kind, _)) => Existing::Other(kind.as_str().to_string()),
        None => Existing::Absent,
    }
}

/// [`existing`] for any contribution.
pub(crate) fn existing_contribution(module: &Module, c: &Contribution) -> Existing {
    match c {
        Contribution::Entity(s) => existing(module, s),
        Contribution::Enum(e) => existing_enum(module, e),
    }
}

/// Declares one contribution in `builder` (entity fields in their canonical order).
pub(crate) fn declare_contribution(
    builder: &mut Builder,
    c: &Contribution,
) -> Result<(), BuildError> {
    match c {
        Contribution::Enum(e) => builder.declare_enum(&e.name, e.cases.clone(), e.loc.clone()),
        Contribution::Entity(schema) => {
            let fields = schema
                .fields
                .iter()
                .map(|(name, ty)| behavior_engine::wire::WField {
                    name: name.clone(),
                    ty: ty.clone(),
                    loc: schema.loc.clone(),
                })
                .collect();
            builder.declare_entity(&schema.entity, fields, schema.loc.clone())
        }
    }
}

/// The identical-or-error merge of one contribution (laws D3, D6) against `current`, the
/// declarations already in `builder`. `claimed` holds the names other models contributed: two
/// semantic origins of one name are a collision even when the declarations agree. Returns
/// whether the builder was changed.
pub(crate) fn merge_contribution(
    builder: &mut Builder,
    current: &Module,
    claimed: &mut BTreeMap<String, String>,
    c: &Contribution,
    errors: &mut Vec<ModelError>,
) -> bool {
    let collision = |existing_kind: String| ModelError::DeclarationCollision {
        name: c.name().to_string(),
        model_ref: c.model_id().to_string(),
        existing_kind,
    };
    if claimed.contains_key(c.name()) {
        errors.push(collision("another model's declaration".into()));
        return false;
    }
    claimed.insert(c.name().to_string(), c.model_id().to_string());
    match existing_contribution(current, c) {
        Existing::Identical => false,
        Existing::Other(kind) => {
            errors.push(collision(kind));
            false
        }
        Existing::Absent => match declare_contribution(builder, c) {
            Ok(()) => true,
            Err(e) => {
                errors.push(collision(e.message));
                false
            }
        },
    }
}

/// Declares `contributions` (in the given topological order) without first admitting the
/// authored declarations, then admits the result. Used when the authored declarations alone do
/// not admit because they name a contributed type (law D5: Core checks declarations together, so
/// such a forward reference is legal once the contribution is in place). A name already taken is
/// left as it is; the caller compares it with the contribution on the returned module.
pub(crate) fn declare_then_admit(
    builder: &mut Builder,
    contributions: &[Contribution],
) -> Result<Module, Vec<ModelError>> {
    for c in contributions {
        // `DUPLICATE_NAME`: an authored declaration holds the name; compared by the caller.
        let _ = declare_contribution(builder, c);
    }
    builder.finish(None).map_err(|r| vec![admission_failed(r)])
}

/// D6 after [`declare_then_admit`], for any contribution.
pub(crate) fn check_contribution(module: &Module, c: &Contribution) -> Option<ModelError> {
    match c {
        Contribution::Entity(s) => check_contributed(module, s),
        Contribution::Enum(e) => match existing_enum(module, e) {
            Existing::Identical => None,
            Existing::Other(kind) => Some(ModelError::DeclarationCollision {
                name: e.name.clone(),
                model_ref: e.model_id.clone(),
                existing_kind: kind,
            }),
            Existing::Absent => Some(ModelError::DeclarationMismatch {
                name: e.name.clone(),
                model_ref: e.model_id.clone(),
            }),
        },
    }
}

/// Law D4 for any contribution: at composition the builder holds exactly the contribution.
pub(crate) fn check_declared_contribution(module: &Module, c: &Contribution) -> Option<ModelError> {
    let identical = matches!(existing_contribution(module, c), Existing::Identical);
    (!identical).then(|| ModelError::DeclarationMismatch {
        name: c.name().to_string(),
        model_ref: c.model_id().to_string(),
    })
}

/// D6 after [`declare_then_admit`]: the declaration under the contributed name must be the
/// contribution itself (or an identical authored one).
pub(crate) fn check_contributed(module: &Module, schema: &DeclaredSchema) -> Option<ModelError> {
    match existing(module, schema) {
        Existing::Identical => None,
        Existing::Other(kind) => Some(ModelError::DeclarationCollision {
            name: schema.entity.clone(),
            model_ref: schema.model_id.clone(),
            existing_kind: kind,
        }),
        Existing::Absent => Some(ModelError::DeclarationMismatch {
            name: schema.entity.clone(),
            model_ref: schema.model_id.clone(),
        }),
    }
}

/// Law D4: at composition the builder must hold exactly the contributed declaration.
pub(crate) fn check_declared(module: &Module, schema: &DeclaredSchema) -> Option<ModelError> {
    match existing(module, schema) {
        Existing::Identical => None,
        _ => Some(ModelError::DeclarationMismatch {
            name: schema.entity.clone(),
            model_ref: schema.model_id.clone(),
        }),
    }
}

/// One owner per field (composition contract §2.2). Conflicts list their models sorted.
pub(crate) fn ownership_table<'a>(
    machines: impl Iterator<Item = &'a ResolvedStateMachine>,
    errors: &mut Vec<ModelError>,
) -> OwnershipTable {
    let mut claims: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for m in machines {
        claims
            .entry((m.subject.clone(), m.field.clone()))
            .or_default()
            .push(m.model_id.to_string());
    }
    let mut table = OwnershipTable::new();
    for ((entity, field), mut models) in claims {
        models.sort();
        if models.len() > 1 {
            errors.push(ModelError::ConflictingOwnership {
                entity,
                field,
                models,
            });
        } else if let Some(owner) = models.pop() {
            table.insert((entity, field), owner);
        }
    }
    table
}

/// The final composition pass over the admitted module (composition contract §4–§5).
///
/// In an admitted module, field values change only through action effects and creations, and
/// `actions()` lists every action. So if this pass accepts, every ordinary write of an owned
/// field lies in one of its owner's generated actions, and every new subject entity starts in
/// a literal entry state (spec FR-017, FR-021; law L6). Reads, removals, derived values,
/// invariants and command emissions never write a field.
pub(crate) fn final_pass(
    module: &Module,
    composed: &[Composed],
    ownership: &OwnershipTable,
) -> Vec<ModelError> {
    let mut errors = Vec::new();
    if ownership.is_empty() {
        return errors;
    }
    let generated_by: BTreeMap<&str, &str> = composed
        .iter()
        .flat_map(|c| {
            c.generated
                .keys()
                .map(move |name| (name.as_str(), c.machine.model_id.as_str()))
        })
        .collect();
    let machines: BTreeMap<&str, &ResolvedStateMachine> = composed
        .iter()
        .map(|c| (c.machine.model_id.as_str(), &c.machine))
        .collect();
    let mut literals: BTreeMap<String, Result<BTreeMap<String, [u8; 32]>, ModelError>> =
        BTreeMap::new();

    for (name, action) in module.actions() {
        for effect in action.effects() {
            let Some(param) = action.params().iter().find(|p| p.name() == effect.param()) else {
                continue;
            };
            let WType::Entity(entity) = param.ty().to_wire_type() else {
                continue;
            };
            let key = (entity, effect.field().to_string());
            // The exemption covers exactly the owner's generated action writing through its
            // subject parameter (feature 507, research R3 requirement 4): a state binding of the
            // subject type never inherits the machine's authority.
            let exempt = |owner: &String| {
                generated_by.get(name.as_str()) == Some(&owner.as_str())
                    && machines
                        .get(owner.as_str())
                        .is_some_and(|m| m.subject_param == effect.param())
            };
            if let Some(owner) = ownership.get(&key)
                && !exempt(owner)
            {
                errors.push(ModelError::UnauthorizedOwnedFieldWrite {
                    action: name.clone(),
                    entity: key.0.clone(),
                    field: key.1.clone(),
                    owner: owner.clone(),
                });
            }
        }
        for created in action.creates() {
            for ((entity, field), owner) in ownership {
                if created.entity() != entity {
                    continue;
                }
                let Some(machine) = machines.get(owner.as_str()) else {
                    continue;
                };
                let Some((_, initial)) = created.fields().iter().find(|(f, _)| f == field) else {
                    continue;
                };
                let lits = literals
                    .entry(owner.clone())
                    .or_insert_with(|| literal_hashes(module, &machine.enum_type));
                let lits = match lits {
                    Ok(l) => l,
                    Err(e) => {
                        errors.push(e.clone());
                        continue;
                    }
                };
                let case = lits.iter().find(|(_, h)| *h == initial.hash());
                match case {
                    Some((case, _)) if machine.entry_states.contains(case) => {}
                    Some((case, _)) => errors.push(ModelError::EntryStateNotAllowed {
                        action: name.clone(),
                        entity: entity.clone(),
                        field: field.clone(),
                        state: case.clone(),
                    }),
                    None => errors.push(ModelError::EntryStateNotStatic {
                        action: name.clone(),
                        entity: entity.clone(),
                        field: field.clone(),
                    }),
                }
            }
        }
    }
    errors
}

/// Core's semantic hash of every literal `lit(T, case)` (research R8). Core builds them in a
/// throwaway module that re-declares `T` with the module's cases and profile; the module is
/// never emitted. Literals hash through the enum declaration's hash, so the probe checks that its
/// enum is identical to the module's before trusting the literal hashes.
fn literal_hashes(
    module: &Module,
    enum_type: &str,
) -> Result<BTreeMap<String, [u8; 32]>, ModelError> {
    let probe_error = |message: String| ModelError::CoreAdmissionFailed {
        errors: vec![("MODEL_PROBE_FAILED".into(), message)],
    };
    let info = module
        .enums()
        .get(enum_type)
        .ok_or_else(|| probe_error(format!("unknown enum `{enum_type}`")))?;
    let loc = || Loc {
        file: "behavior-models.probe".into(),
        line: 1,
    };
    let core = |e: behavior_engine::builder::BuildError| probe_error(e.to_string());
    let mut b = Builder::with_semantic_profile(module.semantic_profile());
    b.declare_enum(&info.name, info.values.clone(), loc())
        .map_err(core)?;
    b.push_scope(ScopeSite::Closed, vec![], loc())
        .map_err(core)?;
    let ty = WType::Enum(info.name.clone());
    for (i, case) in info.values.iter().enumerate() {
        let a = b.lit(ty.clone(), json!(case), loc()).map_err(core)?;
        let c = b.lit(ty.clone(), json!(case), loc()).map_err(core)?;
        let eq = b.op("eq", vec![a, c], loc()).map_err(core)?;
        b.add_global_invariant(&format!("__probe_{i}"), eq, loc())
            .map_err(core)?;
    }
    b.pop_scope();
    let probe = b.finish(None).map_err(admission_failed)?;
    if probe.enums().get(enum_type).map(|e| e.hash) != Some(info.hash) {
        return Err(probe_error(format!(
            "the probe's `{enum_type}` differs from the module's"
        )));
    }
    let mut out = BTreeMap::new();
    for (i, case) in info.values.iter().enumerate() {
        let inv = probe
            .global_invariants()
            .get(&format!("__probe_{i}"))
            .ok_or_else(|| probe_error(format!("probe `{case}` missing")))?;
        let lit = inv
            .body()
            .children()
            .first()
            .map(|e| *e.hash())
            .ok_or_else(|| probe_error(format!("probe `{case}` has no literal")))?;
        out.insert(case.clone(), lit);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::{Config, RngSeed, TestRunner};

    fn set(xs: &[&str]) -> BTreeSet<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    fn edges(xs: &[(&str, &str)]) -> BTreeSet<(String, String)> {
        xs.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    // --- T006: the topological order of G_D (contracts `declaration-dag.md` §2) -------------

    #[test]
    fn without_edges_the_order_is_model_id_order() {
        let order = topo_order(set(&["c", "a", "b"]), edges(&[])).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn a_chain_is_ordered_by_its_edges() {
        // `a` needs a declaration `b` produces.
        let order = topo_order(set(&["a", "b"]), edges(&[("b", "a")])).unwrap();
        assert_eq!(order, vec!["b", "a"]);
    }

    #[test]
    fn diamond_ties_are_taken_in_id_order() {
        let order = topo_order(
            set(&["top", "left", "right", "bottom"]),
            edges(&[
                ("top", "right"),
                ("top", "left"),
                ("left", "bottom"),
                ("right", "bottom"),
            ]),
        )
        .unwrap();
        assert_eq!(order, vec!["top", "left", "right", "bottom"]);
    }

    #[test]
    fn a_cycle_names_exactly_its_models() {
        let err = topo_order(set(&["a", "b", "c"]), edges(&[("a", "b"), ("b", "a")])).unwrap_err();
        assert_eq!(err, vec!["a", "b"]);
    }

    #[test]
    fn self_and_unknown_edges_are_ignored() {
        let order = topo_order(set(&["a"]), edges(&[("a", "a"), ("x", "a")])).unwrap();
        assert_eq!(order, vec!["a"]);
    }

    fn runner(seed: u64) -> TestRunner {
        TestRunner::new_with_rng(
            Config {
                cases: 1000,
                rng_seed: RngSeed::Fixed(seed),
                ..Config::default()
            },
            proptest::test_runner::TestRng::deterministic_rng(
                proptest::test_runner::RngAlgorithm::ChaCha,
            ),
        )
    }

    /// Law D7: random DAGs (edges only from a smaller to a larger index of a random
    /// permutation), inserted in two random orders, give one order, and every edge is respected.
    #[test]
    fn the_order_is_a_function_of_the_graph() {
        let strategy = (2usize..9).prop_flat_map(|n| {
            (
                Just(n),
                Just((0..n).collect::<Vec<_>>()).prop_shuffle(),
                proptest::collection::vec((0..n, 0..n), 0..20),
            )
        });
        runner(503)
            .run(&strategy, |(n, rank, raw)| {
                let names: Vec<String> = (0..n).map(|i| format!("m{i}")).collect();
                let pairs: Vec<(String, String)> = raw
                    .iter()
                    .filter(|(a, b)| rank[*a] < rank[*b])
                    .map(|(a, b)| (names[*a].clone(), names[*b].clone()))
                    .collect();
                let one = topo_order(
                    names.iter().cloned().collect(),
                    pairs.iter().cloned().collect(),
                )
                .unwrap();
                let two = topo_order(
                    names.iter().rev().cloned().collect(),
                    pairs.iter().rev().cloned().collect(),
                )
                .unwrap();
                prop_assert_eq!(&one, &two);
                let at: BTreeMap<&String, usize> =
                    one.iter().enumerate().map(|(i, n)| (n, i)).collect();
                for (a, b) in &pairs {
                    prop_assert!(at[a] < at[b], "{a} before {b}");
                }
                Ok(())
            })
            .unwrap();
    }

    // --- T007: definitions, signatures first, bodies in dependency order (law D10) ---------

    fn spec(model: &str, name: &str, deps: &[&str]) -> DefinitionSpec {
        DefinitionSpec {
            model_id: model.into(),
            local: name.into(),
            name: name.into(),
            kind: DerivedKind::Rule,
            params: Vec::new(),
            result: WType::Bool,
            deps: deps.iter().map(|d| d.to_string()).collect(),
            loc: Loc {
                file: "t.model".into(),
                line: 1,
            },
            body: Box::new(|b: &mut Builder| {
                b.lit(
                    WType::Bool,
                    json!(true),
                    Loc {
                        file: "t.model".into(),
                        line: 1,
                    },
                )
            }),
        }
    }

    fn names(specs: &[DefinitionSpec], order: &[usize]) -> Vec<String> {
        order.iter().map(|i| specs[*i].name.clone()).collect()
    }

    #[test]
    fn bodies_follow_their_dependencies() {
        // Supplied in reverse dependency order.
        let specs = vec![
            spec("p", "p__open", &["p__high", "p__low"]),
            spec("p", "p__high", &["p__total"]),
            spec("p", "p__low", &[]),
            spec("p", "p__total", &[]),
        ];
        let order = definition_order(&specs).unwrap();
        assert_eq!(
            names(&specs, &order),
            // Ready ties by name: `p__low` before `p__total`.
            vec!["p__low", "p__total", "p__high", "p__open"]
        );
    }

    #[test]
    fn ready_ties_go_by_model_id_then_name() {
        let specs = vec![
            spec("b", "b__x", &[]),
            spec("a", "a__y", &[]),
            spec("a", "a__x", &[]),
        ];
        let order = definition_order(&specs).unwrap();
        assert_eq!(names(&specs, &order), vec!["a__x", "a__y", "b__x"]);
    }

    #[test]
    fn a_definition_cycle_is_refused_and_nothing_is_added() {
        let specs = vec![spec("p", "f", &["g"]), spec("p", "g", &["f"])];
        assert_eq!(
            definition_order(&specs).unwrap_err(),
            ModelError::DefinitionCycle {
                names: vec!["f".into(), "g".into()]
            }
        );
        let mut b = Builder::new();
        b.declare_entity(
            "E",
            vec![],
            Loc {
                file: "t.model".into(),
                line: 1,
            },
        )
        .unwrap();
        let before = b.finish(None).unwrap();
        let errs = add_definitions(&mut b, &specs).unwrap_err();
        assert_eq!(errs[0].code(), "DEFINITION_CYCLE");
        assert_eq!(b.finish(None).unwrap().hash(), before.hash());
    }

    #[test]
    fn references_are_compared_in_admitted_form_with_their_markers() {
        let loc = || Loc {
            file: "t.model".into(),
            line: 1,
        };
        let field = |name: &str, ty: WType| behavior_engine::wire::WField {
            name: name.into(),
            ty,
            loc: loc(),
        };
        let opt_ref = || WType::Option(Box::new(WType::Ref("T".into())));
        let mut b = Builder::new();
        b.declare_entity("T", vec![], loc()).unwrap();
        b.declare_entity(
            "E",
            vec![field("o", opt_ref()), field("r", WType::Ref("T".into()))],
            loc(),
        )
        .unwrap();
        let module = b.finish(None).unwrap();
        let schema = |o: WType, r: WType| DeclaredSchema {
            model_id: "p".into(),
            entity: "E".into(),
            fields: vec![("o".into(), o), ("r".into(), r)],
            loc: loc(),
        };
        let same = schema(opt_ref(), WType::Ref("T".into()));
        assert!(matches!(existing(&module, &same), Existing::Identical));
        // The same admitted types without the reference markers are another entity.
        for other in [
            schema(opt_ref(), WType::Id("T".into())),
            schema(
                WType::Option(Box::new(WType::Id("T".into()))),
                WType::Ref("T".into()),
            ),
        ] {
            assert!(matches!(existing(&module, &other), Existing::Other(_)));
        }
    }

    #[test]
    fn a_self_dependent_definition_is_a_cycle() {
        // Direct recursion: `topo_order` ignores self-edges, so this must be refused explicitly.
        let specs = vec![spec("p", "f", &["f"]), spec("p", "g", &[])];
        assert_eq!(
            definition_order(&specs).unwrap_err(),
            ModelError::DefinitionCycle {
                names: vec!["f".into()]
            }
        );
        // A self-dependency next to a longer cycle: both are named, sorted.
        let specs = vec![
            spec("p", "h", &["h"]),
            spec("p", "f", &["g"]),
            spec("p", "g", &["f"]),
        ];
        assert_eq!(
            definition_order(&specs).unwrap_err(),
            ModelError::DefinitionCycle {
                names: vec!["f".into(), "g".into(), "h".into()]
            }
        );
        let mut b = Builder::new();
        let before = b.finish(None).unwrap();
        let errs = add_definitions(&mut b, &[spec("p", "f", &["f"])]).unwrap_err();
        assert_eq!(errs[0].code(), "DEFINITION_CYCLE");
        assert_eq!(b.finish(None).unwrap().hash(), before.hash());
    }

    #[test]
    fn a_claimed_name_is_a_generated_name_collision() {
        let mut b = Builder::new();
        let loc = || Loc {
            file: "t.model".into(),
            line: 1,
        };
        b.declare_entity("E", vec![], loc()).unwrap();
        b.push_scope(ScopeSite::Derived, vec![], loc()).unwrap();
        let t = b.lit(WType::Bool, json!(true), loc()).unwrap();
        b.pop_scope();
        b.add_derived("p__high", DerivedKind::Rule, vec![], t, None, loc())
            .unwrap();
        let errs = add_definitions(&mut b, &[spec("p", "p__high", &[])]).unwrap_err();
        assert_eq!(
            errs,
            vec![ModelError::GeneratedNameCollision {
                model_ref: "p".into(),
                name: "p__high".into()
            }]
        );
        // Two models claiming one name.
        let mut b = Builder::new();
        b.declare_entity("E", vec![], loc()).unwrap();
        let errs =
            add_definitions(&mut b, &[spec("p", "x", &[]), spec("q", "x", &[])]).unwrap_err();
        assert_eq!(errs[0].code(), "GENERATED_NAME_COLLISION");
    }

    #[test]
    fn signatures_need_no_body() {
        let s = spec("p", "p__high", &["p__total"]);
        let sig = s.signature();
        assert_eq!((sig.name.as_str(), sig.result), ("p__high", WType::Bool));
    }

    /// Law D10: the order is a function of the definition *set*.
    #[test]
    fn the_definition_order_is_a_function_of_the_set() {
        let strategy = (2usize..8).prop_flat_map(|n| {
            (
                Just(n),
                proptest::collection::vec((0..n, 0..n), 0..14),
                Just((0..n).collect::<Vec<_>>()).prop_shuffle(),
            )
        });
        runner(10)
            .run(&strategy, |(n, raw, shuffle)| {
                let name = |i: usize| format!("m{}__f{i}", i % 3);
                // deps only on smaller indices: acyclic
                let deps = |i: usize| -> Vec<String> {
                    raw.iter()
                        .filter(|(a, b)| *a == i && *b < i)
                        .map(|(_, b)| name(*b))
                        .collect()
                };
                let make = |i: usize| {
                    let d = deps(i);
                    let refs: Vec<&str> = d.iter().map(String::as_str).collect();
                    spec(&format!("m{}", i % 3), &name(i), &refs)
                };
                let straight: Vec<DefinitionSpec> = (0..n).map(make).collect();
                let shuffled: Vec<DefinitionSpec> = shuffle.iter().map(|i| make(*i)).collect();
                let a = names(&straight, &definition_order(&straight).unwrap());
                let b = names(&shuffled, &definition_order(&shuffled).unwrap());
                prop_assert_eq!(&a, &b);
                let at: BTreeMap<&String, usize> =
                    a.iter().enumerate().map(|(i, n)| (n, i)).collect();
                for i in 0..n {
                    for d in deps(i) {
                        prop_assert!(at[&d] < at[&name(i)]);
                    }
                }
                Ok(())
            })
            .unwrap();
    }

    /// T005 (Rust part): a set without contributions and definitions makes no builder call.
    #[test]
    fn no_contributions_means_no_builder_call() {
        let mut b = Builder::new();
        b.declare_entity(
            "E",
            vec![],
            Loc {
                file: "t.model".into(),
                line: 1,
            },
        )
        .unwrap();
        let before = b.finish(None).unwrap();
        add_definitions(&mut b, &[]).unwrap();
        assert_eq!(b.finish(None).unwrap().hash(), before.hash());
    }
}
