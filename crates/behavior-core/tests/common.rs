//! Helpers shared by integration tests (included with `mod common;`).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn fixtures() -> PathBuf {
    repo_root().join("tests/fixtures")
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

pub fn json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&read(path)).unwrap()
}

/// Sorted list of files in `dir` with the given suffix, excluding `.expected.json`.
pub fn files(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            n.ends_with(suffix) && !n.ends_with(".expected.json")
        })
        .collect();
    out.sort();
    out
}
