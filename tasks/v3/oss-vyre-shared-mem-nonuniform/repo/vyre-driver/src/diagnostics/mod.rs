//! Structured, machine-readable diagnostics.
//!
//! Every fallible operation in vyre eventually surfaces a failure. The
//! legacy `Error` enum carried prose (and `Fix:` hints inside formatted
//! messages). `Diagnostic` is the structured form consumed by IDEs, language
//! servers, CI annotators, and terminal renderers.

mod legacy;
mod serde_cow;
mod types;

pub use types::{Diagnostic, DiagnosticCode, OpLocation, Severity};

#[cfg(test)]
mod tests;
