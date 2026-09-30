//! Semantic IR → SMT-LIB terms (research R3–R6).
//!
//! The encoder asserts exactly what the runtime guarantees before evaluation (types, entity
//! constraints on every incoming entity, state invariants on S, distinct state identities) and
//! models the engine's evaluation order: incoming rules, preconditions in order with
//! short-circuit `and`/`or`, effects against S, then postconditions and rules on S'. Decimal
//! arithmetic is exact, as at runtime (feature 004): `+ − ×` are terms, `÷` is a fresh quotient
//! `q` with `q·d = n` under `d ≠ 0`. What can fail is integer overflow, a fixed-scale range, and
//! division by zero; admission proves every implicit decimal store representable.

use std::collections::{BTreeMap, BTreeSet};

use behavior_core::decimal::Dec;
use behavior_core::exact::Rounding;
use behavior_core::semantic::expr::{Expr, ExprKind};
use behavior_core::semantic::module::{Module, Param, ParamRole};
use behavior_core::semantic::types::{ArithOp, CmpOp, Hash, Prim, Type, Unit, fixed_scale};
use behavior_core::semantic::value::Value;
use behavior_core::wire::Loc;

use crate::smt::{int_lit, quote, real_lit};

mod relational;
pub use relational::Slot;

const I64_MIN: &str = "(- 9223372036854775808)";
const I64_MAX: &str = "9223372036854775807";
/// Largest decimal a request can carry (28 significant digits).
const DEC_INPUT_MAX: &str = "9999999999999999999999999999.0";
/// Readable counterexamples: at most 4 fractional digits and, preferably, magnitude ≤ 10^15
/// (research R5).
const NICE_SCALE: &str = "10000.0";
const NICE_MAX: &str = "1000000000000000.0";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncodeError {
    #[error("cannot encode: {0}")]
    Unsupported(String),
}

type R<T> = Result<T, EncodeError>;

/// An encoded value: a plain term, or an option as a presence flag plus a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Plain(String),
    Opt { some: String, val: String },
}

impl Term {
    fn plain(&self) -> R<&str> {
        match self {
            Term::Plain(t) => Ok(t),
            Term::Opt { .. } => Err(EncodeError::Unsupported("option used as a value".into())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrKind {
    DivisionByZero,
    Overflow,
}

impl ErrKind {
    /// The engine's message prefix for this error (`NumError`'s display).
    pub fn message(self) -> &'static str {
        match self {
            ErrKind::DivisionByZero => "division by zero",
            ErrKind::Overflow => "numeric overflow",
        }
    }
}

/// A possible evaluation error: `cond` holds under `guard` (relative to the step start).
#[derive(Debug, Clone)]
pub struct Obligation {
    pub guard: String,
    pub cond: String,
    pub kind: ErrKind,
    pub text: String,
    pub hash: Hash,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct Encoded {
    pub term: Term,
    pub obligations: Vec<Obligation>,
}

#[derive(Debug, Clone)]
pub enum Binding {
    Entity {
        entity: String,
        fields: BTreeMap<String, Term>,
    },
    Scalar(Term),
}

pub type Env = BTreeMap<String, Binding>;

/// A declared input value, used to turn a model into an evaluation request.
#[derive(Debug, Clone)]
pub struct InputVar {
    pub section: &'static str,
    pub param: String,
    pub field: Option<String>,
    pub ty: Type,
    pub term: Term,
}

pub fn and_all(items: &[String]) -> String {
    let items: Vec<&String> = items.iter().filter(|t| t.as_str() != "true").collect();
    match items.as_slice() {
        [] => "true".into(),
        [one] => (*one).clone(),
        many => format!(
            "(and {})",
            many.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

fn or_all(items: &[String]) -> String {
    match items {
        [] => "false".into(),
        [one] => one.clone(),
        many => format!("(or {})", many.join(" ")),
    }
}

fn not(t: &str) -> String {
    format!("(not {t})")
}

fn is_decimal_sort(t: &Type) -> bool {
    match t {
        Type::Decimal => true,
        Type::Nominal(n) => n.underlying == Prim::Decimal,
        _ => false,
    }
}

fn is_int_sort(t: &Type) -> bool {
    match t {
        Type::Int => true,
        Type::Nominal(n) => n.underlying == Prim::Int,
        _ => false,
    }
}

/// An evaluation fact (feature 006, research R12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FactKind {
    /// `ex_T(x)`: `x` exists in S.
    Exists,
    /// `used_T(x)`: the store has used `x` (`ex_T(x) → used_T(x)`).
    Used,
    /// `refd_T(x)`: some entity not bound by the action holds a `Ref` to `x` in S
    /// (`refd_T(x) → ex_T(x)`).
    Refd,
}

impl FactKind {
    fn function(self, entity: &str) -> String {
        match self {
            FactKind::Exists => format!("ex!{entity}"),
            FactKind::Used => format!("used!{entity}"),
            FactKind::Refd => format!("refd!{entity}"),
        }
    }
}

/// A fact application whose value a counterexample must supply: its identity and truth value
/// symbols (read back from the model).
#[derive(Debug, Clone)]
pub struct FactVar {
    pub kind: FactKind,
    pub entity: String,
    pub id: String,
    pub value: String,
}

/// The entity universe of the encoded transition (feature 006).
#[derive(Debug, Clone, Default)]
pub struct World {
    /// State parameters and their entity types (they exist in S by binding).
    pub bound: Vec<(String, String)>,
    pub env_s: Env,
    pub env_post: Env,
    /// Created entities: type, identity term, and their bindings (label `create[i]`).
    pub creates: Vec<(String, String)>,
    /// Removed state parameters: type, identity term, parameter.
    pub removes: Vec<(String, String, String)>,
}

pub struct Encoder<'m> {
    pub module: &'m Module,
    decls: Vec<String>,
    axioms: Vec<String>,
    nice: Vec<String>,
    nice_scale: Vec<String>,
    fresh: usize,
    memo: BTreeMap<String, Encoded>,
    pub inputs: Vec<InputVar>,
    /// Entity types whose fact functions are declared.
    ufs: BTreeSet<String>,
    pub facts: Vec<FactVar>,
    fact_index: BTreeMap<(FactKind, String, String), String>,
    pub world: World,
    /// Encoding facts on S' (postconditions and outgoing rules) instead of S.
    post: bool,
    /// Unknown-member slots of each queried entity type (feature 007; witness only).
    pub slots: BTreeMap<String, Vec<Slot>>,
    /// Assertions of the witness (concretizing) query only.
    witness: Vec<String>,
    uf_declared: BTreeSet<String>,
    /// Equality-narrowed S counts and S uniqueness flags, by query and key expression.
    eq_counts: Vec<(String, String)>,
    uniq_flags: Vec<(String, String)>,
    /// Encoding a module invariant on S': the `unique` expressions it guarantees on S (its
    /// top-level conjuncts), decided by their delta rule (research R7).
    delta_unique: BTreeSet<Hash>,
}

impl<'m> Encoder<'m> {
    pub fn new(module: &'m Module) -> Self {
        Encoder {
            module,
            decls: Vec::new(),
            axioms: Vec::new(),
            nice: Vec::new(),
            nice_scale: Vec::new(),
            fresh: 0,
            memo: BTreeMap::new(),
            inputs: Vec::new(),
            ufs: BTreeSet::new(),
            facts: Vec::new(),
            fact_index: BTreeMap::new(),
            world: World::default(),
            post: false,
            slots: BTreeMap::new(),
            witness: Vec::new(),
            uf_declared: BTreeSet::new(),
            eq_counts: Vec::new(),
            uniq_flags: Vec::new(),
            delta_unique: BTreeSet::new(),
        }
    }

    /// Declares `ex_T`, `used_T`, `refd_T` once, with the facts every bound entity of `T`
    /// satisfies (it exists, so it is used).
    fn ensure_facts(&mut self, entity: &str) {
        if !self.ufs.insert(entity.to_string()) {
            return;
        }
        for k in [FactKind::Exists, FactKind::Used, FactKind::Refd] {
            self.decls.push(format!(
                "(declare-fun {} (String) Bool)",
                k.function(entity)
            ));
        }
        let bound: Vec<String> = self
            .world
            .bound
            .iter()
            .filter(|(_, e)| e == entity)
            .filter_map(|(p, _)| match self.world.env_s.get(p) {
                Some(Binding::Entity { fields, .. }) => fields
                    .get("id")
                    .and_then(|t| t.plain().ok().map(str::to_string)),
                _ => None,
            })
            .collect();
        for id in bound {
            self.axioms.push(format!("(ex!{entity} {id})"));
            self.axioms.push(format!("(used!{entity} {id})"));
        }
    }

    /// The truth value of fact `kind` of `entity` at identity term `t`, registered for the
    /// counterexample's facts section. The relations the runtime guarantees hold at `t`.
    fn fact(&mut self, kind: FactKind, entity: &str, t: &str) -> String {
        let key = (kind, entity.to_string(), t.to_string());
        if let Some(v) = self.fact_index.get(&key) {
            return v.clone();
        }
        self.ensure_facts(entity);
        self.fresh += 1;
        let (id, value) = (
            format!("fact!{}!id", self.fresh),
            format!("fact!{}", self.fresh),
        );
        self.decls.push(format!("(declare-const {id} String)"));
        self.decls.push(format!("(declare-const {value} Bool)"));
        self.axioms.push(format!("(= {id} {t})"));
        self.axioms
            .push(format!("(= {value} ({} {t}))", kind.function(entity)));
        self.axioms
            .push(format!("(=> (ex!{entity} {t}) (used!{entity} {t}))"));
        self.axioms
            .push(format!("(=> (refd!{entity} {t}) (ex!{entity} {t}))"));
        self.facts.push(FactVar {
            kind,
            entity: entity.to_string(),
            id,
            value: value.clone(),
        });
        self.fact_index.insert(key, value.clone());
        value
    }

    /// `ex_T(t)` without registering a counterexample fact (axioms about bound entities).
    fn fact_free_exists(&mut self, entity: &str, t: &str) -> String {
        self.ensure_facts(entity);
        format!("(ex!{entity} {t})")
    }

    /// `exists(t)`: `ex_T(t)` on S; `(ex_T(t) ∨ created) ∧ ¬removed` on S'.
    fn exists_term(&mut self, entity: &str, t: &str) -> String {
        let ex = self.fact(FactKind::Exists, entity, t);
        if !self.post {
            return ex;
        }
        let eq = |(e, id): (&String, &String)| (e == entity).then(|| format!("(= {t} {id})"));
        let created: Vec<String> = self
            .world
            .creates
            .iter()
            .filter_map(|(e, id)| eq((e, id)))
            .collect();
        let removed: Vec<String> = self
            .world
            .removes
            .iter()
            .filter_map(|(e, id, _)| eq((e, id)))
            .collect();
        let mut alive = vec![ex];
        alive.extend(created);
        and_all(&[or_all(&alive), not(&or_all(&removed))])
    }

    /// Reference fields of `fields` (an entity of type `source`) that point at `t`, as terms.
    fn refs_to(
        &self,
        source: &str,
        fields: &BTreeMap<String, Term>,
        target: &str,
        t: &str,
    ) -> Vec<String> {
        let Some(item) = self.module.entity(source) else {
            return Vec::new();
        };
        item.reference_fields()
            .filter(|(_, tt)| *tt == target)
            .filter_map(|(f, _)| match fields.get(f)? {
                Term::Plain(v) => Some(format!("(= {v} {t})")),
                Term::Opt { some, val } => Some(format!("(and {some} (= {val} {t}))")),
            })
            .collect()
    }

    /// `referenced(t)`: an unbound referrer (`refd_T`), or a bound (on S') or created entity's
    /// reference field equal to `t`.
    fn referenced_term(&mut self, entity: &str, t: &str) -> String {
        let mut alts = vec![self.fact(FactKind::Refd, entity, t)];
        let removed: BTreeSet<&str> = self
            .world
            .removes
            .iter()
            .map(|(_, _, p)| p.as_str())
            .collect();
        let env = if self.post {
            &self.world.env_post
        } else {
            &self.world.env_s
        };
        for (p, source) in &self.world.bound {
            if self.post && removed.contains(p.as_str()) {
                continue;
            }
            if let Some(Binding::Entity { fields, .. }) = env.get(p) {
                alts.extend(self.refs_to(source, fields, entity, t));
            }
        }
        if self.post {
            for (i, (source, _)) in self.world.creates.iter().enumerate() {
                if let Some(Binding::Entity { fields, .. }) =
                    self.world.env_post.get(&format!("create[{i}]"))
                {
                    alts.extend(self.refs_to(source, fields, entity, t));
                }
            }
        }
        or_all(&alts)
    }

    fn sort(t: &Type) -> R<&'static str> {
        Ok(match t {
            Type::Bool => "Bool",
            Type::Int | Type::Enum(_) => "Int",
            Type::Decimal => "Real",
            Type::String | Type::Id(_) => "String",
            Type::Nominal(n) => match n.underlying {
                Prim::Bool => "Bool",
                Prim::Int => "Int",
                Prim::Decimal => "Real",
                Prim::String => "String",
            },
            Type::Option(inner) => Self::sort(inner)?,
            Type::Entity(e) => {
                return Err(EncodeError::Unsupported(format!("entity {e} as value")));
            }
            Type::Exact(_) => "Real",
        })
    }

    fn fresh_int(&mut self, prefix: &str) -> String {
        self.fresh += 1;
        let name = format!("{prefix}!{}", self.fresh);
        self.decls.push(format!("(declare-const {name} Int)"));
        name
    }

    fn fresh_real(&mut self, prefix: &str) -> String {
        self.fresh += 1;
        let name = format!("{prefix}!{}", self.fresh);
        self.decls.push(format!("(declare-const {name} Real)"));
        name
    }

    /// Declares a symbol of type `t` with its domain; decimals also get a "nice" form.
    fn declare_value(&mut self, name: &str, t: &Type) -> R<Term> {
        let inner = match t {
            Type::Option(inner) => inner.as_ref(),
            other => other,
        };
        self.decls
            .push(format!("(declare-const {name} {})", Self::sort(inner)?));
        match inner {
            Type::Enum(e) => self.axioms.push(format!(
                "(and (<= 0 {name}) (<= {name} {}))",
                e.values.len().saturating_sub(1)
            )),
            t if is_int_sort(t) => self
                .axioms
                .push(format!("(and (<= {I64_MIN} {name}) (<= {name} {I64_MAX}))")),
            t if fixed_scale(t).is_some() => {
                // Exactly the runtime guarantee: on the grid 10^-s, |value| < 10^(28-s).
                let s = fixed_scale(t).and_then(|n| n.scale).unwrap_or(0);
                let g = format!("{name}!g");
                self.decls.push(format!("(declare-const {g} Int)"));
                self.axioms
                    .push(format!("(= (* {name} {}) (to_real {g}))", pow10_real(s)));
                self.axioms
                    .push(format!("(and (< (- {GRID_MAX}) {g}) (< {g} {GRID_MAX}))"));
                let k = format!("{name}!k");
                self.decls.push(format!("(declare-const {k} Int)"));
                self.nice_scale
                    .push(format!("(= (* {name} {NICE_SCALE}) (to_real {k}))"));
                self.nice.push(format!(
                    "(and (<= (- {NICE_MAX}) {name}) (<= {name} {NICE_MAX}))"
                ));
            }
            t if is_decimal_sort(t) => {
                self.axioms.push(format!(
                    "(and (<= (- {DEC_INPUT_MAX}) {name}) (<= {name} {DEC_INPUT_MAX}))"
                ));
                let k = format!("{name}!k");
                self.decls.push(format!("(declare-const {k} Int)"));
                self.nice_scale
                    .push(format!("(= (* {name} {NICE_SCALE}) (to_real {k}))"));
                self.nice.push(format!(
                    "(and (<= (- {NICE_MAX}) {name}) (<= {name} {NICE_MAX}))"
                ));
            }
            _ => {}
        }
        if matches!(t, Type::Option(_)) {
            let flag = format!("{name}!some");
            self.decls.push(format!("(declare-const {flag} Bool)"));
            Ok(Term::Opt {
                some: flag,
                val: name.to_string(),
            })
        } else {
            Ok(Term::Plain(name.to_string()))
        }
    }

    /// Declares every field of an entity bound to `param` in `section`.
    pub fn declare_param(&mut self, section: &'static str, p: &Param) -> R<Binding> {
        let base = format!("{section}.{}", p.name());
        match p.ty() {
            Type::Entity(entity) => {
                let item = self
                    .module
                    .entity(entity)
                    .ok_or_else(|| EncodeError::Unsupported(format!("unknown entity {entity}")))?;
                let mut fields = BTreeMap::new();
                for (f, ty) in item.fields() {
                    let term = self.declare_value(&format!("{base}.{f}"), ty)?;
                    self.inputs.push(InputVar {
                        section,
                        param: p.name().to_string(),
                        field: Some(f.clone()),
                        ty: ty.clone(),
                        term: term.clone(),
                    });
                    fields.insert(f.clone(), term);
                }
                Ok(Binding::Entity {
                    entity: entity.clone(),
                    fields,
                })
            }
            ty => {
                let term = self.declare_value(&base, ty)?;
                self.inputs.push(InputVar {
                    section,
                    param: p.name().to_string(),
                    field: None,
                    ty: ty.clone(),
                    term: term.clone(),
                });
                Ok(Binding::Scalar(term))
            }
        }
    }

    pub fn assert_axiom(&mut self, t: String) {
        self.axioms.push(t);
    }

    fn lit(t: &Type, v: &Value) -> R<Term> {
        let plain = |t: &Type, v: &Value| -> R<String> {
            Ok(match (t, v) {
                (_, Value::Bool(b)) => b.to_string(),
                (Type::Enum(e), Value::Str(s)) => e
                    .values
                    .iter()
                    .position(|x| x == s)
                    .map(|i| i.to_string())
                    .ok_or_else(|| EncodeError::Unsupported(format!("enum value {s}")))?,
                (_, Value::Str(s)) => quote(s),
                (_, Value::Int(i)) => int_lit(i128::from(*i)),
                (_, Value::Dec(d)) => real_lit(&d.to_normalized_string()),
                (_, other) => return Err(EncodeError::Unsupported(format!("literal {other:?}"))),
            })
        };
        match t {
            Type::Option(inner) => match v {
                Value::None => Ok(Term::Opt {
                    some: "false".into(),
                    val: Self::default_of(inner)?,
                }),
                v => Ok(Term::Opt {
                    some: "true".into(),
                    val: plain(inner, v)?,
                }),
            },
            t => Ok(Term::Plain(plain(t, v)?)),
        }
    }

    fn default_of(t: &Type) -> R<String> {
        Ok(match Self::sort(t)? {
            "Bool" => "false".into(),
            "Int" => "0".into(),
            "Real" => "0.0".into(),
            _ => "\"\"".into(),
        })
    }

    fn overflow(&self, t: &Type, r: &str) -> Option<String> {
        is_int_sort(t).then(|| format!("(or (< {r} {I64_MIN}) (> {r} {I64_MAX}))"))
    }

    fn obligation(e: &Expr, kind: ErrKind, cond: String) -> Obligation {
        Obligation {
            guard: "true".into(),
            cond,
            kind,
            text: behavior_core::pretty::text(e),
            hash: *e.hash(),
            loc: e.loc().clone(),
        }
    }

    fn guarded(obs: Vec<Obligation>, guard: &str) -> Vec<Obligation> {
        obs.into_iter()
            .map(|mut o| {
                o.guard = and_all(&[guard.to_string(), o.guard.clone()]);
                o
            })
            .collect()
    }

    fn as_real(e: &Expr, t: &str) -> String {
        if is_int_sort(e.ty()) {
            format!("(to_real {t})")
        } else {
            t.to_string()
        }
    }

    /// Encodes an expression; obligation guards are relative to the start of `e`. The same
    /// expression under the same bindings is encoded once.
    pub fn encode(&mut self, e: &Expr, env: &Env) -> R<Encoded> {
        let key = format!("{}{:?}{:?}{env:?}", self.post, self.delta_unique, e.hash());
        if let Some(hit) = self.memo.get(&key) {
            return Ok(hit.clone());
        }
        let r = self.encode_uncached(e, env)?;
        self.memo.insert(key, r.clone());
        Ok(r)
    }

    fn encode_uncached(&mut self, e: &Expr, env: &Env) -> R<Encoded> {
        let no_obs = |term: Term| {
            Ok(Encoded {
                term,
                obligations: Vec::new(),
            })
        };
        match e.kind() {
            ExprKind::Lit(v) => no_obs(Self::lit(e.ty(), v)?),
            ExprKind::Field { param, field } => match env.get(param) {
                Some(Binding::Entity { fields, .. }) => {
                    no_obs(fields.get(field).cloned().ok_or_else(|| {
                        EncodeError::Unsupported(format!("field {param}.{field}"))
                    })?)
                }
                _ => Err(EncodeError::Unsupported(format!("parameter {param}"))),
            },
            ExprKind::Param(name) => match env.get(name) {
                Some(Binding::Scalar(t)) => no_obs(t.clone()),
                _ => Err(EncodeError::Unsupported(format!("parameter {name}"))),
            },
            ExprKind::DerivedRef { name, args, .. } => {
                let d = self
                    .module
                    .derived(name)
                    .ok_or_else(|| EncodeError::Unsupported(format!("derived {name}")))?;
                let mut inner = Env::new();
                for (p, a) in d.params().iter().zip(args) {
                    let b = env
                        .get(a)
                        .cloned()
                        .ok_or_else(|| EncodeError::Unsupported(a.clone()))?;
                    inner.insert(p.name().to_string(), b);
                }
                let key = format!("{}{:?}{name}{inner:?}", self.post, self.delta_unique);
                if let Some(hit) = self.memo.get(&key) {
                    return Ok(hit.clone());
                }
                let body = d.body().clone();
                let r = self.encode(&body, &inner)?;
                self.memo.insert(key, r.clone());
                Ok(r)
            }
            ExprKind::Cmp(op, a, b) => {
                let x = self.encode(a, env)?;
                let y = self.encode(b, env)?;
                let term = match (op, &x.term, &y.term) {
                    (
                        CmpOp::Eq | CmpOp::Ne,
                        Term::Opt { some: s1, val: v1 },
                        Term::Opt { some: s2, val: v2 },
                    ) => {
                        let eq = format!("(and (= {s1} {s2}) (or (not {s1}) (= {v1} {v2})))");
                        if *op == CmpOp::Eq { eq } else { not(&eq) }
                    }
                    (CmpOp::Eq, _, _) => format!("(= {} {})", x.term.plain()?, y.term.plain()?),
                    (CmpOp::Ne, _, _) => {
                        not(&format!("(= {} {})", x.term.plain()?, y.term.plain()?))
                    }
                    (o, _, _) => {
                        let sym = match o {
                            CmpOp::Lt => "<",
                            CmpOp::Le => "<=",
                            CmpOp::Gt => ">",
                            _ => ">=",
                        };
                        format!("({sym} {} {})", x.term.plain()?, y.term.plain()?)
                    }
                };
                let mut obligations = x.obligations;
                obligations.extend(y.obligations);
                Ok(Encoded {
                    term: Term::Plain(term),
                    obligations,
                })
            }
            ExprKind::Arith(..) if matches!(e.ty(), Type::Exact(_)) => self.encode_exact(e, env),
            // Lossless fixed-scale arithmetic: exact, plus the runtime's range check (added by
            // `encode_exact`, which checks fixed-scale nodes wherever they occur).
            ExprKind::Arith(..) if fixed_scale(e.ty()).is_some() => self.encode_exact(e, env),
            // Integer arithmetic (plain or integer nominal): exact, with the runtime's 64-bit
            // overflow check. Decimal arithmetic is exact (above); `I ÷ I` is an exact ratio.
            ExprKind::Arith(op, a, b) => {
                let x = self.encode(a, env)?;
                let y = self.encode(b, env)?;
                let (xt, yt) = (x.term.plain()?.to_string(), y.term.plain()?.to_string());
                let mut obligations = x.obligations;
                obligations.extend(y.obligations);
                let sym = match op {
                    ArithOp::Add => "+",
                    ArithOp::Sub => "-",
                    ArithOp::Mul => "*",
                    ArithOp::Div => {
                        return Err(EncodeError::Unsupported("integer division".into()));
                    }
                };
                let t = format!("({sym} {xt} {yt})");
                if let Some(c) = self.overflow(e.ty(), &t) {
                    obligations.push(Self::obligation(e, ErrKind::Overflow, c));
                }
                Ok(Encoded {
                    term: Term::Plain(t),
                    obligations,
                })
            }
            ExprKind::And(xs) | ExprKind::Or(xs) => {
                let is_and = matches!(e.kind(), ExprKind::And(_));
                let mut terms = Vec::new();
                let mut obligations = Vec::new();
                let mut guard = "true".to_string();
                for x in xs {
                    let r = self.encode(x, env)?;
                    let t = r.term.plain()?.to_string();
                    obligations.extend(Self::guarded(r.obligations, &guard));
                    let step = if is_and { t.clone() } else { not(&t) };
                    guard = and_all(&[guard, step]);
                    terms.push(t);
                }
                let term = if is_and {
                    and_all(&terms)
                } else {
                    or_all(&terms)
                };
                Ok(Encoded {
                    term: Term::Plain(term),
                    obligations,
                })
            }
            ExprKind::Not(a) => {
                let r = self.encode(a, env)?;
                Ok(Encoded {
                    term: Term::Plain(not(r.term.plain()?)),
                    obligations: r.obligations,
                })
            }
            ExprKind::In(a, values) => {
                let r = self.encode(a, env)?;
                let t = r.term.plain()?.to_string();
                let mut alts = Vec::new();
                for v in values {
                    let lit = Self::lit(a.ty(), v)?;
                    alts.push(format!("(= {t} {})", lit.plain()?));
                }
                Ok(Encoded {
                    term: Term::Plain(or_all(&alts)),
                    obligations: r.obligations,
                })
            }
            ExprKind::IsNone(a) | ExprKind::IsSome(a) => {
                let r = self.encode(a, env)?;
                let Term::Opt { some, .. } = &r.term else {
                    return Err(EncodeError::Unsupported("is_none on a non-option".into()));
                };
                let term = if matches!(e.kind(), ExprKind::IsSome(_)) {
                    some.clone()
                } else {
                    not(some)
                };
                Ok(Encoded {
                    term: Term::Plain(term),
                    obligations: r.obligations,
                })
            }
            ExprKind::ValueOr(a, d) => {
                let r = self.encode(a, env)?;
                let dv = self.encode(d, env)?;
                let Term::Opt { some, val } = &r.term else {
                    return Err(EncodeError::Unsupported("value_or on a non-option".into()));
                };
                let mut obligations = r.obligations.clone();
                obligations.extend(Self::guarded(dv.obligations, &not(some)));
                Ok(Encoded {
                    term: Term::Plain(format!("(ite {some} {val} {})", dv.term.plain()?)),
                    obligations,
                })
            }
            ExprKind::Some(a) => {
                let r = self.encode(a, env)?;
                Ok(Encoded {
                    term: Term::Opt {
                        some: "true".into(),
                        val: r.term.plain()?.to_string(),
                    },
                    obligations: r.obligations,
                })
            }
            ExprKind::ToDecimal(a) => {
                let r = self.encode(a, env)?;
                Ok(Encoded {
                    term: Term::Plain(format!("(to_real {})", r.term.plain()?)),
                    obligations: r.obligations,
                })
            }
            ExprKind::Wrap(a) if fixed_scale(e.ty()).is_some() => {
                let mut r = self.encode(a, env)?;
                let t = r.term.plain()?.to_string();
                let s = fixed_scale(e.ty()).and_then(|n| n.scale).unwrap_or(0);
                r.obligations
                    .push(Self::obligation(e, ErrKind::Overflow, out_of_range(&t, s)));
                Ok(r)
            }
            ExprKind::Wrap(a) | ExprKind::Unwrap(a) => self.encode(a, env),
            ExprKind::Rescale { arg, rounding } => {
                let x = self.encode_exact(arg, env)?;
                let xt = Self::as_real(arg, x.term.plain()?);
                let s = fixed_scale(e.ty()).and_then(|n| n.scale).unwrap_or(0);
                let k = self.fresh_int("rescale");
                let y = format!("(* {xt} {})", pow10_real(s));
                self.axioms.push(rounding_constraint(*rounding, &k, &y));
                let mut obligations = x.obligations;
                obligations.push(Self::obligation(
                    e,
                    ErrKind::Overflow,
                    format!("(or (>= {k} {GRID_MAX}) (<= {k} (- {GRID_MAX})))"),
                ));
                Ok(Encoded {
                    term: Term::Plain(format!("(/ (to_real {k}) {})", pow10_real(s))),
                    obligations,
                })
            }
            ExprKind::Count(_) | ExprKind::Fold { .. } => self.encode_relational(e, env),
            ExprKind::Exists(a) | ExprKind::Referenced(a) => {
                let r = self.encode(a, env)?;
                let entity = id_entity(a.ty())
                    .ok_or_else(|| EncodeError::Unsupported("exists on a non-identity".into()))?
                    .to_string();
                let exists = matches!(e.kind(), ExprKind::Exists(_));
                let term = match (&r.term, exists) {
                    (Term::Plain(t), true) => self.exists_term(&entity, t),
                    (Term::Plain(t), false) => self.referenced_term(&entity, t),
                    (Term::Opt { some, val }, true) => {
                        let ex = self.exists_term(&entity, val);
                        format!("(and {some} {ex})")
                    }
                    (Term::Opt { .. }, false) => {
                        return Err(EncodeError::Unsupported("referenced on an option".into()));
                    }
                };
                Ok(Encoded {
                    term: Term::Plain(term),
                    obligations: r.obligations,
                })
            }
        }
    }

    /// Encodes a numeric expression exactly, as the runtime's exact domain does (research R5):
    /// non-integer arithmetic recurses without rounding or range checks (nothing is stored),
    /// conversions pass through, anything else is encoded normally.
    fn encode_exact(&mut self, e: &Expr, env: &Env) -> R<Encoded> {
        let key = format!(
            "exact:{}{:?}{:?}{env:?}",
            self.post,
            self.delta_unique,
            e.hash()
        );
        if let Some(hit) = self.memo.get(&key) {
            return Ok(hit.clone());
        }
        let r = match e.kind() {
            ExprKind::Arith(op, a, b) if !is_int_sort(e.ty()) => {
                let x = self.encode_exact(a, env)?;
                let y = self.encode_exact(b, env)?;
                let (xt, yt) = (
                    Self::as_real(a, x.term.plain()?),
                    Self::as_real(b, y.term.plain()?),
                );
                let mut obligations = x.obligations;
                obligations.extend(y.obligations);
                let term = match op {
                    ArithOp::Add => format!("(+ {xt} {yt})"),
                    ArithOp::Sub => format!("(- {xt} {yt})"),
                    ArithOp::Mul => format!("(* {xt} {yt})"),
                    ArithOp::Div => {
                        obligations.push(Self::obligation(
                            e,
                            ErrKind::DivisionByZero,
                            format!("(= {yt} 0.0)"),
                        ));
                        let q = self.fresh_real("quot");
                        self.axioms
                            .push(format!("(=> (not (= {yt} 0.0)) (= (* {q} {yt}) {xt}))"));
                        q
                    }
                };
                // A fixed-scale node is range-checked wherever it occurs, as at runtime.
                if let Some(s) = fixed_scale(e.ty()).and_then(|n| n.scale) {
                    obligations.push(Self::obligation(
                        e,
                        ErrKind::Overflow,
                        out_of_range(&term, s),
                    ));
                }
                Encoded {
                    term: Term::Plain(term),
                    obligations,
                }
            }
            ExprKind::ToDecimal(a) | ExprKind::Unwrap(a) => {
                let r = self.encode_exact(a, env)?;
                let t = Self::as_real(a, r.term.plain()?);
                Encoded {
                    term: Term::Plain(t),
                    obligations: r.obligations,
                }
            }
            ExprKind::Wrap(a) if fixed_scale(e.ty()).is_none() => self.encode_exact(a, env)?,
            _ => self.encode(e, env)?,
        };
        self.memo.insert(key, r.clone());
        Ok(r)
    }

    /// Declarations, axioms, the requested "nice value" restriction, and the given assertions.
    pub fn script(&self, assertions: &[String], nice: Nice) -> String {
        self.script_with(assertions, nice, false)
    }

    /// Whether counterexamples need a witness query (the action queries entity sets).
    pub fn has_slots(&self) -> bool {
        !self.slots.is_empty()
    }

    /// As [`Encoder::script`]; `witness` adds the definitions of every summary as a fold over the
    /// named universe (feature 007), for concretizing a counterexample. Never used for proofs.
    pub fn script_with(&self, assertions: &[String], nice: Nice, witness: bool) -> String {
        let mut s = self.decls.join("\n");
        s.push('\n');
        for a in &self.axioms {
            s.push_str(&format!("(assert {a})\n"));
        }
        for a in self.unique_links() {
            s.push_str(&format!("(assert {a})\n"));
        }
        if witness {
            for a in self.witness.iter().cloned().chain(self.witness_facts()) {
                s.push_str(&format!("(assert {a})\n"));
            }
        }
        let extra: &[&Vec<String>] = match nice {
            Nice::Raw => &[],
            Nice::Scale => &[&self.nice_scale],
            Nice::Full => &[&self.nice_scale, &self.nice],
        };
        for list in extra {
            for a in list.iter() {
                s.push_str(&format!("(assert {a})\n"));
            }
        }
        for a in assertions {
            s.push_str(&format!("(assert {a})\n"));
        }
        s
    }

    /// The symbols whose values reconstruct the inputs and the evaluation facts.
    pub fn input_symbols(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.facts {
            out.push(f.id.clone());
            out.push(f.value.clone());
        }
        let slot_terms = self.slots.values().flatten().flat_map(|s| {
            let mut terms = vec![Term::Plain(s.present.clone())];
            if let Binding::Entity { fields, .. } = &s.binding {
                terms.extend(fields.values().cloned());
            }
            terms
        });
        for t in self.inputs.iter().map(|v| v.term.clone()).chain(slot_terms) {
            match t {
                Term::Plain(t) => out.push(t),
                Term::Opt { some, val } => {
                    out.push(some);
                    out.push(val);
                }
            }
        }
        out
    }
}

/// The entity type of an `Id<T>` or `Option<Id<T>>`.
fn id_entity(t: &Type) -> Option<&str> {
    match t {
        Type::Id(e) => Some(e),
        Type::Option(inner) => id_entity(inner),
        _ => None,
    }
}

/// `10^s` as a real literal.
fn pow10_real(s: u8) -> String {
    format!("1{}.0", "0".repeat(usize::from(s)))
}

/// Bound of the integer grid index of a fixed-scale value: `|k| < 10^28`.
const GRID_MAX: &str = "10000000000000000000000000000";

/// `t` is outside the range of a fixed-scale type with scale `s` (`|t| >= 10^(28-s)`).
fn out_of_range(t: &str, s: u8) -> String {
    let limit = pow10_real(28u8.saturating_sub(s));
    format!("(or (>= {t} {limit}) (<= {t} (- {limit})))")
}

/// Linear constraints making the integer `k` the result of rounding the real `y` (research R11).
fn rounding_constraint(mode: Rounding, k: &str, y: &str) -> String {
    let kr = format!("(to_real {k})");
    let floor = format!("(and (<= {kr} {y}) (< {y} (+ {kr} 1.0)))");
    let ceiling = format!("(and (< (- {kr} 1.0) {y}) (<= {y} {kr}))");
    let nearest = format!("(and (<= (- {y} {kr}) 0.5) (<= (- {kr} {y}) 0.5))");
    let tie_up = format!("(= (- {kr} {y}) 0.5)");
    let tie_down = format!("(= (- {y} {kr}) 0.5)");
    match mode {
        Rounding::Floor => floor,
        Rounding::Ceiling => ceiling,
        Rounding::Down => format!("(ite (>= {y} 0.0) {floor} {ceiling})"),
        Rounding::Up => format!("(ite (>= {y} 0.0) {ceiling} {floor})"),
        Rounding::HalfUp => {
            format!("(and {nearest} (=> {tie_up} (> {y} 0.0)) (=> {tie_down} (< {y} 0.0)))")
        }
        Rounding::HalfEven => {
            format!("(and {nearest} (=> (or {tie_up} {tie_down}) (= (mod {k} 2) 0)))")
        }
    }
}

/// How readable a counterexample search asks the decimals to be (research R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nice {
    /// Any value in the domain (used for proofs).
    Raw,
    /// At most 4 fractional digits.
    Scale,
    /// At most 4 fractional digits and magnitude ≤ 10^15.
    Full,
}

/// What a step of the runtime path checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Constraint,
    InvariantPre,
    Precondition,
    Effect,
    Postcondition,
    InvariantPost,
    ConstraintPost,
    /// A module invariant on S' (feature 007), for invariants the action can affect.
    InvariantGlobal,
    /// A creation's identity conditions: identities distinct within the transition and never
    /// used (feature 006; the runtime refuses otherwise).
    Lifecycle,
    /// No surviving reference to a removed identity on S' (feature 006).
    Integrity,
}

/// One step of an action's runtime path (research R6).
#[derive(Debug, Clone)]
pub struct Step {
    pub kind: StepKind,
    /// Rule or condition name (rules), or `param.field` (effects), or empty.
    pub name: String,
    /// Content hash of the rule, condition expression, or effect.
    pub hash: Hash,
    /// Action parameter the rule is bound to.
    pub bound: Option<String>,
    /// Bool term that must hold for evaluation to continue (none for effects).
    pub cond: Option<String>,
    pub obligations: Vec<Obligation>,
    pub loc: Loc,
}

/// An action encoded as its runtime path.
pub struct ActionEncoding<'m> {
    pub enc: Encoder<'m>,
    pub steps: Vec<Step>,
    pub action: String,
}

fn section(role: ParamRole) -> &'static str {
    match role {
        ParamRole::State | ParamRole::Read => "state",
        ParamRole::Input => "input",
        ParamRole::Context => "context",
    }
}

impl<'m> ActionEncoding<'m> {
    pub fn build(module: &'m Module, action_name: &str) -> R<Self> {
        let action = module
            .action(action_name)
            .ok_or_else(|| EncodeError::Unsupported(format!("unknown action {action_name}")))?;
        let mut enc = Encoder::new(module);
        let mut env = Env::new();
        for p in action.params() {
            let b = enc.declare_param(section(p.role()), p)?;
            env.insert(p.name().to_string(), b);
        }
        // Distinct identities of state parameters of the same entity type (FR-027).
        let states: Vec<&Param> = action
            .params()
            .iter()
            .filter(|p| p.role() == ParamRole::State)
            .collect();
        for (i, a) in states.iter().enumerate() {
            for b in &states[i + 1..] {
                if a.ty() == b.ty()
                    && let (
                        Some(Binding::Entity { fields: fa, .. }),
                        Some(Binding::Entity { fields: fb, .. }),
                    ) = (env.get(a.name()), env.get(b.name()))
                    && let (Some(ia), Some(ib)) = (fa.get("id"), fb.get("id"))
                {
                    enc.assert_axiom(not(&format!("(= {} {})", ia.plain()?, ib.plain()?)));
                }
            }
        }
        // The universe (feature 006): state entities exist by binding, and their references
        // point at existing entities (the store keeps referential integrity in S).
        enc.world.bound = states
            .iter()
            .filter_map(|p| match p.ty() {
                Type::Entity(e) => Some((p.name().to_string(), e.clone())),
                _ => None,
            })
            .collect();
        enc.world.env_s = env.clone();
        for (param, entity) in enc.world.bound.clone() {
            let (Some(item), Some(Binding::Entity { fields, .. })) =
                (module.entity(&entity), env.get(&param))
            else {
                continue;
            };
            for (f, target) in item.reference_fields() {
                match fields.get(f) {
                    Some(Term::Plain(v)) => {
                        let ex = enc.fact_free_exists(target, v);
                        enc.assert_axiom(ex);
                    }
                    Some(Term::Opt { some, val }) => {
                        let ex = enc.fact_free_exists(target, val);
                        enc.assert_axiom(format!("(=> {some} {ex})"));
                    }
                    None => {}
                }
            }
        }

        // Module invariants hold on S (feature 007): assumed, as the runtime guarantees.
        for g in module.global_invariants().values() {
            let r = enc.encode(g.body(), &Env::new())?;
            let t = r.term.plain()?.to_string();
            enc.assert_axiom(t);
        }

        let mut steps = Vec::new();
        let rule = |enc: &mut Encoder<'m>,
                    kind: StepKind,
                    name: &str,
                    hash: Hash,
                    rule_param: &str,
                    body: &Expr,
                    bound: &str,
                    env: &Env|
         -> R<Step> {
            let mut inner = Env::new();
            if let Some(b) = env.get(bound) {
                inner.insert(rule_param.to_string(), b.clone());
            }
            let r = enc.encode(body, &inner)?;
            Ok(Step {
                kind,
                name: name.to_string(),
                hash,
                bound: Some(bound.to_string()),
                cond: Some(r.term.plain()?.to_string()),
                obligations: r.obligations,
                loc: body.loc().clone(),
            })
        };
        let entity_of = |p: &Param| match p.ty() {
            Type::Entity(e) => Some(e.clone()),
            _ => None,
        };

        // Incoming entity constraints (any role), then state invariants on S. A reference
        // constraint of a state entity holds in S (asserted above), as at runtime.
        for p in action.params() {
            let Some(entity) = entity_of(p) else { continue };
            for (name, c) in module.constraints_for(&entity) {
                if p.role() == ParamRole::State && c.reference().is_some() {
                    continue;
                }
                steps.push(rule(
                    &mut enc,
                    StepKind::Constraint,
                    name,
                    *c.hash(),
                    c.param(),
                    c.body(),
                    p.name(),
                    &env,
                )?);
            }
        }
        for p in &states {
            let Some(entity) = entity_of(p) else { continue };
            for (name, i) in module.invariants_for(&entity) {
                steps.push(rule(
                    &mut enc,
                    StepKind::InvariantPre,
                    name,
                    *i.hash(),
                    i.param(),
                    i.body(),
                    p.name(),
                    &env,
                )?);
            }
        }
        for c in action.preconditions() {
            let r = enc.encode(c.expr(), &env)?;
            steps.push(Step {
                kind: StepKind::Precondition,
                name: behavior_core::pretty::text(c.expr()),
                hash: *c.expr().hash(),
                bound: None,
                cond: Some(r.term.plain()?.to_string()),
                obligations: r.obligations,
                loc: c.loc().clone(),
            });
        }
        // Effects against S, then S'.
        let mut env_post = env.clone();
        for e in action.effects() {
            let mut r = enc.encode(e.value(), &env)?;
            // An exact value stored into a fixed-scale field: on the grid by admission, range
            // checked by the runtime.
            if let Type::Exact(Unit::Nominal(n)) = e.value().ty()
                && let Some(s) = n.scale
            {
                let t = r.term.plain()?.to_string();
                r.obligations.push(Encoder::obligation(
                    e.value(),
                    ErrKind::Overflow,
                    out_of_range(&t, s),
                ));
            }
            if let Some(Binding::Entity { fields, .. }) = env_post.get_mut(e.param()) {
                fields.insert(e.field().to_string(), r.term.clone());
            }
            steps.push(Step {
                kind: StepKind::Effect,
                name: format!("{}.{}", e.param(), e.field()),
                hash: *e.hash(),
                bound: Some(e.param().to_string()),
                cond: None,
                obligations: r.obligations,
                loc: e.loc().clone(),
            });
        }
        // Creations (feature 006): identity and complete value against S.
        let mut created: Vec<(String, String)> = Vec::new();
        for (i, c) in action.creates().iter().enumerate() {
            let id = enc.encode(c.id(), &env)?;
            let id_term = id.term.plain()?.to_string();
            let mut obligations = id.obligations;
            let item = module
                .entity(c.entity())
                .ok_or_else(|| EncodeError::Unsupported(format!("entity {}", c.entity())))?;
            let mut fields = BTreeMap::from([("id".to_string(), id.term.clone())]);
            for (f, v) in c.fields() {
                let mut r = enc.encode(v, &env)?;
                if let Type::Exact(Unit::Nominal(n)) = v.ty()
                    && let Some(s) = n.scale
                {
                    let t = r.term.plain()?.to_string();
                    r.obligations.push(Encoder::obligation(
                        v,
                        ErrKind::Overflow,
                        out_of_range(&t, s),
                    ));
                }
                obligations.extend(r.obligations);
                // A field term of the declared (possibly optional) type.
                let term = match (item.field_type(f), r.term) {
                    (Some(Type::Option(_)), Term::Plain(t)) => Term::Opt {
                        some: "true".into(),
                        val: t,
                    },
                    (_, t) => t,
                };
                fields.insert(f.clone(), term);
            }
            env_post.insert(
                format!("create[{i}]"),
                Binding::Entity {
                    entity: c.entity().to_string(),
                    fields,
                },
            );
            steps.push(Step {
                kind: StepKind::Effect,
                name: format!("create[{i}]"),
                hash: *c.hash(),
                bound: None,
                cond: None,
                obligations,
                loc: c.loc().clone(),
            });
            created.push((c.entity().to_string(), id_term));
        }
        let mut removed: Vec<(String, String, String)> = Vec::new();
        for r in action.removes() {
            if let Some(Binding::Entity { entity, fields }) = env.get(r.param())
                && let Some(id) = fields.get("id")
            {
                removed.push((
                    entity.clone(),
                    id.plain()?.to_string(),
                    r.param().to_string(),
                ));
            }
        }
        if !created.is_empty() {
            // At most one lifecycle operation per identity, and a never-used identity.
            let mut conds = Vec::new();
            for (i, (e, id)) in created.iter().enumerate() {
                for (e2, id2) in &created[i + 1..] {
                    if e == e2 {
                        conds.push(not(&format!("(= {id} {id2})")));
                    }
                }
                for (e2, id2, _) in &removed {
                    if e == e2 {
                        conds.push(not(&format!("(= {id} {id2})")));
                    }
                }
            }
            for (e, id) in &created {
                let used = enc.fact(FactKind::Used, e, id);
                conds.push(not(&used));
            }
            steps.push(Step {
                kind: StepKind::Lifecycle,
                name: "identities".into(),
                hash: *action.hash(),
                bound: None,
                cond: Some(and_all(&conds)),
                obligations: Vec::new(),
                loc: action.loc().clone(),
            });
        }
        enc.world.creates = created;
        enc.world.removes = removed.clone();
        enc.world.env_post = env_post.clone();
        enc.post = true;

        for c in action.postconditions() {
            let r = enc.encode(c.expr(), &env_post)?;
            steps.push(Step {
                kind: StepKind::Postcondition,
                name: behavior_core::pretty::text(c.expr()),
                hash: *c.expr().hash(),
                bound: None,
                cond: Some(r.term.plain()?.to_string()),
                obligations: r.obligations,
                loc: c.loc().clone(),
            });
        }
        // Outgoing rules: surviving state entities, then created entities (runtime order). A
        // survivor's reference constraint is checked only if an effect assigns its field.
        let removed_params: BTreeSet<&str> = removed.iter().map(|(_, _, p)| p.as_str()).collect();
        let survivors: Vec<&&Param> = states
            .iter()
            .filter(|p| !removed_params.contains(p.name()))
            .collect();
        let creates = action.creates();
        for p in &survivors {
            let Some(entity) = entity_of(p) else { continue };
            for (name, i) in module.invariants_for(&entity) {
                steps.push(rule(
                    &mut enc,
                    StepKind::InvariantPost,
                    name,
                    *i.hash(),
                    i.param(),
                    i.body(),
                    p.name(),
                    &env_post,
                )?);
            }
        }
        for (k, c) in creates.iter().enumerate() {
            for (name, i) in module.invariants_for(c.entity()) {
                steps.push(rule(
                    &mut enc,
                    StepKind::InvariantPost,
                    name,
                    *i.hash(),
                    i.param(),
                    i.body(),
                    &format!("create[{k}]"),
                    &env_post,
                )?);
            }
        }
        for p in &survivors {
            let Some(entity) = entity_of(p) else { continue };
            for (name, c) in module.constraints_for(&entity) {
                if let Some(f) = c.reference()
                    && !action
                        .effects()
                        .iter()
                        .any(|e| e.param() == p.name() && e.field() == f)
                {
                    continue;
                }
                steps.push(rule(
                    &mut enc,
                    StepKind::ConstraintPost,
                    name,
                    *c.hash(),
                    c.param(),
                    c.body(),
                    p.name(),
                    &env_post,
                )?);
            }
        }
        for (k, cr) in creates.iter().enumerate() {
            for (name, c) in module.constraints_for(cr.entity()) {
                steps.push(rule(
                    &mut enc,
                    StepKind::ConstraintPost,
                    name,
                    *c.hash(),
                    c.param(),
                    c.body(),
                    &format!("create[{k}]"),
                    &env_post,
                )?);
            }
        }
        // Module invariants on S' that the action can affect (feature 007), `unique` by its
        // delta rule.
        for (name, g) in module.global_invariants() {
            if !behavior_core::affects(module, action, g.signature()) {
                continue;
            }
            enc.delta_unique = behavior_core::held_uniques(g.body());
            let r = enc.encode(g.body(), &Env::new());
            enc.delta_unique.clear();
            let r = r?;
            steps.push(Step {
                kind: StepKind::InvariantGlobal,
                name: name.clone(),
                hash: *g.hash(),
                bound: None,
                cond: Some(r.term.plain()?.to_string()),
                obligations: r.obligations,
                loc: g.loc().clone(),
            });
        }
        // Referential integrity on S' (research R6, R12).
        for (r, (entity, id, param)) in action.removes().iter().zip(&removed) {
            if module.references_to(entity).is_empty() {
                continue;
            }
            let violated = enc.referenced_term(entity, id);
            steps.push(Step {
                kind: StepKind::Integrity,
                name: format!("remove {param}"),
                hash: *r.hash(),
                bound: Some(param.clone()),
                cond: Some(not(&violated)),
                obligations: Vec::new(),
                loc: r.loc().clone(),
            });
        }
        Ok(ActionEncoding {
            enc,
            steps,
            action: action_name.to_string(),
        })
    }

    /// No evaluation error occurs in step `k`.
    pub fn noerr(&self, k: usize) -> String {
        let parts: Vec<String> = self.steps[k]
            .obligations
            .iter()
            .map(|o| not(&and_all(&[o.guard.clone(), o.cond.clone()])))
            .collect();
        and_all(&parts)
    }

    /// Evaluation reaches step `k`: every earlier check held and no earlier error occurred.
    pub fn path(&self, k: usize) -> String {
        let mut parts = Vec::new();
        for i in 0..k {
            if let Some(c) = &self.steps[i].cond {
                parts.push(c.clone());
            }
            parts.push(self.noerr(i));
        }
        and_all(&parts)
    }

    /// Index of the first step of the given kind at or after the preconditions end.
    pub fn after_preconditions(&self) -> usize {
        self.steps
            .iter()
            .position(|s| {
                !matches!(
                    s.kind,
                    StepKind::Constraint | StepKind::InvariantPre | StepKind::Precondition
                )
            })
            .unwrap_or(self.steps.len())
    }
}

/// A rule (Bool derived value) encoded over valid instances of its parameters.
pub struct RuleEncoding<'m> {
    pub enc: Encoder<'m>,
    pub cond: String,
    pub assumptions: Vec<String>,
    pub noerr: String,
}

impl<'m> RuleEncoding<'m> {
    pub fn build(module: &'m Module, name: &str) -> R<Self> {
        let d = module
            .derived(name)
            .ok_or_else(|| EncodeError::Unsupported(format!("rule {name}")))?;
        let mut enc = Encoder::new(module);
        let mut env = Env::new();
        let mut assumptions = Vec::new();
        for p in d.params() {
            let b = enc.declare_param("state", p)?;
            env.insert(p.name().to_string(), b.clone());
            if let Type::Entity(entity) = p.ty() {
                for (_, c) in module.constraints_for(entity) {
                    let mut inner = Env::new();
                    inner.insert(c.param().to_string(), b.clone());
                    let r = enc.encode(c.body(), &inner)?;
                    assumptions.push(r.term.plain()?.to_string());
                    for o in r.obligations {
                        assumptions.push(not(&and_all(&[o.guard, o.cond])));
                    }
                }
            }
        }
        let body = d.body().clone();
        let r = enc.encode(&body, &env)?;
        let noerr = and_all(
            &r.obligations
                .iter()
                .map(|o| not(&and_all(&[o.guard.clone(), o.cond.clone()])))
                .collect::<Vec<_>>(),
        );
        Ok(RuleEncoding {
            cond: r.term.plain()?.to_string(),
            enc,
            assumptions,
            noerr,
        })
    }
}

/// Turns a model value into an engine value of type `t` (decimals must be representable).
pub fn model_value(t: &Type, v: &crate::smt::SmtValue) -> Option<Value> {
    use crate::smt::SmtValue as S;
    match (t, v) {
        (Type::Option(inner), v) => model_value(inner, v),
        (Type::Enum(e), S::Int(i)) => usize::try_from(*i)
            .ok()
            .and_then(|i| e.values.get(i))
            .map(|s| Value::Str(s.clone())),
        (Type::Bool, S::Bool(b)) => Some(Value::Bool(*b)),
        (Type::Nominal(n), v) => model_value(&n.underlying.to_type(), v),
        (Type::Int, S::Int(i)) => i64::try_from(*i).ok().map(Value::Int),
        (Type::Decimal, v) => v
            .to_decimal_string()
            .and_then(|s| Dec::parse_str(&s).ok())
            .map(Value::Dec),
        (Type::String | Type::Id(_), S::Str(s)) => Some(Value::Str(s.clone())),
        _ => None,
    }
}
