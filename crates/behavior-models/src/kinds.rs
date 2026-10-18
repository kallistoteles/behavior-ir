//! The model-kind registry (feature 502): the one place that knows every model kind and
//! orchestrates them over the kind-neutral building blocks of `composition`, `identity` and
//! `source_map`.
//!
//! ```text
//! P2  declare_models   Decls(m) for every model in a topological order of the declaration DAG
//!                      G_D, merged identical-or-error into the builder; then every model's
//!                      definitions (signatures first, bodies in dependency order)
//! P4  compose_models   resolve (D4, D4′) → set checks → lower (creations, decisions;
//!                      transitions through the StateMachine lowerer) → Core admission → final
//!                      pass → identity
//! ```
//!
//! The supplied model set (receipts, the id rule) and the internal composition set (every state
//! machine, including the one each flow derives) are kept apart: a derived machine is checked and
//! lowered like any other, but is not a member of the supplied set.

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::builder::Builder;

use crate::BuildOutput;

/// Preserve all established ownership passes before applying additional restrictions.
pub fn compose_models_with_safety(
    builder: &mut Builder,
    models: Vec<AnyModel<'_>>,
    root: Option<&str>,
    policies: &[crate::safety::FieldOwnershipInput],
) -> Result<crate::SafetyBuildOutput, crate::SafetyBuildError> {
    let behavior =
        compose_models(builder, models, root).map_err(crate::SafetyBuildError::Models)?;
    let analysis = crate::safety::analyze(&behavior.module, policies)?;
    Ok(crate::SafetyBuildOutput { behavior, analysis })
}
use crate::approval::input::ApprovalSpec;
use crate::approval::lower::lower_decision;
use crate::approval::resolve::ResolvedApproval;
use crate::composition::{
    Composed, Contribution, DeclaredEnum, DeclaredSchema, DefinitionSignature, DefinitionSpec,
    add_definitions, admission_failed, check_contribution, check_declared,
    check_declared_contribution, check_definition, declare_then_admit, final_pass,
    merge_contribution, ownership_table, topo_order,
};
use crate::error::{ModelError, sorted_unique};
use crate::request_flow::create::lower_creation;
use crate::request_flow::identity::receipt_rf;
use crate::request_flow::input::RequestFlowSpec;
use crate::request_flow::resolve::{ResolvedRequestFlow, resolve as resolve_flow};
use crate::state_machine::input::{FragmentSource, ModelSpec};
use crate::state_machine::lower::lower_transition;
use crate::state_machine::resolve::resolve as resolve_machine;
use crate::work_queue::input::WorkQueueSpec;
use crate::work_queue::lower::{QUEUE_RULE, QueueSource, lower_enqueue};
use crate::work_queue::resolve::ResolvedWorkQueue;

/// A supplied model of any kind.
pub enum AnyModel<'a> {
    StateMachine(ModelSpec<'a>),
    RequestFlow(RequestFlowSpec<'a>),
    Approval(ApprovalSpec<'a>),
    WorkQueue(WorkQueueSpec<'a>),
}

impl<'a> From<WorkQueueSpec<'a>> for AnyModel<'a> {
    fn from(m: WorkQueueSpec<'a>) -> Self {
        AnyModel::WorkQueue(m)
    }
}

impl<'a> From<ModelSpec<'a>> for AnyModel<'a> {
    fn from(m: ModelSpec<'a>) -> Self {
        AnyModel::StateMachine(m)
    }
}

impl<'a> From<RequestFlowSpec<'a>> for AnyModel<'a> {
    fn from(m: RequestFlowSpec<'a>) -> Self {
        AnyModel::RequestFlow(m)
    }
}

impl<'a> From<ApprovalSpec<'a>> for AnyModel<'a> {
    fn from(m: ApprovalSpec<'a>) -> Self {
        AnyModel::Approval(m)
    }
}

impl AnyModel<'_> {
    pub fn model_id(&self) -> &str {
        match self {
            AnyModel::StateMachine(m) => &m.input.model_id,
            AnyModel::RequestFlow(f) => &f.input.flow_id,
            AnyModel::Approval(a) => &a.input.approval_id,
            AnyModel::WorkQueue(q) => &q.input.queue_id,
        }
    }

    /// `Contrib(M)` from the model input alone (law D1).
    fn input_contributions(&self) -> Result<Vec<Contribution>, Vec<ModelError>> {
        match self {
            AnyModel::StateMachine(_) => Ok(Vec::new()),
            AnyModel::RequestFlow(f) => crate::request_flow::resolve::input_schema(&f.input)
                .map(|s| vec![Contribution::Entity(s)]),
            AnyModel::Approval(a) => crate::approval::resolve::input_contributions(&a.input),
            AnyModel::WorkQueue(q) => crate::work_queue::resolve::input_contributions(&q.input),
        }
    }

    /// `Contrib(M)`, checked against the declarations of `module`.
    fn decls(
        &self,
        module: &behavior_engine::semantic::Module,
    ) -> Result<Vec<Contribution>, Vec<ModelError>> {
        match self {
            AnyModel::StateMachine(_) => Ok(Vec::new()),
            AnyModel::RequestFlow(f) => crate::request_flow::decls::decls(&f.input, module)
                .map(|s| vec![Contribution::Entity(s)]),
            AnyModel::Approval(a) => crate::approval::resolve::decls(&a.input, module),
            AnyModel::WorkQueue(q) => crate::work_queue::resolve::decls(&q.input, module),
        }
    }

    /// `Needs(M)`: the type names its contributions reference and do not declare themselves.
    fn needs(&self) -> BTreeSet<String> {
        match self {
            AnyModel::Approval(a) => crate::approval::decls::needs(&a.input),
            other => {
                let Ok(contributions) = other.input_contributions() else {
                    return BTreeSet::new();
                };
                let own: BTreeSet<&str> = contributions.iter().map(Contribution::name).collect();
                contributions
                    .iter()
                    .flat_map(Contribution::references)
                    .filter(|n| !own.contains(n.as_str()))
                    .collect()
            }
        }
    }

    /// The definitions the model contributes (contracts `declaration-dag.md` §2).
    fn definitions(&self) -> Vec<DefinitionSpec> {
        match self {
            AnyModel::Approval(a) => crate::approval::decls::definitions(&a.input),
            AnyModel::WorkQueue(q) => crate::work_queue::decls::definitions(&q.input),
            AnyModel::StateMachine(_) | AnyModel::RequestFlow(_) => Vec::new(),
        }
    }
}

/// What P2 declared: the contributed entities and enums (in topological order) and the
/// signatures of the contributed definitions.
#[derive(Debug, Clone, Default)]
pub struct Staged {
    pub schemas: Vec<DeclaredSchema>,
    pub enums: Vec<DeclaredEnum>,
    pub definitions: Vec<DefinitionSignature>,
}

impl Staged {
    /// Whether P2 declared nothing at all.
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty() && self.enums.is_empty() && self.definitions.is_empty()
    }

    fn push(&mut self, c: Contribution) {
        match c {
            Contribution::Entity(s) => self.schemas.push(s),
            Contribution::Enum(e) => self.enums.push(e),
        }
    }
}

/// P2: declares every model's contributed declarations and definitions (declaration-staging
/// contract, extended by contracts `declaration-dag.md`). For a set without contributions (every
/// StateMachine-only set) the builder is not touched (law D2).
///
/// Declarations are produced in the topological order of the model-declaration DAG `G_D`
/// (`M_i → M_j` iff `M_j` needs a type `M_i` contributes), ready models by model id (law D7). A
/// cycle is `DECLARATION_CYCLE` before anything is declared (law D8). For sets without edges (every
/// 501 and 502 set) this is model-id order, the order of feature 502 (law D9). Then every
/// definition is added: names claimed first, bodies in dependency order (law D10).
pub fn declare_models(
    builder: &mut Builder,
    models: &[AnyModel<'_>],
) -> Result<Staged, Vec<ModelError>> {
    let contributing: Vec<&AnyModel<'_>> = models
        .iter()
        .filter(|m| !matches!(m, AnyModel::StateMachine(_)))
        .collect();
    if contributing.is_empty() {
        return Ok(Staged::default());
    }
    // One id namespace for every kind: a duplicate id is reported as such, not as the
    // declaration clash it causes.
    let mut ids = BTreeMap::<&str, usize>::new();
    for m in models {
        *ids.entry(m.model_id()).or_default() += 1;
    }
    let duplicates: Vec<ModelError> = ids
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(id, _)| ModelError::DuplicateModelId {
            model_id: id.to_string(),
        })
        .collect();
    if !duplicates.is_empty() {
        return Err(duplicates);
    }

    // The graph G_D, from the inputs alone.
    let mut errors = Vec::new();
    let mut contributions: BTreeMap<&str, Vec<Contribution>> = BTreeMap::new();
    for m in &contributing {
        match m.input_contributions() {
            Ok(cs) => {
                contributions.insert(m.model_id(), cs);
            }
            Err(es) => errors.extend(es),
        }
    }
    let producer: BTreeMap<&str, &str> = contributions
        .iter()
        .flat_map(|(id, cs)| cs.iter().map(move |c| (c.name(), *id)))
        .collect();
    let nodes: BTreeSet<String> = contributing
        .iter()
        .map(|m| m.model_id().to_string())
        .collect();
    let edges: BTreeSet<(String, String)> = contributing
        .iter()
        .flat_map(|m| {
            m.needs()
                .into_iter()
                .filter_map(|n| {
                    producer
                        .get(n.as_str())
                        .map(|p| (p.to_string(), m.model_id().to_string()))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let order = match topo_order(nodes, edges) {
        Ok(order) => order,
        Err(models) => {
            errors.push(ModelError::DeclarationCycle { models });
            return Err(sorted_unique(errors));
        }
    };
    let by_id: BTreeMap<&str, &AnyModel<'_>> =
        contributing.iter().map(|m| (m.model_id(), *m)).collect();

    let mut staged = Staged::default();
    let mut claimed = BTreeMap::new();
    if let Ok(authored) = builder.finish(None) {
        let mut current = authored;
        for id in &order {
            let Some(m) = by_id.get(id.as_str()) else {
                continue;
            };
            if !contributions.contains_key(id.as_str()) {
                continue; // its input errors are already reported
            }
            match m.decls(&current) {
                Ok(cs) => {
                    let mut changed = false;
                    for c in &cs {
                        changed |=
                            merge_contribution(builder, &current, &mut claimed, c, &mut errors);
                    }
                    if changed {
                        current = builder
                            .finish(None)
                            .map_err(|r| vec![admission_failed(r)])?;
                    }
                    cs.into_iter().for_each(|c| staged.push(c));
                }
                Err(es) => errors.extend(es),
            }
        }
    } else {
        // The authored declarations name a contributed type (law D5): declare the contributions
        // from the inputs in topological order, and check everything on the completed
        // declarations.
        if !errors.is_empty() {
            return Err(sorted_unique(errors));
        }
        let ordered: Vec<Contribution> = order
            .iter()
            .filter_map(|id| contributions.get(id.as_str()))
            .flatten()
            .cloned()
            .collect();
        let declared = declare_then_admit(builder, &ordered)?;
        for id in &order {
            let Some(m) = by_id.get(id.as_str()) else {
                continue;
            };
            if let Err(es) = m.decls(&declared) {
                errors.extend(es);
            }
        }
        for c in ordered {
            if claimed
                .insert(c.name().to_string(), c.model_id().to_string())
                .is_some()
            {
                errors.push(ModelError::DeclarationCollision {
                    name: c.name().to_string(),
                    model_ref: c.model_id().to_string(),
                    existing_kind: "another model's declaration".into(),
                });
            } else if let Some(e) = check_contribution(&declared, &c) {
                errors.push(e);
            }
            staged.push(c);
        }
    }
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    // Definitions, in model-id order of their models (the order of each body is D10's).
    let mut specs = Vec::new();
    for m in by_id.values() {
        specs.extend(m.definitions());
    }
    add_definitions(builder, &specs)?;
    staged.definitions = specs.iter().map(DefinitionSpec::signature).collect();
    Ok(staged)
}

enum Resolved<'a> {
    Machine(
        Box<crate::state_machine::resolve::ResolvedStateMachine>,
        ModelSpec<'a>,
    ),
    Flow(Box<ResolvedRequestFlow>, RequestFlowSpec<'a>),
    Approval(Box<ResolvedApproval>, ApprovalSpec<'a>),
    Queue(Box<ResolvedWorkQueue>, WorkQueueSpec<'a>),
}

/// P4: lowers and composes every supplied model (lowering contract §4, extended by feature 502).
pub fn compose_models(
    builder: &mut Builder,
    models: Vec<AnyModel<'_>>,
    root: Option<&str>,
) -> Result<BuildOutput, Vec<ModelError>> {
    let base = builder
        .finish(root)
        .map_err(|r| vec![admission_failed(r)])?;

    // Resolution over the supplied set, keyed by id (one id namespace for every kind).
    let mut errors = Vec::new();
    let mut by_id: BTreeMap<String, Vec<AnyModel<'_>>> = BTreeMap::new();
    for m in models {
        by_id.entry(m.model_id().to_string()).or_default().push(m);
    }
    let mut resolved = Vec::new();
    for (id, ms) in by_id {
        if ms.len() > 1 {
            errors.push(ModelError::DuplicateModelId { model_id: id });
            continue;
        }
        for m in ms {
            match m {
                AnyModel::StateMachine(spec) => match resolve_machine(&spec.input, &base) {
                    Ok(machine) => resolved.push(Resolved::Machine(Box::new(machine), spec)),
                    Err(es) => errors.extend(es),
                },
                AnyModel::RequestFlow(spec) => {
                    // Law D4: the builder holds exactly the declaration the flow contributes.
                    match crate::request_flow::decls::decls(&spec.input, &base) {
                        Ok(schema) => {
                            if let Some(e) = check_declared(&base, &schema) {
                                errors.push(e);
                                continue;
                            }
                        }
                        Err(es) => {
                            errors.extend(es);
                            continue;
                        }
                    }
                    match resolve_flow(&spec.input, &base) {
                        Ok(flow) => resolved.push(Resolved::Flow(Box::new(flow), spec)),
                        Err(es) => errors.extend(es),
                    }
                }
                AnyModel::Approval(spec) => {
                    // Laws D4 and D4′: the builder holds exactly the declarations and the
                    // definitions the approval contributes.
                    let before = errors.len();
                    match crate::approval::resolve::decls(&spec.input, &base) {
                        Ok(cs) => errors.extend(
                            cs.iter()
                                .filter_map(|c| check_declared_contribution(&base, c)),
                        ),
                        Err(es) => errors.extend(es),
                    }
                    errors.extend(
                        crate::approval::decls::definitions(&spec.input)
                            .iter()
                            .filter_map(|d| check_definition(&base, d)),
                    );
                    if errors.len() > before {
                        continue;
                    }
                    match crate::approval::resolve::resolve(&spec.input, &base) {
                        Ok(a) => resolved.push(Resolved::Approval(Box::new(a), spec)),
                        Err(es) => errors.extend(es),
                    }
                }
                AnyModel::WorkQueue(spec) => {
                    // Laws D4 and D4′, as for approvals.
                    let before = errors.len();
                    match crate::work_queue::resolve::decls(&spec.input, &base) {
                        Ok(cs) => errors.extend(
                            cs.iter()
                                .filter_map(|c| check_declared_contribution(&base, c)),
                        ),
                        Err(es) => errors.extend(es),
                    }
                    errors.extend(
                        crate::work_queue::decls::definitions(&spec.input)
                            .iter()
                            .filter_map(|d| check_definition(&base, d)),
                    );
                    if errors.len() > before {
                        continue;
                    }
                    match crate::work_queue::resolve::resolve(&spec.input, &base) {
                        Ok(q) => resolved.push(Resolved::Queue(Box::new(q), spec)),
                        Err(es) => errors.extend(es),
                    }
                }
            }
        }
    }
    // Ownership over the internal composition set: supplied machines and derived ones.
    let ownership = ownership_table(
        resolved.iter().filter_map(|r| match r {
            Resolved::Machine(m, _) => Some(&**m),
            Resolved::Flow(f, _) => Some(&f.machine),
            Resolved::Queue(q, _) => Some(&q.machine),
            Resolved::Approval(..) => None,
        }),
        &mut errors,
    );
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    // Lowering, in canonical model-id order (`resolved` is in id order).
    let mut composed = Vec::new();
    let mut flows = Vec::new();
    let mut approvals = Vec::new();
    let mut queues = Vec::new();
    for r in resolved {
        match r {
            Resolved::Machine(machine, mut spec) => {
                let generated =
                    lower_machine(builder, &machine, spec.fragments.as_mut(), &mut errors);
                composed.push(Composed {
                    machine: *machine,
                    generated,
                    supplied: true,
                });
            }
            Resolved::Flow(flow, mut spec) => {
                for c in flow.creations.values() {
                    if let Err(es) = lower_creation(builder, &flow, c, spec.fragments.as_mut()) {
                        errors.extend(es);
                        // Core's builder cannot take an action back: once an ill-typed creation
                        // is in it, every later admission repeats that one failure.
                        if builder.finish(None).is_err() {
                            return Err(sorted_unique(errors));
                        }
                    }
                }
                let source: &mut dyn FragmentSource = spec.fragments.as_mut();
                let generated = lower_machine(builder, &flow.machine, source, &mut errors);
                composed.push(Composed {
                    machine: flow.machine.clone(),
                    generated,
                    supplied: false,
                });
                flows.push(*flow);
            }
            Resolved::Approval(approval, mut spec) => {
                for c in approval.capabilities.values() {
                    if let Err(es) = lower_decision(builder, &approval, c, spec.fragments.as_mut())
                    {
                        errors.extend(es);
                        // As for creations: an ill-typed action cannot be taken back.
                        if builder.finish(None).is_err() {
                            return Err(sorted_unique(errors));
                        }
                    }
                }
                approvals.push(*approval);
            }
            Resolved::Queue(queue, mut spec) => {
                if let Err(es) = lower_enqueue(builder, &queue, spec.fragments.as_mut()) {
                    errors.extend(es);
                    if builder.finish(None).is_err() {
                        return Err(sorted_unique(errors));
                    }
                }
                let mut source = QueueSource {
                    queue: &queue,
                    inner: spec.fragments.as_mut(),
                    errors: Vec::new(),
                };
                let mut machine_errors = Vec::new();
                let generated =
                    lower_machine(builder, &queue.machine, &mut source, &mut machine_errors);
                // A typed WorkQueue error replaces the fragment error that stopped the machine.
                errors.extend(machine_errors.into_iter().filter(|e| {
                    !matches!(e, ModelError::FragmentInvalid { core_code, .. } if core_code == QUEUE_RULE)
                }));
                errors.extend(source.errors);
                if errors.is_empty() {
                    if let Err(e) = crate::work_queue::decls::add_invariant(builder, &queue) {
                        errors.push(e);
                    }
                    if let Err(e) = crate::work_queue::decls::add_reads(builder, &queue) {
                        errors.push(e);
                    }
                }
                composed.push(Composed {
                    machine: queue.machine.clone(),
                    generated,
                    supplied: false,
                });
                queues.push(*queue);
            }
        }
    }
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    let module = builder
        .finish(root)
        .map_err(|r| vec![admission_failed(r)])?;

    // The final pass: StateMachine rules over every machine, then each kind's own rules.
    let mut errors = final_pass(&module, &composed, &ownership);
    errors.extend(crate::request_flow::ownership::check(&module, &flows));
    errors.extend(crate::approval::ownership::check(&module, &approvals));
    errors.extend(crate::work_queue::ownership::check(&module, &queues));
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    let (mut receipts, diagnostics) = crate::identity::outputs(&module, &composed, root)?;
    for f in &flows {
        receipts.push(receipt_rf(f, &module, root).map_err(|e| vec![e])?);
    }
    for a in &approvals {
        receipts
            .push(crate::approval::identity::receipt_ap(a, &module, root).map_err(|e| vec![e])?);
    }
    for q in &queues {
        receipts
            .push(crate::work_queue::identity::receipt_wq(q, &module, root).map_err(|e| vec![e])?);
    }
    receipts.sort_by_key(|r| receipt_id(&r.0));
    Ok(BuildOutput {
        module,
        receipts,
        diagnostics,
    })
}

fn receipt_id(r: &serde_json::Value) -> String {
    let m = &r["model"];
    m["model_id"]
        .as_str()
        .or_else(|| m["flow_id"].as_str())
        .or_else(|| m["approval_id"].as_str())
        .or_else(|| m["queue_id"].as_str())
        .unwrap_or_default()
        .to_string()
}

/// Lowers every transition of `machine` through the StateMachine lowerer (unchanged).
fn lower_machine(
    builder: &mut Builder,
    machine: &crate::state_machine::resolve::ResolvedStateMachine,
    source: &mut dyn FragmentSource,
    errors: &mut Vec<ModelError>,
) -> BTreeMap<String, String> {
    let mut generated = BTreeMap::new();
    for t in machine.transitions.values() {
        match lower_transition(builder, machine, t, source) {
            Ok(()) => {
                generated.insert(t.action_name.clone(), t.transition_id.to_string());
            }
            Err(es) => errors.extend(es),
        }
    }
    generated
}
