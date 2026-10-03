//! The store schema (feature 009, research R1): the persisted-state schema under which a state is
//! valid. It is the set of entity declarations; enum and nominal types reachable from persisted
//! fields enter through the declaration hashes, which hash them by content. Behavior-only
//! definitions (actions, rules, derived values, invariants, constraints) are not part of it.

use std::collections::BTreeMap;

use crate::admit::hash;
use crate::semantic::module::{Kind, Module};
use crate::semantic::types::hash_display;

/// The domain tag of a SchemaHash.
pub const TAG_STORE_SCHEMA: &str = hash::TAG_STORE_SCHEMA;

/// A store schema: entity name → declaration hash (`sha256:…`), and its SchemaHash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSchema {
    pub hash: String,
    pub declarations: BTreeMap<String, String>,
}

impl StoreSchema {
    /// The schema of a declaration map (e.g. a genesis's `entity_declarations`).
    pub fn of(declarations: BTreeMap<String, String>) -> StoreSchema {
        StoreSchema {
            hash: hash_display(&hash::store_schema(&declarations)),
            declarations,
        }
    }

    /// The entity types declared differently in `other` (or in only one of the two), sorted.
    pub fn differing(&self, other: &StoreSchema) -> Vec<String> {
        let mut names: Vec<String> = self
            .declarations
            .keys()
            .chain(other.declarations.keys())
            .filter(|n| self.declarations.get(*n) != other.declarations.get(*n))
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

/// The entity declarations of `module`: entity name → declaration hash.
pub fn declarations(module: &Module) -> BTreeMap<String, String> {
    module
        .name_table()
        .iter()
        .filter(|((k, _), _)| *k == Kind::Entity)
        .map(|((_, n), h)| (n.clone(), hash_display(h)))
        .collect()
}

/// The store schema `module` declares.
pub fn schema(module: &Module) -> StoreSchema {
    StoreSchema::of(declarations(module))
}
