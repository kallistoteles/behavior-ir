#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Wire IR 0.7 (feature 010): the module `reads` section and the read document. Reads need 0.7;
//! decoding is strict like every other wire form.

mod common;

use behavior_core::admission_report;
use behavior_core::wire::{DecodeError, WItem, WReadBody, decode_module, decode_read_document};
use serde_json::{Value, json};

fn lab() -> Value {
    common::json(&common::fixtures().join("reads/modules/lab.json"))
}

fn adhoc(name: &str) -> Value {
    common::json(&common::fixtures().join(format!("reads/valid/{name}.json")))
}

fn structure(r: Result<impl std::fmt::Debug, DecodeError>) -> (String, String) {
    match r {
        Err(DecodeError::Structure { path, message }) => (path, message),
        other => panic!("expected a structural decode error, got {other:?}"),
    }
}

fn structure_path(r: Result<impl std::fmt::Debug, DecodeError>) -> String {
    structure(r).0
}

#[test]
fn reads_need_0_7() {
    let mut doc = lab();
    doc["ir_version"] = json!("0.6");
    assert_eq!(
        decode_module(&doc.to_string()).err(),
        Some(DecodeError::NeedsReadVersion("reads".into()))
    );
    let r = admission_report(&doc.to_string());
    let codes: Vec<&str> = r.errors.iter().map(|e| e.code.as_str()).collect();
    assert_eq!(codes, ["UNSUPPORTED_IR_VERSION"], "{:?}", r.errors);
    assert!(r.errors[0].message.contains("0.7"), "{:?}", r.errors);
}

#[test]
fn a_module_with_reads_decodes_at_0_7() {
    let w = decode_module(&lab().to_string()).unwrap();
    let names: Vec<&str> = w.reads.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "active_count",
            "open_total",
            "order_view",
            "active_cultures",
            "customer_summary",
            "smallest_order",
            "average_ph",
            "big_orders"
        ]
    );
    let view = &w.reads[2];
    match &view.body {
        WReadBody::Project { param, items, .. } => {
            assert_eq!(param, "o");
            assert_eq!(
                items,
                &[WItem::Field("status".into()), WItem::Field("amount".into())]
            );
        }
        other => panic!("order_view is a projection: {other:?}"),
    }
    assert!(matches!(w.reads[0].body, WReadBody::Value(_)));
    // Without a `reads` section, a 0.7 module decodes with no reads.
    let empty = common::read(&common::fixtures().join("reads/modules/lab_empty_reads.json"));
    assert!(decode_module(&empty).unwrap().reads.is_empty());
    let mut none = common::json(&common::fixtures().join("reads/modules/lab_empty_reads.json"));
    none.as_object_mut().unwrap().remove("reads");
    assert!(decode_module(&none.to_string()).unwrap().reads.is_empty());
}

#[test]
fn a_read_document_decodes() {
    let r = decode_read_document(&adhoc("orders_over").to_string()).unwrap();
    assert_eq!(r.name, "orders_over");
    assert_eq!(r.params.len(), 1);
    assert!(matches!(r.body, WReadBody::Project { .. }));
    // A read document needs 0.7 and nothing but `ir_version` and `read`.
    let mut old = adhoc("customer_count");
    old["ir_version"] = json!("0.6");
    assert!(matches!(
        decode_read_document(&old.to_string()),
        Err(DecodeError::UnsupportedVersion(_))
    ));
    let mut extra = adhoc("customer_count");
    extra["module"] = json!({});
    let (path, message) = structure(decode_read_document(&extra.to_string()));
    assert_eq!(path, "$");
    assert!(message.contains("`module`"), "{message}");
}

#[test]
fn read_decoding_is_strict() {
    let mut unknown = lab();
    unknown["reads"][0]["extra"] = json!(true);
    let (path, message) = structure(decode_module(&unknown.to_string()));
    assert_eq!(path, "$.reads[0]");
    assert!(message.contains("`extra`"), "{message}");

    let mut both = lab();
    let value = both["reads"][0]["body"]["value"].clone();
    both["reads"][0]["body"]["project"] = json!({"over": value, "param": "x", "items": []});
    assert!(
        structure_path(decode_module(&both.to_string())).starts_with("$.reads[0].body"),
        "a body has exactly one of `value` and `project`"
    );

    let mut neither = lab();
    neither["reads"][0]["body"] = json!({});
    assert!(structure_path(decode_module(&neither.to_string())).starts_with("$.reads[0].body"));

    let mut item = lab();
    item["reads"][2]["body"]["project"]["items"][0]["derived"] = json!("total");
    assert!(
        structure_path(decode_module(&item.to_string()))
            .starts_with("$.reads[2].body.project.items[0]")
    );

    let mut role = adhoc("orders_over");
    role["read"]["params"][0]["role"] = json!("read");
    assert_eq!(
        structure_path(decode_read_document(&role.to_string())),
        "$.read.params[0].role"
    );
}
