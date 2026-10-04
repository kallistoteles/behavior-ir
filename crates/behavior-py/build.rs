//! Records the Core Release this extension is built against, as core-release.json declares it
//! (feature 011, FR-017): the binding reports it and refuses to import on any other core.

use std::fs;
use std::path::Path;

fn main() {
    let pin_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core-release.json");
    println!("cargo::rerun-if-changed={}", pin_path.display());
    let text = fs::read_to_string(&pin_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", pin_path.display()));
    let pin: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not JSON: {e}", pin_path.display()));
    for (key, var) in [
        ("version", "BEHAVIOR_CORE_VERSION"),
        ("commit", "BEHAVIOR_CORE_COMMIT"),
    ] {
        let value = pin[key]
            .as_str()
            .unwrap_or_else(|| panic!("{} has no {key}", pin_path.display()));
        println!("cargo::rustc-env={var}={value}");
    }
}
