//! Release-gate types for clang/vyrec parity reports.
//!
//! These types intentionally model the launch decision, not the fact extraction
//! machinery. Clang extraction, vyrec extraction, and object decoding can evolve
//! independently, but all of them must reduce their results to this gate before
//! a release target can be called ready.

use std::collections::BTreeMap;

use super::parity_location::ParitySourceProvenance;

mod category;
mod comparable_fact;
mod compare;
mod construct;
mod finding;
mod gpu_residency;
mod performance;
mod report;

pub use category::ParityFactCategory;
pub use comparable_fact::ParityComparableFact;
pub use compare::compare_parity_facts;
pub use construct::{ParityConstructStatus, ParityUnsupportedConstruct};
pub use finding::{ParityFinding, ParityFindingKind};
pub use gpu_residency::ParityGpuResidencyProof;
pub use performance::{ParityPerformanceProof, ParityPerformanceProofError};
pub use report::{ParityReleaseDashboard, ParityReleaseReport};
