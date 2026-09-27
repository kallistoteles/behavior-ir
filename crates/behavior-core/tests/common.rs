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

/// Checks that `expected` is a JSON subset of `got` (objects: listed keys; arrays: same length,
/// matching elements). Keys starting with `$` are directives: `$contains` maps dotted paths to
/// substrings the string there must contain; `$absent` lists dotted paths that must not exist.
pub fn subset_problems(expected: &serde_json::Value, got: &serde_json::Value) -> Vec<String> {
    use serde_json::Value;
    fn walk(expected: &Value, got: &Value, path: &str, out: &mut Vec<String>) {
        match (expected, got) {
            (Value::Object(e), Value::Object(g)) => {
                for (k, v) in e {
                    if k.starts_with('$') {
                        continue;
                    }
                    match g.get(k) {
                        Some(gv) => walk(v, gv, &format!("{path}.{k}"), out),
                        None => out.push(format!("{path}.{k}: missing")),
                    }
                }
            }
            (Value::Array(e), Value::Array(g)) => {
                if e.len() != g.len() {
                    out.push(format!(
                        "{path}: expected {} elements, got {}: {got}",
                        e.len(),
                        g.len()
                    ));
                    return;
                }
                for (i, (ev, gv)) in e.iter().zip(g).enumerate() {
                    walk(ev, gv, &format!("{path}[{i}]"), out);
                }
            }
            (e, g) if e == g => {}
            (e, g) => out.push(format!("{path}: expected {e}, got {g}")),
        }
    }
    fn at<'a>(v: &'a Value, dotted: &str) -> Option<&'a Value> {
        dotted
            .split('.')
            .try_fold(v, |cur, part| match part.parse::<usize>() {
                Ok(i) => cur.get(i),
                Err(_) => cur.get(part),
            })
    }
    let mut out = Vec::new();
    walk(expected, got, "", &mut out);
    if let Some(Value::Object(c)) = expected.get("$contains") {
        for (p, needles) in c {
            let s = at(got, p).and_then(Value::as_str).unwrap_or_default();
            for n in needles.as_array().unwrap() {
                if !s.contains(n.as_str().unwrap()) {
                    out.push(format!("{p}: `{s}` does not contain {n}"));
                }
            }
        }
    }
    if let Some(Value::Array(paths)) = expected.get("$absent") {
        for p in paths {
            if at(got, p.as_str().unwrap()).is_some() {
                out.push(format!("{p}: should be absent"));
            }
        }
    }
    out
}
