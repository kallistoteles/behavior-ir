//! Check results cached by check key (research R8).
//!
//! A directory of canonical JSON files named by the key's hex digest. The key covers every
//! content hash the outcome depends on plus the profile, verifier, and solver versions, so an
//! entry is valid exactly as long as its key is. Results stopped by the wall-clock guard are
//! never written. Unreadable or mismatching entries are ignored (recomputed).

use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use behavior_core::canonical::to_canonical_string;

use crate::checks::CheckResult;

/// The default cache directory, relative to the working directory.
pub const DEFAULT_DIR: &str = ".behavior/verify-cache";

#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(dir: &Path) -> Cache {
        Cache {
            dir: dir.to_path_buf(),
        }
    }

    fn path(&self, key: &str) -> Option<PathBuf> {
        let hex = key.strip_prefix("sha256:")?;
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(self.dir.join(format!("{hex}.json")))
    }

    pub fn get(&self, key: &str) -> Option<CheckResult> {
        let text = std::fs::read_to_string(self.path(key)?).ok()?;
        let entry: Json = serde_json::from_str(&text).ok()?;
        if entry["check"]["key"] != key {
            return None;
        }
        let finding = match &entry["finding"] {
            Json::Null => None,
            f => Some(f.clone()),
        };
        CheckResult::from_json(&entry["check"], finding)
    }

    /// Stores a reproducible result; failures to write only cost a recomputation later.
    pub fn put(&self, result: &CheckResult) {
        if !result.reproducible() {
            return;
        }
        let Some(path) = self.path(&result.key) else {
            return;
        };
        let entry = json!({"check": result.to_json(false), "finding": result.finding});
        let Ok(text) = to_canonical_string(&entry) else {
            return;
        };
        if std::fs::create_dir_all(&self.dir).is_ok() {
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}
