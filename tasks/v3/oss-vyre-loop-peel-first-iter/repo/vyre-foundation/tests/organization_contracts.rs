//! Organization-level contract tests for the vyre-foundation ecosystem.
//!
//! These tests enforce long-term structural contracts without relying on
//! brittle message wording. They may fail when code violates a contract.

use std::collections::HashSet;
use std::path::PathBuf;

use vyre_foundation::error::Error;
use vyre_foundation::graph_view::{
    from_graph, DataEdge, DataflowKind, EdgeKind, GraphNode, GraphValidateError, NodeGraph,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::program_caps::{
    check_backend_capabilities, MissingCapability, RequiredCapabilities,
};
use vyre_foundation::validate::validate;
use vyre_foundation::validate::ValidationError;

// ---------------------------------------------------------------------------
// 1. No wildcard public re-export expansion in new modules
// ---------------------------------------------------------------------------

/// Baseline the existing wildcard pub re-exports in vyre-foundation and fail
/// if any new ones are introduced. Expansion increases API surface unpredictably.

mod organization_contracts_part1 {

    include!("__split/organization_contracts_part1.rs");
}
mod organization_contracts_part2 {
    include!("__split/organization_contracts_part2.rs");
}
mod organization_contracts_part3 {
    include!("__split/organization_contracts_part3.rs");
}
