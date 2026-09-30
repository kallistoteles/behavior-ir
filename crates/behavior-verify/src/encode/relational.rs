//! Relational queries (feature 007, research R10): symbolic summaries over S, exact deltas for S'.
//!
//! A summary (the count, a sum, an extremum or the uniqueness of a query instance on S) is an
//! uninterpreted function of the instance's captured values: equal captures give the same
//! instance and so the same summary, a changed capture an unrelated one. The action's bound
//! entities are known candidates: every summary is constrained to account for them. On S' a
//! summary is the S summary of the S' instance plus the exact effect of every created, removed
//! or changed bound candidate; an extremum after a removal or change is fresh.
//!
//! Quantifiers are normalized onto counts of narrowed instances
//! (`any(q, p) ≡ count(where(q, p)) > 0`, `all(q, p) ≡ count(where(q, not p)) == 0`), and an
//! equality narrowing `key == v` is keyed by the key expression with `v` as an argument, so
//! `not any(q, e → e.k == v)` and the `unique` delta rule share one summary.
//!
//! Counterexamples: every queried type has a few explicit unknown-member slots. Only the witness
//! query defines the summaries as folds over the known candidates and the present slots; proofs
//! never see those definitions, so slots are witnesses only, never a finite-universe bound.

use std::collections::BTreeMap;

use behavior_core::semantic::expr::{
    CANDIDATE, Expr, ExprKind, FoldOp, QueryKind, QueryNode, SetOp,
};
use behavior_core::semantic::types::{Hash, Type, fixed_scale, hash_display};

use super::{
    Binding, EncodeError, Encoded, Encoder, Env, ErrKind, FactKind, Obligation, R, Term, and_all,
    not, or_all, out_of_range,
};

/// Summary arguments: captured value terms and their sorts.
type Args = Vec<(String, &'static str)>;

/// Unknown-member slots per queried entity type.
const SLOTS: usize = 3;

/// An unknown member of a queried type: a presence flag and a symbolic value.
#[derive(Debug, Clone)]
pub struct Slot {
    pub present: String,
    pub binding: Binding,
}

/// How an instance narrows its query.
#[derive(Clone)]
enum Narrow<'e> {
    None,
    Pred(&'e Expr),
    NotPred(&'e Expr),
    /// `key(candidate) == value`, `value` a term of the enclosing state.
    Eq {
        key: &'e Expr,
        value: Term,
    },
}

#[derive(Clone)]
struct Inst<'e> {
    q: &'e QueryNode,
    narrow: Narrow<'e>,
}

fn short(h: &Hash) -> String {
    let d = hash_display(h);
    d.rsplit(':')
        .next()
        .unwrap_or(&d)
        .chars()
        .take(16)
        .collect()
}

impl Inst<'_> {
    fn key(&self) -> String {
        let q = short(self.q.hash());
        match &self.narrow {
            Narrow::None => q,
            Narrow::Pred(p) => format!("{q}!p{}", short(p.hash())),
            Narrow::NotPred(p) => format!("{q}!n{}", short(p.hash())),
            Narrow::Eq { key, .. } => format!("{q}!k{}", short(key.hash())),
        }
    }
}

/// Whether `e` reads the candidate.
fn reads_candidate(e: &Expr) -> bool {
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        match x.kind() {
            ExprKind::Field { param, .. } if param == CANDIDATE => return true,
            ExprKind::Param(p) if p == CANDIDATE => return true,
            ExprKind::DerivedRef { args, .. } if args.iter().any(|a| a == CANDIDATE) => {
                return true;
            }
            _ => {}
        }
        stack.extend(x.children());
    }
    false
}

/// The captured reads of `exprs` (fields of bound parameters and parameters, never the
/// candidate), in first-appearance order. A derived value applied to anything but the candidate
/// would hide reads: not encoded.
fn capture_exprs<'e>(exprs: &[&'e Expr]) -> R<Vec<&'e Expr>> {
    let mut out: Vec<&Expr> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut stack: Vec<&Expr> = exprs.iter().rev().copied().collect();
    while let Some(e) = stack.pop() {
        match e.kind() {
            ExprKind::Field { param, field } if param != CANDIDATE => {
                let k = format!("{param}.{field}");
                if !seen.contains(&k) {
                    seen.push(k);
                    out.push(e);
                }
            }
            ExprKind::Param(name) if name != CANDIDATE => {
                if !seen.contains(name) {
                    seen.push(name.clone());
                    out.push(e);
                }
            }
            ExprKind::DerivedRef { args, name, .. } if args.iter().any(|a| a != CANDIDATE) => {
                return Err(EncodeError::Unsupported(format!(
                    "derived value {name} of a non-candidate inside a query"
                )));
            }
            _ => {
                let mut children = e.children();
                children.reverse();
                stack.extend(children);
            }
        }
    }
    Ok(out)
}

fn ite01(m: &str, one: &str, zero: &str) -> String {
    format!("(ite {m} {one} {zero})")
}

fn add_all(items: &[String], zero: &str) -> String {
    match items {
        [] => zero.to_string(),
        [one] => one.clone(),
        many => format!("(+ {})", many.join(" ")),
    }
}

/// Equality of two terms of the same type (options compare presence, then value).
fn eq_terms(a: &Term, b: &Term) -> R<String> {
    Ok(match (a, b) {
        (Term::Opt { some: s1, val: v1 }, Term::Opt { some: s2, val: v2 }) => {
            format!("(and (= {s1} {s2}) (or (not {s1}) (= {v1} {v2})))")
        }
        _ => format!("(= {} {})", a.plain()?, b.plain()?),
    })
}

/// A candidate for a summary: its binding and whether it is a member only under a condition
/// (a slot's presence).
struct Cand {
    binding: Binding,
    guard: Option<String>,
}

impl<'m> Encoder<'m> {
    /// Declares the unknown-member slots of `entity` once, with what the store guarantees of any
    /// existing entity (witness-only: they constrain nothing a proof uses).
    fn ensure_slots(&mut self, entity: &str) -> R<()> {
        if self.slots.contains_key(entity) {
            return Ok(());
        }
        self.ensure_facts(entity);
        let item = self
            .module
            .entity(entity)
            .ok_or_else(|| EncodeError::Unsupported(format!("unknown entity {entity}")))?;
        let fields_decl: Vec<(String, Type)> = item
            .fields()
            .iter()
            .map(|(f, t)| (f.clone(), t.clone()))
            .collect();
        let known = self.known_ids(entity);
        let mut slots: Vec<Slot> = Vec::new();
        let saved_post = self.post;
        self.post = false;
        for i in 0..SLOTS {
            let base = format!("slot!{entity}!{i}");
            let present = format!("{base}!present");
            self.decls.push(format!("(declare-const {present} Bool)"));
            let mut fields = BTreeMap::new();
            for (f, ty) in &fields_decl {
                let term = self.declare_value(&format!("{base}.{f}"), ty)?;
                fields.insert(f.clone(), term);
            }
            let id = fields
                .get("id")
                .and_then(|t| t.plain().ok().map(str::to_string))
                .ok_or_else(|| EncodeError::Unsupported(format!("{entity} without id")))?;
            let mut distinct: Vec<String> = known
                .iter()
                .map(|k| not(&format!("(= {id} {k})")))
                .collect();
            for s in &slots {
                if let Binding::Entity { fields: other, .. } = &s.binding
                    && let Some(Ok(oid)) = other.get("id").map(Term::plain)
                {
                    distinct.push(format!(
                        "(=> {} {})",
                        s.present,
                        not(&format!("(= {id} {oid})"))
                    ));
                }
            }
            self.witness
                .push(format!("(=> {present} {})", and_all(&distinct)));
            self.witness
                .push(format!("(=> {present} (ex!{entity} {id}))"));
            self.witness
                .push(format!("(=> {present} (used!{entity} {id}))"));
            let binding = Binding::Entity {
                entity: entity.to_string(),
                fields,
            };
            // A stored entity satisfies its constraints and invariants.
            let rules: Vec<(String, Expr)> = self
                .module
                .constraints_for(entity)
                .map(|(_, c)| (c.param().to_string(), c.body().clone()))
                .chain(
                    self.module
                        .invariants_for(entity)
                        .map(|(_, i)| (i.param().to_string(), i.body().clone())),
                )
                .collect();
            for (param, body) in rules {
                let mut env = Env::new();
                env.insert(param, binding.clone());
                let r = self.encode(&body, &env)?;
                self.witness
                    .push(format!("(=> {present} {})", r.term.plain()?));
            }
            slots.push(Slot { present, binding });
        }
        self.post = saved_post;
        self.slots.insert(entity.to_string(), slots);
        Ok(())
    }

    /// Identity terms of the bound entities of `entity` (they exist in S).
    fn known_ids(&self, entity: &str) -> Vec<String> {
        self.known_s(entity)
            .into_iter()
            .filter_map(|b| match b {
                Binding::Entity { fields, .. } => fields
                    .get("id")
                    .and_then(|t| t.plain().ok().map(str::to_string)),
                Binding::Scalar(_) => None,
            })
            .collect()
    }

    /// The S values of the bound entities of `entity`.
    fn known_s(&self, entity: &str) -> Vec<Binding> {
        self.world
            .bound
            .iter()
            .filter(|(_, e)| e == entity)
            .filter_map(|(p, _)| self.world.env_s.get(p).cloned())
            .collect()
    }

    /// Bound entities of `entity` the transition removes or changes: `(S value, S' value or None
    /// if removed)`.
    fn touched(&self, entity: &str) -> Vec<(Binding, Option<Binding>)> {
        let mut out = Vec::new();
        for (p, e) in &self.world.bound {
            if e != entity {
                continue;
            }
            let (Some(s), Some(post)) = (self.world.env_s.get(p), self.world.env_post.get(p))
            else {
                continue;
            };
            let removed = self.world.removes.iter().any(|(_, _, rp)| rp == p);
            if removed {
                out.push((s.clone(), None));
            } else if format!("{s:?}") != format!("{post:?}") {
                out.push((s.clone(), Some(post.clone())));
            }
        }
        out
    }

    /// Entities of `entity` the transition creates, with their values.
    fn created(&self, entity: &str) -> Vec<Binding> {
        self.world
            .creates
            .iter()
            .enumerate()
            .filter(|(_, (e, _))| e == entity)
            .filter_map(|(i, _)| self.world.env_post.get(&format!("create[{i}]")).cloned())
            .collect()
    }

    fn slot_cands(&self, entity: &str) -> Vec<Cand> {
        self.slots
            .get(entity)
            .map(|v| {
                v.iter()
                    .map(|s| Cand {
                        binding: s.binding.clone(),
                        guard: Some(s.present.clone()),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Applies (declaring once) the uninterpreted function `name` to `args`.
    fn uf_app(&mut self, name: &str, args: &[(String, &'static str)], ret: &str) -> String {
        if self.uf_declared.insert(name.to_string()) {
            let sorts: Vec<&str> = args.iter().map(|(_, s)| *s).collect();
            self.decls
                .push(format!("(declare-fun {name} ({}) {ret})", sorts.join(" ")));
        }
        if args.is_empty() {
            name.to_string()
        } else {
            let terms: Vec<&str> = args.iter().map(|(t, _)| t.as_str()).collect();
            format!("({name} {})", terms.join(" "))
        }
    }

    fn flatten(&self, t: &Term, ty: &Type) -> R<Vec<(String, &'static str)>> {
        let sort = Self::sort(ty)?;
        Ok(match t {
            Term::Plain(x) => vec![(x.clone(), sort)],
            Term::Opt { some, val } => vec![(some.clone(), "Bool"), (val.clone(), sort)],
        })
    }

    /// The instance's captured values in the enclosing environment (the summary's arguments).
    fn inst_args(&mut self, inst: &Inst<'_>, env: &Env) -> R<(Args, Vec<Obligation>)> {
        let mut exprs = inst.q.bodies();
        match &inst.narrow {
            Narrow::Pred(p) | Narrow::NotPred(p) => exprs.push(p),
            Narrow::Eq { key, .. } => exprs.push(key),
            Narrow::None => {}
        }
        let mut args = Vec::new();
        let mut obligations = Vec::new();
        for c in capture_exprs(&exprs)? {
            let r = self.encode(c, env)?;
            args.extend(self.flatten(&r.term, c.ty())?);
            obligations.extend(r.obligations);
        }
        if let Narrow::Eq { key, value } = &inst.narrow {
            args.extend(self.flatten(value, key.ty())?);
        }
        Ok((args, obligations))
    }

    fn member_q(&mut self, q: &QueryNode, env: &Env, obs: &mut Vec<Obligation>) -> R<String> {
        Ok(match q.kind() {
            QueryKind::Select => "true".into(),
            QueryKind::Where { base, body, .. } => {
                let b = self.member_q(base, env, obs)?;
                let r = self.encode(body, env)?;
                obs.extend(r.obligations);
                and_all(&[b, r.term.plain()?.to_string()])
            }
            QueryKind::Set { op, a, b } => {
                let x = self.member_q(a, env, obs)?;
                let y = self.member_q(b, env, obs)?;
                match op {
                    SetOp::Union => or_all(&[x, y]),
                    SetOp::Intersection => and_all(&[x, y]),
                    SetOp::Difference => and_all(&[x, not(&y)]),
                }
            }
        })
    }

    /// Membership of a candidate in an instance, and the obligations of evaluating it.
    fn member(
        &mut self,
        inst: &Inst<'_>,
        cand: &Cand,
        env: &Env,
        obs: &mut Vec<Obligation>,
    ) -> R<String> {
        let mut env2 = env.clone();
        env2.insert(CANDIDATE.to_string(), cand.binding.clone());
        let m = self.member_q(inst.q, &env2, obs)?;
        let n = match &inst.narrow {
            Narrow::None => "true".to_string(),
            Narrow::Pred(p) | Narrow::NotPred(p) => {
                let r = self.encode(p, &env2)?;
                obs.extend(r.obligations);
                let t = r.term.plain()?.to_string();
                if matches!(inst.narrow, Narrow::NotPred(_)) {
                    not(&t)
                } else {
                    t
                }
            }
            Narrow::Eq { key, value } => {
                let r = self.encode(key, &env2)?;
                obs.extend(r.obligations);
                eq_terms(&r.term, value)?
            }
        };
        let mut parts = Vec::new();
        if let Some(g) = &cand.guard {
            parts.push(g.clone());
        }
        parts.push(m);
        parts.push(n);
        Ok(and_all(&parts))
    }

    /// The value of `body` for a candidate.
    fn value_of(
        &mut self,
        body: &Expr,
        cand: &Cand,
        env: &Env,
        obs: &mut Vec<Obligation>,
    ) -> R<String> {
        let mut env2 = env.clone();
        env2.insert(CANDIDATE.to_string(), cand.binding.clone());
        let r = self.encode(body, &env2)?;
        obs.extend(r.obligations);
        Ok(r.term.plain()?.to_string())
    }

    /// Candidates of the S' instance and the exact change: `(sign, candidate)` pairs, `-1` for a
    /// touched bound entity's S value, `+1` for its S' value and for every creation.
    fn deltas(&self, entity: &str) -> Vec<(bool, Cand)> {
        let mut out = Vec::new();
        for (s, post) in self.touched(entity) {
            out.push((
                false,
                Cand {
                    binding: s,
                    guard: None,
                },
            ));
            if let Some(p) = post {
                out.push((
                    true,
                    Cand {
                        binding: p,
                        guard: None,
                    },
                ));
            }
        }
        for c in self.created(entity) {
            out.push((
                true,
                Cand {
                    binding: c,
                    guard: None,
                },
            ));
        }
        out
    }

    /// The members of the instance on the current state that the encoder can name: known
    /// candidates (S values, or surviving S' values and creations on S') and slots.
    fn named_members(&self, entity: &str) -> Vec<Cand> {
        let mut out: Vec<Cand> = Vec::new();
        if self.post {
            let touched = self.touched(entity);
            for b in self.known_s(entity) {
                let key = format!("{b:?}");
                match touched.iter().find(|(s, _)| format!("{s:?}") == key) {
                    Some((_, Some(p))) => out.push(Cand {
                        binding: p.clone(),
                        guard: None,
                    }),
                    Some((_, None)) => {}
                    None => out.push(Cand {
                        binding: b,
                        guard: None,
                    }),
                }
            }
            out.extend(self.created(entity).into_iter().map(|b| Cand {
                binding: b,
                guard: None,
            }));
        } else {
            out.extend(self.known_s(entity).into_iter().map(|b| Cand {
                binding: b,
                guard: None,
            }));
        }
        out.extend(self.slot_cands(entity));
        out
    }

    /// Turns the obligations of evaluating candidates into flags: which candidate is visited
    /// first is unknown, so an error may or may not happen; a flag that is free keeps both
    /// proofs (path) and error checks (the flag can hold) sound.
    fn error_flags(&mut self, obs: Vec<Obligation>) -> Vec<Obligation> {
        let mut seen: Vec<(ErrKind, String)> = Vec::new();
        let mut out = Vec::new();
        for o in obs {
            if seen.contains(&(o.kind, o.text.clone())) {
                continue;
            }
            seen.push((o.kind, o.text.clone()));
            self.fresh += 1;
            let flag = format!("qerr!{}", self.fresh);
            self.decls.push(format!("(declare-const {flag} Bool)"));
            out.push(Obligation {
                guard: "true".into(),
                cond: flag,
                ..o
            });
        }
        out
    }

    /// The S summary count of an instance: its application and the known-candidate lower bound.
    fn s_count(
        &mut self,
        inst: &Inst<'_>,
        args: &[(String, &'static str)],
        env: &Env,
        obs: &mut Vec<Obligation>,
    ) -> R<(String, String)> {
        let entity = inst.q.entity().to_string();
        let app = self.uf_app(&format!("count!{}", inst.key()), args, "Int");
        let mut known = Vec::new();
        for b in self.known_s(&entity) {
            let m = self.member(
                inst,
                &Cand {
                    binding: b,
                    guard: None,
                },
                env,
                obs,
            )?;
            known.push(ite01(&m, "1", "0"));
        }
        let known_count = add_all(&known, "0");
        self.axioms.push(format!("(>= {app} {known_count})"));
        let mut all = known;
        for c in self.slot_cands(&entity) {
            let m = self.member(inst, &c, env, obs)?;
            all.push(ite01(&m, "1", "0"));
        }
        self.witness
            .push(format!("(= {app} {})", add_all(&all, "0")));
        if let Narrow::Eq { key, .. } = &inst.narrow {
            self.eq_counts.push((
                format!("{}!{}", short(inst.q.hash()), short(key.hash())),
                app.clone(),
            ));
        }
        Ok((app, known_count))
    }

    /// The count of an instance on the current state.
    fn count_of(&mut self, inst: &Inst<'_>, env: &Env, obs: &mut Vec<Obligation>) -> R<String> {
        self.ensure_slots(inst.q.entity())?;
        let (args, cap_obs) = self.inst_args(inst, env)?;
        obs.extend(cap_obs);
        let (app, _) = self.s_count(inst, &args, env, obs)?;
        if !self.post {
            return Ok(app);
        }
        let mut terms = vec![app];
        for (plus, c) in self.deltas(inst.q.entity()) {
            let m = self.member(inst, &c, env, obs)?;
            terms.push(if plus {
                ite01(&m, "1", "0")
            } else {
                ite01(&m, "(- 1)", "0")
            });
        }
        Ok(add_all(&terms, "0"))
    }

    fn zero(ty: &Type) -> R<&'static str> {
        Ok(if Self::sort(ty)? == "Int" { "0" } else { "0.0" })
    }

    fn sum_of(
        &mut self,
        q: &QueryNode,
        body: &Expr,
        env: &Env,
        obs: &mut Vec<Obligation>,
    ) -> R<String> {
        let entity = q.entity().to_string();
        self.ensure_slots(&entity)?;
        let inst = Inst {
            q,
            narrow: Narrow::None,
        };
        let (args, cap_obs) = self.inst_args(&inst, env)?;
        obs.extend(cap_obs);
        let (count, known_count) = self.s_count(&inst, &args, env, obs)?;
        let sort = Self::sort(body.ty())?;
        let zero = Self::zero(body.ty())?;
        let app = self.uf_app(
            &format!("sum!{}!{}", inst.key(), short(body.hash())),
            &args,
            sort,
        );
        let mut known = Vec::new();
        for b in self.known_s(&entity) {
            let c = Cand {
                binding: b,
                guard: None,
            };
            let m = self.member(&inst, &c, env, obs)?;
            let v = self.value_of(body, &c, env, obs)?;
            known.push(ite01(&m, &v, zero));
        }
        // No unknown member: the sum is the known candidates' (the empty sum is the exact zero).
        self.axioms.push(format!(
            "(=> (= {count} {known_count}) (= {app} {}))",
            add_all(&known, zero)
        ));
        let mut all = known;
        for c in self.slot_cands(&entity) {
            let m = self.member(&inst, &c, env, obs)?;
            let v = self.value_of(body, &c, env, obs)?;
            all.push(ite01(&m, &v, zero));
        }
        self.witness
            .push(format!("(= {app} {})", add_all(&all, zero)));
        if !self.post {
            return Ok(app);
        }
        let mut terms = vec![app];
        for (plus, c) in self.deltas(&entity) {
            let m = self.member(&inst, &c, env, obs)?;
            let v = self.value_of(body, &c, env, obs)?;
            terms.push(if plus {
                ite01(&m, &v, zero)
            } else {
                ite01(&m, &format!("(- {v})"), zero)
            });
        }
        Ok(add_all(&terms, zero))
    }

    /// Exact extremum over named candidates (witness only): `some ⇔ ∃ member`, `val` bounds every
    /// member and equals one.
    fn define_extremum(&mut self, some: &str, val: &str, members: &[(String, String)], min: bool) {
        let op = if min { "<=" } else { ">=" };
        let ms: Vec<String> = members.iter().map(|(m, _)| m.clone()).collect();
        self.witness.push(format!("(= {some} {})", or_all(&ms)));
        for (m, v) in members {
            self.witness.push(format!("(=> {m} ({op} {val} {v}))"));
        }
        let hits: Vec<String> = members
            .iter()
            .map(|(m, v)| format!("(and {m} (= {val} {v}))"))
            .collect();
        self.witness.push(format!("(=> {some} {})", or_all(&hits)));
    }

    fn extremum_of(
        &mut self,
        q: &QueryNode,
        body: &Expr,
        min: bool,
        env: &Env,
        obs: &mut Vec<Obligation>,
    ) -> R<Term> {
        let entity = q.entity().to_string();
        self.ensure_slots(&entity)?;
        let inst = Inst {
            q,
            narrow: Narrow::None,
        };
        let (args, cap_obs) = self.inst_args(&inst, env)?;
        obs.extend(cap_obs);
        let (count, _) = self.s_count(&inst, &args, env, obs)?;
        let sort = Self::sort(body.ty())?;
        let tag = if min { "min" } else { "max" };
        let name = format!("{tag}!{}!{}", inst.key(), short(body.hash()));
        let some = self.uf_app(&format!("{name}!some"), &args, "Bool");
        let val = self.uf_app(&format!("{name}!val"), &args, sort);
        let op = if min { "<=" } else { ">=" };
        self.axioms.push(format!("(= {some} (> {count} 0))"));
        // Members named on S (known candidates, then slots) for the S summary.
        let saved = self.post;
        self.post = false;
        let mut s_members = Vec::new();
        for c in self.named_members(&entity) {
            let known = c.guard.is_none();
            let m = self.member(&inst, &c, env, obs)?;
            let v = self.value_of(body, &c, env, obs)?;
            if known {
                self.axioms.push(format!("(=> {m} ({op} {val} {v}))"));
            }
            s_members.push((m, v));
        }
        self.post = saved;
        self.define_extremum(&some, &val, &s_members, min);
        if !self.post {
            return Ok(Term::Opt { some, val });
        }
        if self.touched(&entity).is_empty() {
            // Only creations: the extremum composes exactly.
            let (mut cs, mut cv) = (some, val);
            for b in self.created(&entity) {
                let c = Cand {
                    binding: b,
                    guard: None,
                };
                let m = self.member(&inst, &c, env, obs)?;
                let v = self.value_of(body, &c, env, obs)?;
                let better = if min {
                    format!("(< {v} {cv})")
                } else {
                    format!("(> {v} {cv})")
                };
                cv = format!("(ite (and {m} (or (not {cs}) {better})) {v} {cv})");
                cs = or_all(&[cs, m]);
            }
            return Ok(Term::Opt { some: cs, val: cv });
        }
        // A removal or change: a fresh extremum, present exactly when the S' count is positive.
        let count_post = self.count_of(&inst, env, obs)?;
        self.fresh += 1;
        let (fs, fv) = (
            format!("{tag}!post!{}!some", self.fresh),
            format!("{tag}!post!{}", self.fresh),
        );
        self.decls.push(format!("(declare-const {fs} Bool)"));
        self.decls.push(format!("(declare-const {fv} {sort})"));
        self.axioms.push(format!("(= {fs} (> {count_post} 0))"));
        let mut post_members = Vec::new();
        for c in self.named_members(&entity) {
            let known = c.guard.is_none();
            let m = self.member(&inst, &c, env, obs)?;
            let v = self.value_of(body, &c, env, obs)?;
            if known {
                self.axioms.push(format!("(=> {m} ({op} {fv} {v}))"));
            }
            post_members.push((m, v));
        }
        self.define_extremum(&fs, &fv, &post_members, min);
        Ok(Term::Opt { some: fs, val: fv })
    }

    /// Normalizes a quantifier's predicate: an equality between a candidate-only key and a
    /// candidate-free value becomes an equality narrowing.
    fn narrow_of<'e>(
        &mut self,
        p: &'e Expr,
        positive: bool,
        env: &Env,
        obs: &mut Vec<Obligation>,
    ) -> R<Narrow<'e>> {
        if !positive {
            return Ok(Narrow::NotPred(p));
        }
        if let ExprKind::Cmp(behavior_core::semantic::types::CmpOp::Eq, a, b) = p.kind() {
            for (key, value) in [(a.as_ref(), b.as_ref()), (b.as_ref(), a.as_ref())] {
                if reads_candidate(key)
                    && capture_exprs(&[key])?.is_empty()
                    && !reads_candidate(value)
                {
                    let r = self.encode(value, env)?;
                    obs.extend(r.obligations);
                    return Ok(Narrow::Eq { key, value: r.term });
                }
            }
        }
        Ok(Narrow::Pred(p))
    }

    fn unique_of(
        &mut self,
        q: &QueryNode,
        body: &Expr,
        delta: bool,
        env: &Env,
        obs: &mut Vec<Obligation>,
    ) -> R<String> {
        let entity = q.entity().to_string();
        self.ensure_slots(&entity)?;
        let all = Inst {
            q,
            narrow: Narrow::None,
        };
        if self.post && delta {
            // Research R7: `unique` held on S, so only touched members can collide; each must be
            // the only member with its key on S'.
            let mut conds = Vec::new();
            let mut touched: Vec<Binding> = self
                .touched(&entity)
                .into_iter()
                .filter_map(|(_, p)| p)
                .collect();
            touched.extend(self.created(&entity));
            for b in touched {
                let c = Cand {
                    binding: b,
                    guard: None,
                };
                let m = self.member(&all, &c, env, obs)?;
                let mut env2 = env.clone();
                env2.insert(CANDIDATE.to_string(), c.binding.clone());
                let k = self.encode(body, &env2)?;
                obs.extend(k.obligations);
                let same = Inst {
                    q,
                    narrow: Narrow::Eq {
                        key: body,
                        value: k.term,
                    },
                };
                let n = self.count_of(&same, env, obs)?;
                conds.push(format!("(=> {m} (= {n} 1))"));
            }
            return Ok(and_all(&conds));
        }
        let members: Vec<(String, String)> = {
            let mut out = Vec::new();
            for c in self.named_members(&entity) {
                let m = self.member(&all, &c, env, obs)?;
                let v = self.value_of(body, &c, env, obs)?;
                out.push((m, v));
            }
            out
        };
        let flag = if self.post {
            self.fresh += 1;
            let f = format!("uniq!post!{}", self.fresh);
            self.decls.push(format!("(declare-const {f} Bool)"));
            f
        } else {
            let (args, cap_obs) = self.inst_args(&all, env)?;
            obs.extend(cap_obs);
            let f = self.uf_app(
                &format!("uniq!{}!{}", all.key(), short(body.hash())),
                &args,
                "Bool",
            );
            self.uniq_flags.push((
                format!("{}!{}", short(q.hash()), short(body.hash())),
                f.clone(),
            ));
            f
        };
        let mut clashes = Vec::new();
        for (i, (mi, vi)) in members.iter().enumerate() {
            for (mj, vj) in &members[i + 1..] {
                clashes.push(format!("(and {mi} {mj} (= {vi} {vj}))"));
            }
        }
        self.witness
            .push(format!("(= {flag} {})", not(&or_all(&clashes))));
        Ok(flag)
    }

    /// `count(q)` or a relational operator with a lambda.
    pub(super) fn encode_relational(&mut self, e: &Expr, env: &Env) -> R<Encoded> {
        let env = env.clone();
        let mut obs: Vec<Obligation> = Vec::new();
        let term = match e.kind() {
            ExprKind::Count(q) => {
                let inst = Inst {
                    q,
                    narrow: Narrow::None,
                };
                Term::Plain(self.count_of(&inst, &env, &mut obs)?)
            }
            ExprKind::Fold {
                op, query, body, ..
            } => match op {
                FoldOp::Any | FoldOp::All => {
                    let positive = *op == FoldOp::Any;
                    let narrow = self.narrow_of(body, positive, &env, &mut obs)?;
                    let inst = Inst { q: query, narrow };
                    let n = self.count_of(&inst, &env, &mut obs)?;
                    Term::Plain(if positive {
                        format!("(> {n} 0)")
                    } else {
                        format!("(= {n} 0)")
                    })
                }
                FoldOp::Sum => {
                    let s = self.sum_of(query, body, &env, &mut obs)?;
                    // Partial sums are range-checked in canonical order: a total out of range
                    // certainly fails; anything else may (a free flag).
                    self.fresh += 1;
                    let flag = format!("qerr!{}", self.fresh);
                    self.decls.push(format!("(declare-const {flag} Bool)"));
                    let out = if let Some(sc) = fixed_scale(body.ty()).and_then(|n| n.scale) {
                        Some(out_of_range(&s, sc))
                    } else {
                        self.overflow(body.ty(), &s)
                    };
                    if let Some(c) = out {
                        self.axioms.push(format!("(=> {c} {flag})"));
                        // A witness of the error: a total out of range (the runtime's final check
                        // fails whatever the order).
                        self.witness.push(format!("(= {flag} {c})"));
                    }
                    let mut o = Self::obligation(body, ErrKind::Overflow, flag);
                    o.guard = "true".into();
                    obs.push(o);
                    Term::Plain(s)
                }
                FoldOp::Min | FoldOp::Max => {
                    self.extremum_of(query, body, *op == FoldOp::Min, &env, &mut obs)?
                }
                FoldOp::Unique => {
                    // Only a `unique` the invariant guarantees on S has the delta rule.
                    let delta = self.delta_unique.contains(e.hash());
                    Term::Plain(self.unique_of(query, body, delta, &env, &mut obs)?)
                }
            },
            _ => return Err(EncodeError::Unsupported("not a relational form".into())),
        };
        // Candidate evaluation may fail on members of unknown order: free flags (see above);
        // the sum's own range flag is already one.
        let (flags, direct): (Vec<Obligation>, Vec<Obligation>) =
            obs.into_iter().partition(|o| !o.cond.starts_with("qerr!"));
        let mut obligations = self.error_flags(flags);
        obligations.extend(direct);
        Ok(Encoded { term, obligations })
    }

    /// Witness-only facts at script time: registered existence facts of a queried type are
    /// exactly membership in the named universe (known candidates and present slots).
    pub(super) fn witness_facts(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.facts {
            if f.kind != FactKind::Exists {
                continue;
            }
            let Some(slots) = self.slots.get(&f.entity) else {
                continue;
            };
            let mut alts: Vec<String> = self
                .known_ids(&f.entity)
                .iter()
                .map(|k| format!("(= {} {k})", f.id))
                .collect();
            for s in slots {
                if let Binding::Entity { fields, .. } = &s.binding
                    && let Some(Ok(id)) = fields.get("id").map(Term::plain)
                {
                    alts.push(format!("(and {} (= {} {id}))", s.present, f.id));
                }
            }
            out.push(format!("(= {} {})", f.value, or_all(&alts)));
        }
        out
    }

    /// `unique` assumed on S bounds every equality count of its key by one.
    pub(super) fn unique_links(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (k, flag) in &self.uniq_flags {
            for (k2, app) in &self.eq_counts {
                if k == k2 {
                    out.push(format!("(=> {flag} (<= {app} 1))"));
                }
            }
        }
        out
    }
}
