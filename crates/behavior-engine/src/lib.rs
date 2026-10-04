//! The only supported programmatic API of Behavior Core (feature 011, FR-006).
//!
//! Bindings, the command-line tool and every other consumer depend on this crate alone. The
//! crates behind it (`behavior-core`, `behavior-store`, `behavior-verify`) are implementation
//! details that may be reorganized freely. Every item is re-exported explicitly, one per line,
//! under the namespace of its original module; `api/engine-surface.txt` lists them and
//! `scripts/check-public-surface.sh` keeps the two equal. A wildcard re-export is never allowed.
#![forbid(unsafe_code)]

/// Every version of this release: the engine and its formats, the store document tags, and the
/// verifier (feature 008, research R2). The command line prints exactly this.
pub fn engine_info() -> serde_json::Value {
    let mut v = behavior_core::format_versions();
    if let serde_json::Value::Object(m) = &mut v {
        m.insert(
            "store_documents".into(),
            serde_json::json!(behavior_store::documents::DOCUMENT_TAGS),
        );
        m.insert(
            "verifier".into(),
            serde_json::json!(behavior_verify::VERIFIER_VERSION),
        );
    }
    v
}

pub use behavior_core::AdmissionResult;
pub use behavior_core::DecisionRecord;
pub use behavior_core::IntentRejection;
pub use behavior_core::admission_report;
pub use behavior_core::admission_result;
pub use behavior_core::admit;
pub use behavior_core::evaluate;
pub use behavior_core::evaluate_intent;
pub use behavior_core::format_versions;
pub use behavior_core::replay;
pub use behavior_core::schema;
pub mod builder {
    pub use behavior_core::builder::BuildError;
    pub use behavior_core::builder::Builder;
    pub use behavior_core::builder::Node;
    pub use behavior_core::builder::ScopeSite;
}
pub mod canonical {
    pub use behavior_core::canonical::to_canonical_string;
}
pub mod migration {
    pub use behavior_core::migration::Migration;
    pub use behavior_core::migration::SourceEntity;
    pub use behavior_core::migration::admit_migration;
    pub use behavior_core::migration::apply_migration;
    pub use behavior_core::migration::outcome_json;
}
pub mod read {
    pub use behavior_core::read::ReadExecution;
    pub use behavior_core::read::ReadSource;
    pub use behavior_core::read::evaluate_read;
    pub use behavior_core::read::evaluate_read_intent;
    pub use behavior_core::read::evaluate_read_request;
    pub use behavior_core::read::replay_read;
}
pub mod semantic {
    pub use behavior_core::semantic::Module;
    pub mod module {
        pub use behavior_core::semantic::module::Module;
        pub use behavior_core::semantic::module::ReadItem;
    }
    pub mod types {
        pub use behavior_core::semantic::types::hash_display;
    }
}
pub mod serialize {
    pub use behavior_core::serialize::to_wire_json;
}
pub mod store {
    pub use behavior_store::Backend;
    pub use behavior_store::BackendError;
    pub use behavior_store::CasOutcome;
    pub use behavior_store::InMemoryBackend;
    pub use behavior_store::RefEdge;
    pub use behavior_store::Store;
    pub mod conformance {
        pub use behavior_store::conformance::run;
    }
    pub mod documents {
        pub use behavior_store::documents::CommitBundle;
        pub use behavior_store::documents::DOCUMENT_TAGS;
        pub use behavior_store::documents::EntityKey;
        pub use behavior_store::documents::EntityVersion;
        pub use behavior_store::documents::Evidence;
        pub use behavior_store::documents::EvidencePolicy;
        pub use behavior_store::documents::Genesis;
        pub use behavior_store::documents::Head;
        pub use behavior_store::documents::RefChange;
        pub use behavior_store::documents::SeedEntity;
        pub use behavior_store::documents::StateRef;
        pub use behavior_store::documents::StoreError;
        pub use behavior_store::documents::TransitionRecord;
        pub use behavior_store::documents::data_version;
        pub use behavior_store::documents::decode;
    }
    pub mod replay {
        pub use behavior_store::replay::replay_behavior_with;
        pub use behavior_store::replay::replay_data;
    }
    #[allow(clippy::module_inception)]
    pub mod store {
        pub use behavior_store::store::genesis_for;
    }
}
pub mod verify {
    pub use behavior_verify::CheckKind;
    pub use behavior_verify::Profile;
    pub use behavior_verify::VERIFIER_VERSION;
    pub use behavior_verify::verify;
    pub use behavior_verify::verify_migration;
    pub mod governance {
        pub use behavior_verify::governance::GovernanceError;
        pub use behavior_verify::governance::authorize;
        pub use behavior_verify::governance::authorize_migration;
        pub use behavior_verify::governance::sign_waiver;
        pub use behavior_verify::governance::waiver_hash;
    }
    pub mod solver {
        pub use behavior_verify::solver::Z3Process;
    }
}
pub mod wire {
    pub use behavior_core::wire::DerivedKind;
    pub use behavior_core::wire::Loc;
    pub use behavior_core::wire::Role;
    pub use behavior_core::wire::TARGET_SIDE;
    pub use behavior_core::wire::WField;
    pub use behavior_core::wire::WItem;
    pub use behavior_core::wire::WLifecycle;
    pub use behavior_core::wire::WParam;
    pub use behavior_core::wire::WRead;
    pub use behavior_core::wire::WType;
}
