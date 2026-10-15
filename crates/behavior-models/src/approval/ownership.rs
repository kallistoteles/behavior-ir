//! Approval's ownership rules over the admitted module (data model §7). Implemented in T032.

use behavior_engine::semantic::Module;

use crate::approval::resolve::ResolvedApproval;
use crate::error::ModelError;

/// Every violation of the decision rules in `module`, over all approvals.
pub fn check(_module: &Module, _approvals: &[ResolvedApproval]) -> Vec<ModelError> {
    Vec::new()
}
