//! WorkQueue lowering laws over random definitions (feature 507, T032; premises P1, P3, P4, P7
//! of contracts/safety-arguments.md). The safety properties over random *histories* run through
//! the Store in `models/examples/work_queue/test_obligations.py` (PROP.mono); here the generated
//! module itself is checked for every M, payload and capability order.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::WType;
use common::wq::*;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// 1,000 cases in full mode (`BEHAVIOR_FULL_PROPERTIES=1`, as the gates run in full); 100 in
/// the quick gate, since each case builds two modules in a debug build.
fn config() -> ProptestConfig {
    let full = std::env::var("BEHAVIOR_FULL_PROPERTIES").as_deref() == Ok("1");
    ProptestConfig {
        cases: if full { 1000 } else { 100 },
        rng_seed: RngSeed::Fixed(507),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn payload() -> impl Strategy<Value = Vec<(String, WType)>> {
    proptest::sample::subsequence(
        vec![
            ("document".to_string(), WType::Id("Document".into())),
            ("priority".to_string(), WType::Int),
            ("label".to_string(), WType::String),
            ("urgent".to_string(), WType::Bool),
        ],
        0..=4,
    )
    .prop_shuffle()
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn every_generated_module_keeps_the_premises(
        m in 1i64..1_000_000,
        payload in payload(),
        order in Just((0..9).collect::<Vec<usize>>()).prop_shuffle(),
    ) {
        let mut q = queue(m);
        q.payload = payload.clone();
        q.capabilities = order.iter().map(|i| capability(CAPABILITIES[*i])).collect();
        let out = build_with(|_| {}, vec![spec(q, source())]).unwrap();
        // P1: only claim writes `attempts`.
        for (name, action) in out.module.actions() {
            let writes = action.effects().iter().any(|e| e.field() == "attempts");
            prop_assert_eq!(writes, name == "document_work__claim");
        }
        // P4: renew, complete, retry and fail ensure `attempts == generation`; claim ensures
        // the increment (two generated postconditions after the machine's status one).
        for c in ["claim", "renew", "complete", "retry", "fail"] {
            let a = &out.module.actions()[&format!("document_work__{c}")];
            prop_assert_eq!(a.postconditions().len(), 2);
        }
        // Identity: independent of the capability and payload order.
        let mut sorted_payload = payload;
        sorted_payload.sort_by(|a, b| a.0.cmp(&b.0));
        let mut r = queue(m);
        r.payload = sorted_payload;
        let reference = build_with(|_| {}, vec![spec(r, source())]).unwrap();
        prop_assert_eq!(&out.receipts[0].0["model_hash"], &reference.receipts[0].0["model_hash"]);
        prop_assert_eq!(out.module.behavior_version(), reference.module.behavior_version());
    }
}
