//! Embeds the pinned Core Release (`core-release.json`) so that every lowering receipt names the
//! exact Core it was produced against (feature 501, FR-001, FR-010).

use std::path::Path;

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core-release.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let pin: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid core-release.json: {e}"));
    let field = |name: &str| {
        pin[name]
            .as_str()
            .unwrap_or_else(|| panic!("core-release.json lacks `{name}`"))
            .to_string()
    };
    println!(
        "cargo:rustc-env=BEHAVIOR_CORE_RELEASE_VERSION={}",
        field("version")
    );
    println!(
        "cargo:rustc-env=BEHAVIOR_CORE_RELEASE_COMMIT={}",
        field("commit")
    );
}
