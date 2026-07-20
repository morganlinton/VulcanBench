#![forbid(unsafe_code)]

//! Shared conformance-runner library surface.
//!
//! The CLI, parity matrix, lens checks, and certificate regression tests all
//! depend on this crate exposing the same primitives. Keeping the root limited
//! to module declarations and deliberate re-exports prevents the runner binary
//! from becoming the hidden owner of conformance logic.

pub mod bundle_cert;
pub mod cert;
pub mod convergence_lens;
pub mod dispatch_grid;
pub mod fp_parity;
pub mod lens;
#[doc(hidden)]
pub mod witness_plan;

pub use bundle_cert::{
    issue_bundle_cert, verify_bundle_against_reference, verify_bundle_with_backend,
    verify_cert_signature_hex, BundleCertError, BundleCertificate, CorpusWitness,
};
pub use cert::{issue_certificate, verify_structural, Certificate, CertificateError, IssueInput};
