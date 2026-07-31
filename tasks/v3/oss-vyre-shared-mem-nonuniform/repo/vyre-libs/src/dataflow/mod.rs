//! Compatibility facade for shared dataflow soundness contracts.
//!
//! `vyre-libs::dataflow` remains as a stable import path for older consumers,
//! but platform crates must not re-export downstream analysis engines. Concrete
//! IFDS, SSA, reaching-definition, callgraph, slicing, range, and related
//! analyses live in their owning engine crates and consume these shared
//! contracts from `vyre-foundation`.

use serde::{Deserialize, Serialize};

pub use vyre_foundation::soundness::{
    validate_dynamic_pipeline, validate_dynamic_primitive, validate_pipeline, validate_primitive,
    DynamicPrimitiveSoundness, DynamicSoundnessViolation, PrecisionContract, PrimitiveSoundness,
    Soundness, SoundnessTagged, SoundnessViolation,
};

/// Shared fact-schema version for security, borrowck, and Weir/Vyre bridges.
pub const SHARED_FACT_SCHEMA_VERSION: u16 = 1;

/// Cross-engine fact families accepted by the shared dataflow schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum SharedFactKind {
    /// Attacker-controlled or analysis source fact.
    Source,
    /// Security sink fact.
    Sink,
    /// Taint/dataflow reachability fact.
    Taint,
    /// Sanitizer or kill-set fact.
    Sanitizer,
    /// Program graph edge or call/control edge fact.
    GraphEdge,
    /// Rust borrow loan fact.
    BorrowLoan,
    /// Rust origin/region fact.
    BorrowOrigin,
    /// Rust origin subset/outlives fact.
    BorrowSubset,
    /// Dominance or authorization-guard fact.
    Dominance,
    /// Numeric range or bounds fact.
    Range,
    /// Source-to-sink witness/path fact.
    Witness,
}

impl SharedFactKind {
    /// Stable wire tag used by columnar schemas and release evidence.
    #[must_use]
    pub const fn wire_tag(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Sink => "sink",
            Self::Taint => "taint",
            Self::Sanitizer => "sanitizer",
            Self::GraphEdge => "graph_edge",
            Self::BorrowLoan => "borrow_loan",
            Self::BorrowOrigin => "borrow_origin",
            Self::BorrowSubset => "borrow_subset",
            Self::Dominance => "dominance",
            Self::Range => "range",
            Self::Witness => "witness",
        }
    }
}

/// Minimal cross-engine fact header.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SharedFactHeader {
    /// Schema version, currently [`SHARED_FACT_SCHEMA_VERSION`].
    pub schema_version: u16,
    /// Producer id such as `c-c11`, `rustc-nll`, or `weir`.
    pub producer: String,
    /// Shared fact family.
    pub kind: SharedFactKind,
    /// Stable producer-local fact id.
    pub fact_id: u64,
    /// Primary subject id.
    pub subject: u64,
    /// Optional object id.
    pub object: Option<u64>,
    /// Optional auxiliary id, usually a point, edge kind, or relation id.
    pub aux: Option<u64>,
    /// Stable file id, or zero when not source-spanned.
    pub file_id: u32,
    /// Start byte offset, inclusive.
    pub start_byte: u32,
    /// End byte offset, exclusive.
    pub end_byte: u32,
    /// Soundness label for the fact.
    pub soundness: Soundness,
}

impl SharedFactHeader {
    /// Build one shared fact header.
    #[must_use]
    pub fn new(
        producer: impl Into<String>,
        kind: SharedFactKind,
        fact_id: u64,
        subject: u64,
        soundness: Soundness,
    ) -> Self {
        Self {
            schema_version: SHARED_FACT_SCHEMA_VERSION,
            producer: producer.into(),
            kind,
            fact_id,
            subject,
            object: None,
            aux: None,
            file_id: 0,
            start_byte: 0,
            end_byte: 0,
            soundness,
        }
    }

    /// Attach an object id.
    #[must_use]
    pub const fn with_object(mut self, object: u64) -> Self {
        self.object = Some(object);
        self
    }

    /// Attach an auxiliary relation id.
    #[must_use]
    pub const fn with_aux(mut self, aux: u64) -> Self {
        self.aux = Some(aux);
        self
    }

    /// Attach a byte span.
    #[must_use]
    pub const fn with_span(mut self, file_id: u32, start_byte: u32, end_byte: u32) -> Self {
        self.file_id = file_id;
        self.start_byte = start_byte;
        self.end_byte = end_byte;
        self
    }

    /// Render the stable compact header used by schema contract tests.
    ///
    /// Absent optional fields use `-` as their sentinel so that `object=None`
    /// and `object=Some(0)` produce distinct tokens (`object=-` vs `object=0`).
    /// Since Polonius origin/loan/point ids are dense `u32` starting from 0,
    /// `Some(0)` is a valid, common value and must not be conflated with absence.
    #[must_use]
    pub fn wire_header(&self) -> String {
        let object_token = self
            .object
            .map_or_else(|| "-".to_string(), |v| v.to_string());
        let aux_token = self
            .aux
            .map_or_else(|| "-".to_string(), |v| v.to_string());
        format!(
            "schema=v{};producer={};kind={};fact_id={};subject={};object={};aux={};file={};start={};end={};soundness={:?}",
            self.schema_version,
            self.producer,
            self.kind.wire_tag(),
            self.fact_id,
            self.subject,
            object_token,
            aux_token,
            self.file_id,
            self.start_byte,
            self.end_byte,
            self.soundness
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_security_source_fact_header_is_exact() {
        let header = SharedFactHeader::new("c-c11", SharedFactKind::Source, 1, 42, Soundness::Exact)
            .with_span(7, 100, 120);

        // object and aux are absent (None): wire token is "-", not "0".
        // "0" is a valid Polonius id (first interned origin/loan) and must not
        // be conflated with absence.
        assert_eq!(
            header.wire_header(),
            "schema=v1;producer=c-c11;kind=source;fact_id=1;subject=42;object=-;aux=-;file=7;start=100;end=120;soundness=Exact"
        );
    }

    #[test]
    fn rust_borrow_subset_fact_header_is_exact() {
        let header = SharedFactHeader::new(
            "rustc-nll",
            SharedFactKind::BorrowSubset,
            9,
            3,
            Soundness::Exact,
        )
        .with_object(5)
        .with_aux(11);

        assert_eq!(
            header.wire_header(),
            "schema=v1;producer=rustc-nll;kind=borrow_subset;fact_id=9;subject=3;object=5;aux=11;file=0;start=0;end=0;soundness=Exact"
        );
    }

    /// Regression: `object=None` and `object=Some(0)` must produce distinct wire
    /// tokens.  Before the fix both produced `object=0`; now they produce
    /// `object=-` and `object=0` respectively.
    #[test]
    fn wire_header_distinguishes_absent_object_from_zero_object() {
        let no_object =
            SharedFactHeader::new("rustc-nll", SharedFactKind::BorrowLoan, 1, 5, Soundness::Exact);
        let object_zero = no_object.clone().with_object(0);

        // Semantic difference must be preserved on the wire.
        assert_ne!(
            no_object.wire_header(),
            object_zero.wire_header(),
            "wire_header must distinguish object=None from object=Some(0)"
        );
        assert!(
            no_object.wire_header().contains("object=-"),
            "absent object must encode as 'object=-', got: {}",
            no_object.wire_header()
        );
        assert!(
            object_zero.wire_header().contains("object=0"),
            "object=Some(0) must encode as 'object=0', got: {}",
            object_zero.wire_header()
        );
    }

    /// Same injectivity requirement for the aux field.
    #[test]
    fn wire_header_distinguishes_absent_aux_from_zero_aux() {
        let no_aux =
            SharedFactHeader::new("rustc-nll", SharedFactKind::BorrowLoan, 2, 7, Soundness::Exact);
        let aux_zero = no_aux.clone().with_aux(0);

        assert_ne!(
            no_aux.wire_header(),
            aux_zero.wire_header(),
            "wire_header must distinguish aux=None from aux=Some(0)"
        );
        assert!(
            no_aux.wire_header().contains("aux=-"),
            "absent aux must encode as 'aux=-', got: {}",
            no_aux.wire_header()
        );
        assert!(
            aux_zero.wire_header().contains("aux=0"),
            "aux=Some(0) must encode as 'aux=0', got: {}",
            aux_zero.wire_header()
        );
    }

    #[test]
    fn weir_witness_fact_header_is_exact() {
        let header = SharedFactHeader::new("weir", SharedFactKind::Witness, 13, 21, Soundness::Exact)
            .with_object(34)
            .with_aux(55);

        assert_eq!(
            header.wire_header(),
            "schema=v1;producer=weir;kind=witness;fact_id=13;subject=21;object=34;aux=55;file=0;start=0;end=0;soundness=Exact"
        );
    }
}
