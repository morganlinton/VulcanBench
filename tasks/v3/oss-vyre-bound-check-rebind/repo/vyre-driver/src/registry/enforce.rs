//! Conformance enforcement policy.
//!
//! `EnforceGate` is the frozen contract between vyre's compile pipeline and
//! the conformance harness: it decides, given a `Program` and a proposed
//! lowering, whether the artifact is allowed to reach dispatch. The gate is
//! called once per compile, reads (never mutates) the Program, and returns a
//! structured decision that names every policy that applied.
//!
//! The default gate in `vyre-core::ops::registry::gate` verifies that every
//! op invoked by the Program is registered in the `DialectRegistry`. Downstream
//! crates compose additional gates (certificate verification, witness-domain
//! coverage, signature checks) by stacking policies via `EnforceGate::then`.

use vyre_foundation::ir::Program;

pub(crate) mod private {
    pub trait Sealed {}
}

/// Outcome of a conformance enforcement decision.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnforceVerdict {
    /// The program is allowed to reach dispatch.
    Allow,
    /// The program is blocked with a machine-readable reason.
    ///
    /// The `policy` string names the gate that produced the veto (e.g.
    /// `registry_gate`, `witness_coverage`, `certificate_signature`). The
    /// `detail` string is a human-readable message that must start with
    /// `"Fix:"`.
    Deny {
        /// Stable identifier for the gate that produced the veto.
        policy: &'static str,
        /// Human-readable reason; the prose MUST start with `Fix:` so the
        /// `check_expect_has_fix.sh` gate accepts it.
        detail: String,
    },
}

/// Frozen contract: a conformance gate that inspects a `Program` and returns
/// a structured verdict. Implementations are composed into a pipeline via
/// [`Chain`]; the full gate returns `Allow` only when every stage allows.
pub trait EnforceGate: private::Sealed + Send + Sync {
    /// Name of this gate  -  appears in verdicts and logs.
    fn name(&self) -> &'static str;

    /// Evaluate the gate against `program`. Must be pure.
    fn evaluate(&self, program: &Program) -> EnforceVerdict;
}

/// Compose two gates in series.
pub struct Chain<A, B> {
    first: A,
    second: B,
}

impl<A: EnforceGate, B: EnforceGate> private::Sealed for Chain<A, B> {}

impl<A: EnforceGate, B: EnforceGate> Chain<A, B> {
    /// Pair two gates  -  `self` runs first, `other` runs only on Allow.
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<A: EnforceGate, B: EnforceGate> EnforceGate for Chain<A, B> {
    fn name(&self) -> &'static str {
        "chain"
    }

    fn evaluate(&self, program: &Program) -> EnforceVerdict {
        match self.first.evaluate(program) {
            EnforceVerdict::Allow => self.second.evaluate(program),
            deny => deny,
        }
    }
}
