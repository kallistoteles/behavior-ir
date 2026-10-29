//! Delegating public Backend probe. Counts real Core reads, never implements validity.
#![forbid(unsafe_code)]
use behavior_engine::store::documents::*;
use behavior_engine::store::{Backend, BackendError, CasOutcome, InMemoryBackend, RefEdge, Store};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, path::Path, rc::Rc, time::Instant};

#[derive(Default, Clone)]
struct Counts {
    enumerations: usize,
    members: usize,
    versions: usize,
    removals: usize,
}
impl Counts {
    fn json(&self) -> Value {
        json!({"keys_at_calls":self.enumerations,"keys_returned":self.members,
        "version_at_calls":self.versions,"removed_at_calls":self.removals})
    }
}
struct Observed {
    inner: InMemoryBackend,
    counts: Rc<RefCell<Counts>>,
}
impl Backend for Observed {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.inner.genesis()
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.inner.head()
    }
    fn create(
        &mut self,
        g: &Genesis,
        h: &Head,
        s: &[EntityVersion],
        r: &[RefChange],
    ) -> Result<(), BackendError> {
        self.inner.create(g, h, s, r)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.counts.borrow_mut().versions += 1;
        self.inner.version_at(k, p)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.inner.record(p)
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.counts.borrow_mut().removals += 1;
        self.inner.removed_at(k)
    }
    fn incoming_at(&self, k: &EntityKey, p: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.inner.incoming_at(k, p)
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        let rows = self.inner.keys_at(t, p)?;
        let mut c = self.counts.borrow_mut();
        c.enumerations += 1;
        c.members += rows.len();
        Ok(rows)
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        self.inner.keys_by_field_at(t, f, v, p)
    }
    fn used_at(&self, k: &EntityKey, p: u64) -> Result<bool, BackendError> {
        self.inner.used_at(k, p)
    }
    fn commit(
        &mut self,
        e: &str,
        v: &[EntityVersion],
        r: &[EntityKey],
        f: &[RefChange],
        t: &TransitionRecord,
        h: &Head,
    ) -> Result<CasOutcome, BackendError> {
        self.inner.commit(e, v, r, f, t, h)
    }
}

fn run(path: &Path, n: usize, history: usize, mode: &str) -> Value {
    let module = behavior_engine::admit(&std::fs::read_to_string(path).unwrap()).unwrap();
    let counts = Rc::new(RefCell::new(Counts::default()));
    let backend = Observed {
        inner: InMemoryBackend::default(),
        counts: counts.clone(),
    };
    let seed = (0..n)
        .map(|i| SeedEntity {
            entity: "Target".into(),
            value: json!({"id":format!("t{i}"),"value":0}),
        })
        .collect();
    let policy=behavior_engine::verify::governance::EvidencePolicyV2::from_json(
        r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#).unwrap();
    let genesis = behavior_engine::store::store::genesis_v2_for(&module, policy, seed).unwrap();
    let mut store = Store::create(backend, &module, genesis).unwrap();
    let initial = store.current().unwrap();
    let bindings = BTreeMap::from([("target".into(), "t0".into())]);
    let now = "2026-10-24T00:00:00Z";
    let mut observed = Vec::new();
    for step in 0..history + 3 {
        *counts.borrow_mut() = Counts::default();
        let start = Instant::now();
        let ev = store
            .evaluate(
                &module,
                "update",
                &bindings,
                &json!({}),
                &json!({}),
                now,
                None,
            )
            .unwrap();
        let elapsed = start.elapsed().as_nanos();
        assert_eq!(ev.record["result"], "ALLOW");
        let reads = counts.borrow().json();
        // A second evaluation at the exact same head is the cache-hit control.
        *counts.borrow_mut() = Counts::default();
        let repeat = store
            .evaluate(
                &module,
                "update",
                &bindings,
                &json!({}),
                &json!({}),
                now,
                None,
            )
            .unwrap();
        assert_eq!(repeat.record, ev.record);
        let cached = counts.borrow().json();
        assert_eq!(cached["keys_returned"], 0);
        store
            .commit(
                &module,
                &store.current().unwrap(),
                ev.bundle.as_ref().unwrap(),
            )
            .unwrap();
        assert_eq!(store.current().unwrap().position, (step + 1) as u64);
        if step >= history {
            observed.push(json!({"parent_position":step,"evaluate_ns":elapsed,
            "reads":reads,"same_head_reads":cached}));
        }
    }
    // Separate creation control: keep the evaluation measurement after insertion.
    let mut growth = Vec::new();
    for i in 0..3 {
        let ev = store
            .evaluate(
                &module,
                "add",
                &BTreeMap::new(),
                &json!({"new_id":format!("new{i}")}),
                &json!({}),
                now,
                None,
            )
            .unwrap();
        assert_eq!(ev.record["result"], "ALLOW");
        store
            .commit(
                &module,
                &store.current().unwrap(),
                ev.bundle.as_ref().unwrap(),
            )
            .unwrap();
        *counts.borrow_mut() = Counts::default();
        let ev = store
            .evaluate(
                &module,
                "update",
                &bindings,
                &json!({}),
                &json!({}),
                now,
                None,
            )
            .unwrap();
        assert_eq!(ev.record["result"], "ALLOW");
        growth.push(json!({"live":n+i+1,"reads":counts.borrow().json()}));
    }
    // Replay runs after all measured counts have been copied, never in their scope.
    let final_state = store.current().unwrap();
    let data = behavior_engine::store::replay::replay_data(&store, &initial, &final_state);
    let modules = BTreeMap::from([(module.behavior_version(), module.clone())]);
    let behavior = behavior_engine::store::replay::replay_behavior_with(
        &store,
        &modules,
        &BTreeMap::new(),
        &initial,
        &final_state,
    );
    assert!(data.ok && behavior.ok);
    assert_eq!(behavior.checked, final_state.position);
    json!({"mode":mode,"live":n,"history_warmup":history,"behavior_hash":module.behavior_version(),
        "observations":observed,"after_creation":growth,"final_position":final_state.position,
        "data_replay_ok":data.ok,"behavior_replay_ok":behavior.ok,"replayed":behavior.checked})
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    let minimal = args.iter().any(|a| a == "--minimal");
    let mut results = Vec::new();
    let modes: &[&str] = if minimal {
        &["local", "global_true"]
    } else {
        &["local", "global_true", "global_query"]
    };
    let sizes: &[usize] = if minimal { &[16] } else { &[1, 16, 128, 512] };
    for &mode in modes {
        for &n in sizes {
            results.push(run(&input.join(format!("008-{mode}.json")), n, 0, mode));
        }
        if !minimal {
            results.push(run(&input.join(format!("008-{mode}.json")), 16, 32, mode));
        }
    }
    std::fs::write(
        out,
        serde_json::to_string_pretty(
            &json!({"core":behavior_engine::engine_info(),"cases":results}),
        )
        .unwrap(),
    )
    .unwrap();
    for r in &results {
        println!(
            "{} N={} history={} first/postcommit keys={:?} cached={:?}",
            r["mode"],
            r["live"],
            r["history_warmup"],
            r["observations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["reads"]["keys_returned"].clone())
                .collect::<Vec<_>>(),
            r["observations"][0]["same_head_reads"]["keys_returned"]
        );
    }
    if args.iter().any(|a| a == "--require-local-promotion") {
        for r in &results {
            if r["mode"] == "global_true" && r["live"].as_u64().unwrap() > 1 {
                assert_eq!(
                    r["observations"][1]["reads"]["keys_returned"], 0,
                    "Gap008: even a constant global invariant prevents incremental promotion"
                );
            }
        }
    }
}
