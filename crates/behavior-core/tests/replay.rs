#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use behavior_core::{admit, evaluate, replay};
use serde_json::Value;

fn wire(name: &str) -> String {
    common::read(&common::fixtures().join(format!("wire/valid/{name}.json")))
}

fn cases() -> Vec<(String, String)> {
    let exp = common::json(&common::fixtures().join("requests/expectations.json"));
    exp.as_object()
        .unwrap()
        .iter()
        .map(|(n, e)| (n.clone(), e["wire"].as_str().unwrap().to_string()))
        .collect()
}

#[test]
fn every_record_replays() {
    for (name, w) in cases() {
        let m = admit(&wire(&w)).unwrap();
        let request = common::read(&common::fixtures().join(format!("requests/{name}.json")));
        let record = evaluate(&m, &request).to_json_string();
        let r = replay(&m, &record);
        assert!(r.matches, "{name}: {:?}", r.diff);
        assert_eq!(r.to_json_string(), r#"{"matches":true}"#);
    }
}

#[test]
fn changed_behavior_is_a_mismatch() {
    let m = admit(&wire("invoice")).unwrap();
    let request = common::read(&common::fixtures().join("requests/invoice_allowed.json"));
    let record = evaluate(&m, &request).to_json_string();
    let mut changed: Value = serde_json::from_str(&wire("invoice")).unwrap();
    changed["actions"][0]["preconditions"][1]["expr"]["args"][1]["value"] = "controller".into();
    let m2 = admit(&changed.to_string()).unwrap();
    let r = replay(&m2, &record);
    assert!(!r.matches);
    assert!(r.diff.unwrap().contains("behavior_version"));
}

#[test]
fn altered_record_is_a_mismatch_with_path() {
    let m = admit(&wire("invoice")).unwrap();
    let request = common::read(&common::fixtures().join("requests/invoice_allowed.json"));
    let mut record: Value = serde_json::from_str(&evaluate(&m, &request).to_json_string()).unwrap();
    record["trace"][3]["reads"]["invoice.amount"] = "1".into();
    let r = replay(&m, &record.to_string());
    assert!(!r.matches);
    let diff = r.diff.unwrap();
    assert!(diff.contains("trace[3].reads"), "{diff}");
}
