//! Substrate-neutral kernel descriptor.
//!
//! This is the type that lives BETWEEN the optimizer and the emitters.
//! Every emitter takes a `KernelDescriptor` and produces a backend
//! artifact.
//!
//! ## Design principles
//!
//! - **Faithful to vyre IR**: embeds the same `BinOp`, `UnOp`,
//!   `AtomicOp`, `MemoryOrdering`, and `DataType` enums as the IR. No
//!   re-enumeration that would force the lowering to map "vyre IR op X"
//!   to "descriptor op Y" with a translation table; the descriptor
//!   carries the same op identity.
//! - **SSA-shaped**: every value-producing op assigns a unique 32-bit
//!   `result` id. Operands reference earlier results by id. No named
//!   variables at this layer  -  the lowering pass converts vyre IR's
//!   named bindings (`Node::Let`, `Node::Assign`, `Expr::Var`) into
//!   id references.
//! - **Structured control flow only**: `StructuredIfThen`,
//!   `StructuredIfThenElse`, `StructuredForLoop` carry indices into
//!   `KernelBody::child_bodies`. There is no goto / arbitrary jump;
//!   that's an explicit constraint required by structured compute
//!   emitters and low-level instruction emitters alike.
//! - **Substrate-neutral**: nothing in this module names any specific
//!   backend. Substrate-specific assumptions live in emitter crates.
//! - **Round-trippable**: serde-derived for every value; emitters can
//!   cache descriptors on disk.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use vyre_foundation::ir::{AtomicOp, BinOp, DataType, SubgroupReduceOp, UnOp};
use vyre_foundation::runtime::memory_model::MemoryOrdering;

pub const TRAP_SIDECAR_NAME: &str = "__vyre_descriptor_trap_sidecar";
pub const TRAP_SIDECAR_WORDS: u32 = 4;

/// Workgroup dispatch shape. `[x, y, z]` matches every modern
/// compute backend. `(1, 1, 1)` is a single invocation per workgroup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dispatch {
    pub workgroup_size: [u32; 3],
}

impl Dispatch {
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self {
            workgroup_size: [x, y, z],
        }
    }
}

/// Where a binding's storage lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryClass {
    /// Global / device memory; visible to every workgroup.
    Global,
    /// Workgroup-shared memory.
    Shared,
    /// Read-only constant memory backed by a storage buffer
    /// (`BufferDecl::storage(.., ReadOnly, ..)`). Bind in group 0
    /// alongside `Global` writers.
    Constant,
    /// True uniform-buffer memory backed by `BufferDecl::uniform`.
    /// Maps to WGSL `var<uniform>` / Vulkan `uniform_buffer` descriptor
    /// and binds in group 1 per `bind_group_for`. Distinct from
    /// `Constant` so the emitter can pick `AddressSpace::Uniform` and
    /// the layout builder can reserve the second bind group.
    Uniform,
    /// Backend-managed scratch storage.
    Scratch,
}

impl MemoryClass {
    /// True iff this memory class is visible across workgroups
    /// (Global, Constant). Shared and Scratch are workgroup-local.
    #[must_use]
    pub fn is_global_visibility(self) -> bool {
        matches!(self, Self::Global | Self::Constant)
    }

    /// True iff this memory class can be written by the kernel.
    /// Constant is read-only; the rest are writable.
    #[must_use]
    pub fn is_writable(self) -> bool {
        !matches!(self, Self::Constant)
    }
}

/// Read/write visibility for a binding slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BindingVisibility {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl BindingVisibility {
    /// True iff the binding can be read by the kernel
    /// (`ReadOnly` or `ReadWrite`).
    #[must_use]
    pub fn is_readable(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }

    /// True iff the binding can be written by the kernel
    /// (`WriteOnly` or `ReadWrite`).
    #[must_use]
    pub fn is_writable(self) -> bool {
        matches!(self, Self::WriteOnly | Self::ReadWrite)
    }
}

/// One bound buffer at the kernel boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BindingSlot {
    /// Bind-group-slot index, stable across emitters.
    pub slot: u32,
    /// Element type. Carries the full vyre IR DataType so emitters can
    /// reproduce the exact type information (lane counts, sparse
    /// layouts, etc.).
    pub element_type: DataType,
    /// Element count. `None` means runtime-sized.
    pub element_count: Option<u32>,
    pub memory_class: MemoryClass,
    pub visibility: BindingVisibility,
    /// Caller-friendly identifier (for debug; does NOT participate in
    /// kernel hashing).
    pub name: String,
}

/// Full binding layout for a kernel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BindingLayout {
    pub slots: Vec<BindingSlot>,
}

pub const DESCRIPTOR_INTENT_SCHEMA_VERSION: u32 = 1;

/// Backend-neutral scan intent attached beside a [`KernelDescriptor`].
///
/// These intents describe why descriptor regions exist, not how a backend must
/// implement them. CUDA, WGPU, Metal, SPIR-V, and CPU emitters can route from
/// these strategy classes without owning scan compiler policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DescriptorIntentKind {
    LiteralPrefilter,
    AutomataTransition,
    Verifier,
    OutputCompaction,
    RelationSeed,
    StreamingState,
}

impl DescriptorIntentKind {
    #[must_use]
    pub const fn strategy(self) -> DescriptorIntentStrategy {
        match self {
            Self::LiteralPrefilter => DescriptorIntentStrategy::Prefilter,
            Self::AutomataTransition => DescriptorIntentStrategy::Automata,
            Self::Verifier => DescriptorIntentStrategy::Verifier,
            Self::OutputCompaction => DescriptorIntentStrategy::Compaction,
            Self::RelationSeed => DescriptorIntentStrategy::RelationSeed,
            Self::StreamingState => DescriptorIntentStrategy::Streaming,
        }
    }

    const fn digest_tag(self) -> u8 {
        match self {
            Self::LiteralPrefilter => 1,
            Self::AutomataTransition => 2,
            Self::Verifier => 3,
            Self::OutputCompaction => 4,
            Self::RelationSeed => 5,
            Self::StreamingState => 6,
        }
    }
}

/// Routing class derived from [`DescriptorIntentKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DescriptorIntentStrategy {
    Prefilter,
    Automata,
    Verifier,
    Compaction,
    RelationSeed,
    Streaming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum ScanConstructIntentClass {
    Literal,
    Automata,
    Verifier,
    Derivative,
    ExternalAccelerator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ScanConstructIntentMapping {
    pub construct_id: &'static str,
    pub tier: &'static str,
    pub classes: &'static [ScanConstructIntentClass],
    pub required_intents: &'static [DescriptorIntentKind],
    pub verifier_fragment_id: Option<&'static str>,
}

impl ScanConstructIntentMapping {
    #[must_use]
    pub fn requires_verifier_fragment(self) -> bool {
        self.required_intents
            .iter()
            .any(|intent| *intent == DescriptorIntentKind::Verifier)
    }

    #[must_use]
    pub fn includes_class(self, class: ScanConstructIntentClass) -> bool {
        self.classes.iter().any(|candidate| *candidate == class)
    }
}

pub const SCAN_CONSTRUCT_INTENT_MAPPINGS: &[ScanConstructIntentMapping] = &[
    ScanConstructIntentMapping {
        construct_id: "regular_exact_core",
        tier: "supported",
        classes: &[
            ScanConstructIntentClass::Literal,
            ScanConstructIntentClass::Automata,
        ],
        required_intents: &[
            DescriptorIntentKind::LiteralPrefilter,
            DescriptorIntentKind::AutomataTransition,
            DescriptorIntentKind::OutputCompaction,
        ],
        verifier_fragment_id: None,
    },
    ScanConstructIntentMapping {
        construct_id: "unsupported_backtracking_constructs",
        tier: "rejected",
        classes: &[ScanConstructIntentClass::Verifier],
        required_intents: &[DescriptorIntentKind::Verifier],
        verifier_fragment_id: Some("unsupported-backtracking-diagnostic"),
    },
    ScanConstructIntentMapping {
        construct_id: "lookaround_prefilter_constructs",
        tier: "approximated",
        classes: &[
            ScanConstructIntentClass::Literal,
            ScanConstructIntentClass::Verifier,
        ],
        required_intents: &[
            DescriptorIntentKind::LiteralPrefilter,
            DescriptorIntentKind::Verifier,
        ],
        verifier_fragment_id: Some("lookaround-verifier"),
    },
    ScanConstructIntentMapping {
        construct_id: "hardware_rule_database_constructs",
        tier: "accelerator-only",
        classes: &[
            ScanConstructIntentClass::ExternalAccelerator,
            ScanConstructIntentClass::Verifier,
        ],
        required_intents: &[
            DescriptorIntentKind::Verifier,
            DescriptorIntentKind::OutputCompaction,
        ],
        verifier_fragment_id: Some("external-rule-database-verifier"),
    },
    ScanConstructIntentMapping {
        construct_id: "capture_extraction_constructs",
        tier: "verifier-required",
        classes: &[ScanConstructIntentClass::Verifier],
        required_intents: &[
            DescriptorIntentKind::Verifier,
            DescriptorIntentKind::OutputCompaction,
        ],
        verifier_fragment_id: Some("capture-extraction-verifier"),
    },
    ScanConstructIntentMapping {
        construct_id: "derivative_regex_constructs",
        tier: "verifier-required",
        classes: &[
            ScanConstructIntentClass::Derivative,
            ScanConstructIntentClass::Verifier,
        ],
        required_intents: &[
            DescriptorIntentKind::AutomataTransition,
            DescriptorIntentKind::Verifier,
            DescriptorIntentKind::OutputCompaction,
        ],
        verifier_fragment_id: Some("derivative-regex-verifier"),
    },
];

#[must_use]
pub fn scan_construct_intent_mapping(
    construct_id: &str,
) -> Option<&'static ScanConstructIntentMapping> {
    SCAN_CONSTRUCT_INTENT_MAPPINGS
        .iter()
        .find(|mapping| mapping.construct_id == construct_id)
}

/// One intent annotation for a descriptor region, binding, or result id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DescriptorIntent {
    pub kind: DescriptorIntentKind,
    pub binding_slot: Option<u32>,
    pub op_result: Option<u32>,
    pub stream_state_bytes: u32,
    pub relation_arity: u16,
    pub section_digest: u64,
}

impl DescriptorIntent {
    #[must_use]
    pub const fn new(kind: DescriptorIntentKind, section_digest: u64) -> Self {
        Self {
            kind,
            binding_slot: None,
            op_result: None,
            stream_state_bytes: 0,
            relation_arity: 0,
            section_digest,
        }
    }

    #[must_use]
    pub const fn with_binding_slot(mut self, slot: u32) -> Self {
        self.binding_slot = Some(slot);
        self
    }

    #[must_use]
    pub const fn with_op_result(mut self, result: u32) -> Self {
        self.op_result = Some(result);
        self
    }

    #[must_use]
    pub const fn with_stream_state_bytes(mut self, bytes: u32) -> Self {
        self.stream_state_bytes = bytes;
        self
    }

    #[must_use]
    pub const fn with_relation_arity(mut self, arity: u16) -> Self {
        self.relation_arity = arity;
        self
    }
}

/// Intent sidecar for a descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DescriptorIntentSet {
    pub schema_version: u32,
    pub intents: Vec<DescriptorIntent>,
}

impl DescriptorIntentSet {
    #[must_use]
    pub fn new(intents: Vec<DescriptorIntent>) -> Self {
        Self {
            schema_version: DESCRIPTOR_INTENT_SCHEMA_VERSION,
            intents,
        }
    }

    /// Validate intent references against a descriptor and return routing
    /// evidence for emitters and benchmark artifacts.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorIntentError`] when the sidecar is empty,
    /// version-skewed, references missing descriptor slots/results, or omits
    /// required relation/streaming payload metadata.
    pub fn validate_for_descriptor(
        &self,
        descriptor: &KernelDescriptor,
    ) -> Result<DescriptorIntentEvidence, DescriptorIntentError> {
        if self.schema_version != DESCRIPTOR_INTENT_SCHEMA_VERSION {
            return Err(DescriptorIntentError::UnsupportedSchemaVersion {
                expected: DESCRIPTOR_INTENT_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.intents.is_empty() {
            return Err(DescriptorIntentError::EmptyIntentSet);
        }

        let mut evidence = DescriptorIntentEvidence {
            schema_version: DESCRIPTOR_INTENT_SCHEMA_VERSION,
            descriptor_id: descriptor.id.clone(),
            intent_count: self.intents.len(),
            has_literal_prefilter: false,
            has_automata_transition: false,
            has_verifier: false,
            has_output_compaction: false,
            has_relation_seed: false,
            has_streaming_state: false,
            strategy_digest: stable_descriptor_intent_digest(&descriptor.id, &self.intents),
        };

        for intent in &self.intents {
            if intent.section_digest == 0 {
                return Err(DescriptorIntentError::MissingSectionDigest { kind: intent.kind });
            }
            if let Some(slot) = intent.binding_slot {
                let known = descriptor
                    .bindings
                    .slots
                    .iter()
                    .any(|binding| binding.slot == slot);
                if !known {
                    return Err(DescriptorIntentError::UnknownBindingSlot {
                        kind: intent.kind,
                        slot,
                    });
                }
            }
            if let Some(result) = intent.op_result {
                if descriptor.find_op_by_id(result).is_none() {
                    return Err(DescriptorIntentError::UnknownOpResult {
                        kind: intent.kind,
                        result,
                    });
                }
            }

            match intent.kind {
                DescriptorIntentKind::LiteralPrefilter => evidence.has_literal_prefilter = true,
                DescriptorIntentKind::AutomataTransition => {
                    evidence.has_automata_transition = true;
                }
                DescriptorIntentKind::Verifier => evidence.has_verifier = true,
                DescriptorIntentKind::OutputCompaction => evidence.has_output_compaction = true,
                DescriptorIntentKind::RelationSeed => {
                    if intent.relation_arity == 0 {
                        return Err(DescriptorIntentError::MissingRelationArity);
                    }
                    evidence.has_relation_seed = true;
                }
                DescriptorIntentKind::StreamingState => {
                    if intent.stream_state_bytes == 0 {
                        return Err(DescriptorIntentError::MissingStreamingStateBytes);
                    }
                    evidence.has_streaming_state = true;
                }
            }
        }

        Ok(evidence)
    }
}

/// Descriptor plus validated intent sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntentAnnotatedDescriptor {
    pub descriptor: KernelDescriptor,
    pub intents: DescriptorIntentSet,
}

impl IntentAnnotatedDescriptor {
    /// Construct an intent-annotated descriptor after validating intent
    /// references against the descriptor body and binding layout.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorIntentError`] when the intent set is incomplete or
    /// references descriptor state that does not exist.
    pub fn try_new(
        descriptor: KernelDescriptor,
        intents: DescriptorIntentSet,
    ) -> Result<Self, DescriptorIntentError> {
        intents.validate_for_descriptor(&descriptor)?;
        Ok(Self {
            descriptor,
            intents,
        })
    }

    /// Preserve descriptor intent through a rewrite while revalidating any
    /// referenced binding slots or result ids against the rewritten descriptor.
    ///
    /// This is the neutral seam VX-337 needs: descriptor rewrites transform
    /// descriptor shape, while scan decomposition intent remains attached until
    /// the rewrite actually invalidates one of its declared references.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorIntentError`] when the rewritten descriptor no
    /// longer contains a binding slot or result id referenced by the sidecar.
    pub fn preserve_intents_after_rewrite(
        self,
        descriptor: KernelDescriptor,
    ) -> Result<Self, DescriptorIntentError> {
        Self::try_new(descriptor, self.intents)
    }

    pub fn evidence(&self) -> Result<DescriptorIntentEvidence, DescriptorIntentError> {
        self.intents.validate_for_descriptor(&self.descriptor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DescriptorIntentEvidence {
    pub schema_version: u32,
    pub descriptor_id: String,
    pub intent_count: usize,
    pub has_literal_prefilter: bool,
    pub has_automata_transition: bool,
    pub has_verifier: bool,
    pub has_output_compaction: bool,
    pub has_relation_seed: bool,
    pub has_streaming_state: bool,
    pub strategy_digest: u64,
}

impl DescriptorIntentEvidence {
    #[must_use]
    pub const fn covers_full_scan_pipeline(&self) -> bool {
        self.schema_version == DESCRIPTOR_INTENT_SCHEMA_VERSION
            && self.intent_count > 0
            && self.has_literal_prefilter
            && self.has_automata_transition
            && self.has_verifier
            && self.has_output_compaction
            && self.has_relation_seed
            && self.has_streaming_state
            && self.strategy_digest != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DescriptorIntentError {
    UnsupportedSchemaVersion {
        expected: u32,
        actual: u32,
    },
    EmptyIntentSet,
    MissingSectionDigest {
        kind: DescriptorIntentKind,
    },
    UnknownBindingSlot {
        kind: DescriptorIntentKind,
        slot: u32,
    },
    UnknownOpResult {
        kind: DescriptorIntentKind,
        result: u32,
    },
    MissingRelationArity,
    MissingStreamingStateBytes,
}

impl std::fmt::Display for DescriptorIntentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { expected, actual } => write!(
                f,
                "descriptor intent schema version {actual} is unsupported; expected {expected}"
            ),
            Self::EmptyIntentSet => f.write_str("descriptor intent set is empty"),
            Self::MissingSectionDigest { kind } => {
                write!(f, "descriptor intent {kind:?} is missing section_digest")
            }
            Self::UnknownBindingSlot { kind, slot } => write!(
                f,
                "descriptor intent {kind:?} references unknown binding slot {slot}"
            ),
            Self::UnknownOpResult { kind, result } => write!(
                f,
                "descriptor intent {kind:?} references unknown op result {result}"
            ),
            Self::MissingRelationArity => {
                f.write_str("relation-seed descriptor intent is missing relation_arity")
            }
            Self::MissingStreamingStateBytes => {
                f.write_str("streaming-state descriptor intent is missing stream_state_bytes")
            }
        }
    }
}

impl std::error::Error for DescriptorIntentError {}

fn stable_descriptor_intent_digest(descriptor_id: &str, intents: &[DescriptorIntent]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    fn mix(mut digest: u64, byte: u8) -> u64 {
        digest ^= u64::from(byte);
        digest.wrapping_mul(FNV_PRIME)
    }

    fn mix_u64(mut digest: u64, value: u64) -> u64 {
        for byte in value.to_le_bytes() {
            digest = mix(digest, byte);
        }
        digest
    }

    let mut digest = FNV_OFFSET;
    for byte in descriptor_id.as_bytes() {
        digest = mix(digest, *byte);
    }
    for intent in intents {
        digest = mix(digest, intent.kind.digest_tag());
        digest = mix_u64(digest, u64::from(intent.binding_slot.unwrap_or(u32::MAX)));
        digest = mix_u64(digest, u64::from(intent.op_result.unwrap_or(u32::MAX)));
        digest = mix_u64(digest, u64::from(intent.stream_state_bytes));
        digest = mix_u64(digest, u64::from(intent.relation_arity));
        digest = mix_u64(digest, intent.section_digest);
    }
    if digest == 0 {
        FNV_OFFSET
    } else {
        digest
    }
}

/// A literal value that can sit in the literal pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiteralValue {
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
}

impl Eq for LiteralValue {}

impl std::hash::Hash for LiteralValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::U32(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Self::I32(v) => {
                1u8.hash(state);
                v.hash(state);
            }
            // Hash f32 by its bit pattern so NaN-with-different-payloads
            // hash distinctly. Equality uses bit pattern too via PartialEq
            // on the `==` of f32  -  note this means two NaNs are not equal,
            // which is correct for caching purposes (they CAN be different
            // NaNs).
            Self::F32(v) => {
                2u8.hash(state);
                v.to_bits().hash(state);
            }
            Self::Bool(v) => {
                3u8.hash(state);
                v.hash(state);
            }
        }
    }
}

/// Stable identifier for a named entity (variable, region label, async
/// tag, trap tag). Mirrors vyre-foundation's `Ident` shape so the
/// lowering can preserve names for diagnostics.
pub type Name = Arc<str>;

/// Matrix multiply-accumulate tile shape for descriptor-level MMA ops.
///
/// These are mathematical fragment shapes, not backend instruction names.
/// Emitters map supported shapes to their native substrate and reject shapes
/// they cannot lower.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatrixMmaShape {
    /// 16 rows × 8 columns × 16 reduction lanes.
    M16N8K16,
}

/// Element type used by a matrix MMA fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatrixMmaElement {
    F16,
    BF16,
    TF32,
    F32,
}

/// Matrix fragment layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatrixMmaLayout {
    RowMajor,
    ColMajor,
}

/// One lowered op in the kernel body. Operands are referenced by
/// 32-bit id; the id space is per-`KernelBody`. SoA-friendly: an
/// emitter walks `body.ops` linearly and looks up operand ops by id
/// when needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelOp {
    pub kind: KernelOpKind,
    /// Operand ids into the same `KernelBody.ops` (or the literal pool
    /// for `Literal*` kinds  -  see the per-kind documentation).
    pub operands: Vec<u32>,
    /// Result id this op assigns. `None` for ops with no value
    /// (stores, barriers, returns, structured-control-flow markers).
    pub result: Option<u32>,
}

impl Eq for KernelOp {}

impl std::hash::Hash for KernelOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.operands.hash(state);
        self.result.hash(state);
    }
}

impl KernelOp {
    /// Number of result ids this op defines.
    #[must_use]
    pub fn result_id_count(&self) -> u32 {
        match self.kind {
            KernelOpKind::MatrixMma { .. } => 4,
            _ => u32::from(self.result.is_some()),
        }
    }

    /// Every result id produced by this op.
    ///
    /// Most descriptor ops produce zero or one id. Matrix MMA produces a
    /// compact four-id accumulator tuple starting at `result`.
    pub fn result_ids(&self) -> impl Iterator<Item = u32> + '_ {
        let base = self.result;
        (0..self.result_id_count())
            .filter_map(move |offset| base.and_then(|id| id.checked_add(offset)))
    }
}

/// Lowered op kinds. Closed enum but covers the entire vyre IR
/// surface. Adding a new vyre IR variant requires a matching variant
/// here AND emit rules in every `vyre-emit-*` crate  -  that's the cost
/// of substrate parity.
///
/// Operand semantics are documented per variant. Reading a kind without
/// reading its operand contract gives wrong code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KernelOpKind {
    // ---------- Literals ----------
    /// Operand 0 = index into `KernelBody.literals`. Result is the
    /// literal value typed per the literal pool entry.
    Literal,
    /// Snapshot a result value. Operand 0 = source result id. Result is
    /// a fresh SSA value with the source value at this program point.
    /// This is required when a source-level `Let` captures a mutable
    /// loop carrier: aliasing the carrier result id would read the
    /// later carrier value after a subsequent `Assign`.
    Copy,
    // ---------- Variable binding (lowered from Node::Let/Assign and Expr::Var) ----------
    //
    // The lowering pass converts vyre IR's named variables into SSA
    // form. `Node::Let` becomes "the result id of the bound expression
    // is now what `Var(name)` refers to in subsequent ops". `Node::Assign`
    // becomes a fresh result id that supersedes the earlier one. Names
    // are erased at this layer; the emitter never sees them.

    // ---------- Builtins ----------
    /// `LocalInvocationId.x/y/z`. Operand 0 = axis (0/1/2) as a small
    /// inline literal (NOT a literal-pool reference  -  emit picks the
    /// builtin directly).
    LocalInvocationId,
    /// `GlobalInvocationId.x/y/z`.
    GlobalInvocationId,
    /// `WorkgroupId.x/y/z`.
    WorkgroupId,
    /// Subgroup local invocation id (a.k.a. lane id).
    SubgroupLocalId,
    /// Subgroup size.
    SubgroupSize,
    /// Current induction value for the nearest structured loop that
    /// declared this variable. Produced as the first op in that loop's
    /// child body so uses of `Expr::Var(loop_var)` remain SSA-shaped
    /// instead of resolving to the loop's lower bound.
    LoopIndex { loop_var: Name },

    /// Initialize the loop-carrier slot for `name` from the pre-loop
    /// SSA value. Emitted ONCE in the PARENT body before the
    /// `StructuredForLoop` op. Operands: `[seed_value_id]`. No result.
    /// Emitters allocate a function-scope `LocalVariable` keyed by
    /// `name` (if not already allocated) and `Store(local, seed_value)`
    /// in the parent block.
    LoopCarrierInit { name: Name },

    /// Pure read of the carrier slot for `name`. Operands: `[]`.
    /// Result: the SSA id that in-loop reads of the source-level
    /// variable resolve to. Emit semantics: `Load` from the
    /// function-local allocated by the matching `LoopCarrierInit`.
    /// Used in three places per loop: (a) once at the top of each
    /// iteration so per-iteration reads resolve to the latest stored
    /// value; (b) in the parent body AFTER the loop so post-loop
    /// readers observe the loop's final value. Without this op,
    /// `Node::Assign` inside a loop body would have no observable
    /// effect on subsequent iterations  -  name resolution would always
    /// pick the pre-loop SSA, which is baked at lowering time.
    LoopCarrier { name: Name },

    /// Loop-carried-variable write at iteration end. Operands:
    /// `[final_value_id]`. No result. Pairs with `LoopCarrier { name }`
    /// to commit the iteration's final value of `name` back to the
    /// carrier local so the next iteration (or the post-loop reader)
    /// observes it.
    LoopCarrierEnd { name: Name },

    // ---------- Buffer access ----------
    /// `load(buf, index)`. Operands: [binding_slot, index_op_id].
    /// Result is the loaded value, dtype = binding's element type.
    LoadGlobal,
    /// `load(buf, index)` for a workgroup-shared binding.
    LoadShared,
    /// `load(buf, index)` for a constant/uniform binding.
    LoadConstant,
    /// Buffer length (number of elements). Operand 0 = binding_slot
    /// inline. Result is u32.
    BufferLength,
    /// `store(buf, index, value)`. Operands: [binding_slot, index_op_id, value_op_id].
    /// Result: None.
    StoreGlobal,
    /// `store(buf, index, value)` for a workgroup-shared binding.
    StoreShared,

    // ---------- Arithmetic / logic ----------
    /// Binary op. Operands: [left_op_id, right_op_id]. Result has the
    /// dtype dictated by the operand dtypes (per vyre-spec rules).
    BinOpKind(BinOp),
    /// Unary op. Operands: `operand_op_id`. Result dtype per spec.
    UnOpKind(UnOp),

    // ---------- Composite ops ----------
    /// Fused multiply-add: `a * b + c`. Operands: [a_id, b_id, c_id].
    Fma,
    /// Matrix multiply-accumulate fragment op.
    ///
    /// Operand contract for `M16N8K16/F16/F16/F32`:
    /// `[a0,a1,a2,a3, b0,b1, c0,c1,c2,c3]`, where `a*` and `b*` are
    /// packed 16-bit fragment words and `c*` are f32 accumulators. `result`
    /// is the first of four consecutive result ids (`result..result+4`).
    /// This keeps the descriptor SSA-shaped without adding backend-specific
    /// register-fragment objects to the neutral IR.
    MatrixMma {
        shape: MatrixMmaShape,
        a_layout: MatrixMmaLayout,
        b_layout: MatrixMmaLayout,
        a_type: MatrixMmaElement,
        b_type: MatrixMmaElement,
        accum_type: MatrixMmaElement,
    },
    /// Conditional select: `if cond { true_val } else { false_val }`.
    /// Operands: [cond_id, true_val_id, false_val_id].
    Select,
    /// Type cast. Operands: `value_id`. The target dtype is on the op.
    Cast { target: DataType },
    /// Atomic op. Operands: [binding_slot, index_op_id, value_op_id]
    /// for most ops. CompareExchange variants prepend `expected_op_id`:
    /// [binding_slot, index_op_id, expected_op_id, value_op_id].
    Atomic {
        op: AtomicOp,
        ordering: MemoryOrdering,
    },

    // ---------- Subgroup ops ----------
    /// Operand 0 = bool-typed cond_op_id. Result is u32 ballot mask.
    SubgroupBallot,
    /// Operands: [value_op_id, lane_op_id]. Result has the value's dtype.
    SubgroupShuffle,
    /// Operands: [value_op_id, lane_op_id]. Broadcasts `value` from the lane
    /// named by `lane` (uniform) to every lane; result has the value's dtype.
    /// Distinct from `SubgroupShuffle` (per-lane source) — broadcast requires a
    /// uniform source lane and emits `subgroupBroadcast`.
    SubgroupBroadcast,
    /// Operand 0 = value_op_id. Reduces across the subgroup with `op`;
    /// result has the value's dtype.
    SubgroupReduce { op: SubgroupReduceOp },

    // ---------- Structured control flow ----------
    /// `if (cond) { body }`. Operands: [cond_op_id, child_body_index].
    /// `child_body_index` references `KernelBody.child_bodies`.
    /// Result: None.
    StructuredIfThen,
    /// `if (cond) { then } else { otherwise }`. Operands:
    /// [cond_op_id, then_body_index, otherwise_body_index].
    StructuredIfThenElse,
    /// `for (var = lo; var < hi; ++var) { body }`. Operands:
    /// [lo_op_id, hi_op_id, body_index]. The loop variable name is
    /// embedded on the op (preserved for debug, not for codegen).
    StructuredForLoop { loop_var: Name },
    /// Inline statement block  -  explicit grouping; semantically a
    /// no-op (body is flattened during emit). Operand 0 = body_index.
    StructuredBlock,
    /// Function/kernel return. Operands: empty. Result: None.
    Return,
    /// Memory barrier with explicit ordering.
    Barrier { ordering: MemoryOrdering },
    /// Tracing/grouping marker (vyre IR `Node::Region`). Operand 0 =
    /// body_index. Carries no execution semantics; emitters MAY pass
    /// through as a comment or annotation. SEPARATION_AUDIT S5 plans
    /// to move this to a sidecar; until then it's an op so the
    /// descriptor preserves it round-trip.
    Region { generator: Name },

    // ---------- Async ----------
    /// `cp.async`-style global-to-shared copy. Operands:
    /// [src_binding, dst_binding, offset_op_id, size_op_id].
    /// `tag` ties the load to a matching `AsyncWait`.
    AsyncLoad { tag: Name },
    /// Mirror of AsyncLoad for shared-to-global. Operands:
    /// [src_binding, dst_binding, offset_op_id, size_op_id].
    AsyncStore { tag: Name },
    /// Wait on a previously-issued AsyncLoad/Store. Operands: empty.
    AsyncWait { tag: Name },

    // ---------- Effect handlers ----------
    /// Trap into a host-side effect handler. Operands: `address_op_id`.
    Trap { tag: Name },
    /// Resume from a previously-trapped effect.
    Resume { tag: Name },

    // ---------- Indirect dispatch ----------
    /// Indirect-dispatch hint. The dispatch shape comes from
    /// `count_buffer[count_offset]`. Operand 0 = count_buffer
    /// binding_slot. Result: None.
    IndirectDispatch { count_offset: u64 },

    // ---------- Calls ----------
    /// Call into a known op-id (e.g., a vyre-primitives builder
    /// surface). Operand list is the call's args. The op_id picks the
    /// callee at emit time.
    Call { op_id: Name },

    // ---------- Extension escape hatches ----------
    /// Opaque expression extension. The extension id resolves through
    /// vyre-core's extension registry. Emitters that don't recognize
    /// the extension MUST surface an error rather than silently emit
    /// nothing.
    ///
    /// Boxed to keep the common-case `KernelOpKind` small: most ops
    /// are Literal/BinOp/Load/Store at ≤16 bytes; without boxing,
    /// every op in the `ops` Vec pays the 52-byte OpaqueExpr tax.
    OpaqueExpr(Box<OpaqueExprData>),
    /// Opaque statement-node extension.
    OpaqueNode(Box<OpaqueNodeData>),
}

/// Heap-allocated payload for [`KernelOpKind::OpaqueExpr`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]

pub struct OpaqueExprData {
    pub extension_id: u32,
    pub extension_kind: String,
    pub payload: Vec<u8>,
}

/// Heap-allocated payload for [`KernelOpKind::OpaqueNode`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpaqueNodeData {
    pub extension_kind: String,
    pub payload: Vec<u8>,
}

impl Eq for KernelOpKind {}

impl std::hash::Hash for KernelOpKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::BinOpKind(op) => op.hash(state),
            Self::UnOpKind(op) => op.hash(state),
            Self::MatrixMma {
                shape,
                a_layout,
                b_layout,
                a_type,
                b_type,
                accum_type,
            } => {
                shape.hash(state);
                a_layout.hash(state);
                b_layout.hash(state);
                a_type.hash(state);
                b_type.hash(state);
                accum_type.hash(state);
            }
            Self::Cast { target } => target.hash(state),
            Self::Atomic { op, ordering } => {
                op.hash(state);
                ordering.hash(state);
            }
            Self::StructuredForLoop { loop_var } => loop_var.hash(state),
            Self::LoopIndex { loop_var } => loop_var.hash(state),
            Self::Barrier { ordering } => ordering.hash(state),
            Self::Region { generator } => generator.hash(state),
            Self::AsyncLoad { tag }
            | Self::AsyncStore { tag }
            | Self::AsyncWait { tag }
            | Self::Trap { tag }
            | Self::Resume { tag } => tag.hash(state),
            Self::LoopCarrierInit { name }
            | Self::LoopCarrier { name }
            | Self::LoopCarrierEnd { name } => name.hash(state),
            Self::IndirectDispatch { count_offset } => count_offset.hash(state),
            Self::Call { op_id } => op_id.hash(state),
            Self::OpaqueExpr(data) => {
                data.extension_id.hash(state);
                data.extension_kind.hash(state);
                data.payload.hash(state);
            }
            Self::OpaqueNode(data) => {
                data.extension_kind.hash(state);
                data.payload.hash(state);
            }
            _ => {}
        }
    }
}

/// One kernel body. Flat op stream + child bodies for nested
/// structured control flow. The entry point is `KernelDescriptor.body`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelBody {
    pub ops: Vec<KernelOp>,
    /// Child bodies referenced by `StructuredIfThen` etc. operand
    /// indices. Indexed from 0 within this body's child_bodies vec.
    pub child_bodies: Vec<KernelBody>,
    /// Literal pool referenced by `KernelOpKind::Literal` ops.
    pub literals: Vec<LiteralValue>,
}

impl Eq for KernelBody {}

impl std::hash::Hash for KernelBody {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ops.hash(state);
        self.child_bodies.hash(state);
        for lit in &self.literals {
            lit.hash(state);
        }
    }
}

/// The full kernel descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelDescriptor {
    /// Stable kernel identifier (for caching). Computed from the
    /// content hash by `lower::lower`. Empty string until lowering
    /// assigns it.
    pub id: String,
    pub bindings: BindingLayout,
    pub dispatch: Dispatch,
    pub body: KernelBody,
}

impl Eq for KernelDescriptor {}

impl std::hash::Hash for KernelDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.bindings.hash(state);
        self.dispatch.hash(state);
        self.body.hash(state);
    }
}

impl KernelDescriptor {
    /// One-line human-readable summary. Useful for diagnostic output.
    /// Format: `"<id>: N ops, M bindings, K child bodies, L literals,
    /// dispatch [x, y, z]"`.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}: {} ops, {} bindings, {} child bodies, {} literals, dispatch {:?}",
            if self.id.is_empty() {
                "<unnamed>"
            } else {
                &self.id
            },
            self.body.ops.len(),
            self.bindings.slots.len(),
            self.body.child_bodies.len(),
            self.body.literals.len(),
            self.dispatch.workgroup_size,
        )
    }

    /// Terser alternative to [`Self::summary`]. Format: `"<id>(N ops, M bindings)"`.
    /// Useful for compact terminal output where the full summary is
    /// too noisy.
    #[must_use]
    pub fn summary_compact(&self) -> String {
        format!(
            "{}({} ops, {} bindings)",
            if self.id.is_empty() {
                "<unnamed>"
            } else {
                &self.id
            },
            self.body.ops.len(),
            self.bindings.slots.len(),
        )
    }

    /// Total op count across the parent body AND every nested child
    /// body, recursively. The parent-only `body.ops.len()` is the
    /// flat count; this is the deep count.
    #[must_use]
    pub fn total_ops(&self) -> usize {
        fn walk(b: &KernelBody) -> usize {
            b.ops.len() + b.child_bodies.iter().map(walk).sum::<usize>()
        }
        walk(&self.body)
    }

    /// Total number of bodies (the parent counts as 1, plus each
    /// nested child recursively). Useful for "how nested is this
    /// kernel?" telemetry  -  a kernel with one big flat body has
    /// `body_count() == 1`; one with deep control flow has more.
    #[must_use]
    pub fn body_count(&self) -> usize {
        fn walk(b: &KernelBody) -> usize {
            1 + b.child_bodies.iter().map(walk).sum::<usize>()
        }
        walk(&self.body)
    }

    /// Maximum nesting depth of child bodies. A flat kernel returns
    /// `0`. A kernel with one If returns `1`. An If-inside-an-If
    /// returns `2`. Useful for routing decisions (deeply-nested
    /// kernels may need a different optimization strategy).
    #[must_use]
    pub fn max_body_depth(&self) -> usize {
        fn walk(b: &KernelBody) -> usize {
            b.child_bodies
                .iter()
                .map(|c| 1 + walk(c))
                .max()
                .unwrap_or(0)
        }
        walk(&self.body)
    }

    /// Look up a body by its path (a Vec of child-body indices).
    /// Empty path returns the parent body. Each element of `path`
    /// indexes into the child_bodies of the body it descends into.
    /// Returns None if any index is out of range.
    ///
    /// Matches the `body_path` shape used by `verify::VerifyError`,
    /// so tooling can take a verify error and resolve it to the
    /// actual body the error refers to.
    #[must_use]
    pub fn body_at(&self, path: &[usize]) -> Option<&KernelBody> {
        let mut current = &self.body;
        for &idx in path {
            current = current.child_bodies.get(idx)?;
        }
        Some(current)
    }

    /// True iff the descriptor has no ops at all (no parent ops AND
    /// no ops in any child body). The dispatch geometry and bindings
    /// can still be populated  -  this only asks about op content.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total_ops() == 0
    }

    /// True iff the descriptor is pure  -  no side-effecting ops anywhere.
    /// Inverse of `has_side_effects`. Pure kernels can be safely
    /// cached by descriptor identity since they produce no observable
    /// output (the only "result" is whatever value-flow the consumer
    /// inspects, which is fully determined by the descriptor).
    #[must_use]
    pub fn is_pure(&self) -> bool {
        !self.has_side_effects()
    }

    /// Iterator over every `KernelOp` in the descriptor (parent body
    /// + every nested child body, depth-first pre-order). Useful for
    /// tooling that wants to walk all ops without writing the
    /// recursion themselves.
    pub fn ops_iter(&self) -> KernelOpsIter<'_> {
        KernelOpsIter {
            stack: vec![(&self.body, 0)],
        }
    }

    /// Find the first op anywhere in the descriptor whose `result`
    /// matches `id`. Per-body id space means an id may be reused
    /// across child bodies  -  this returns the FIRST match in DFS
    /// pre-order. For a given body's view, callers should iterate
    /// `body.ops` directly.
    #[must_use]
    pub fn find_op_by_id(&self, id: u32) -> Option<&KernelOp> {
        self.ops_iter().find(|op| op.result == Some(id))
    }

    /// Total threads per workgroup (the product of `dispatch.workgroup_size`).
    /// Saturates on overflow rather than wrapping. Useful for
    /// per-dispatch resource calculations (shared memory budget,
    /// register pressure, etc.).
    #[must_use]
    pub fn dispatch_total_threads(&self) -> u32 {
        let wg = self.dispatch.workgroup_size;
        wg[0].saturating_mul(wg[1]).saturating_mul(wg[2])
    }

    /// Return a clone of this descriptor with a new `id` field.
    /// Body, bindings, dispatch all unchanged. Useful for tooling
    /// that wants to fork a descriptor for ablation testing or
    /// versioning.
    #[must_use]
    pub fn with_id(&self, id: impl Into<String>) -> Self {
        let mut clone = self.clone();
        clone.id = id.into();
        clone
    }

    /// True iff the descriptor has at least one side-effecting op (memory
    /// write, atomic, sync/async op, barrier, control exit, indirect dispatch,
    /// call, or opaque extension). A pure descriptor with no side effects
    /// produces no observable output, so a caller is free to drop it entirely.
    ///
    /// "Side effect" here means observable-or-cross-thread: `AsyncLoad` writes
    /// shared memory other threads read, `AsyncWait`/`Barrier` are sync points,
    /// and `IndirectDispatch` reconfigures the grid — all are unsafe to drop
    /// even though none produces a global-buffer write.
    #[must_use]
    pub fn has_side_effects(&self) -> bool {
        fn walk(b: &KernelBody) -> bool {
            for op in &b.ops {
                use KernelOpKind::*;
                // EXHAUSTIVE ON PURPOSE (no `_` wildcard): a future KernelOpKind
                // with an observable or cross-thread effect must be classified
                // here, never silently default to "droppable".
                let effecting = match op.kind {
                    StoreGlobal
                    | StoreShared
                    | LoopCarrierInit { .. }
                    | LoopCarrierEnd { .. }
                    | Atomic { .. }
                    | AsyncLoad { .. }
                    | AsyncStore { .. }
                    | AsyncWait { .. }
                    | Barrier { .. }
                    | Trap { .. }
                    | Resume { .. }
                    | Return
                    | IndirectDispatch { .. }
                    | Call { .. }
                    | OpaqueExpr(..)
                    | OpaqueNode(..) => true,
                    // Structured control flow / Region carry no direct effect:
                    // their child bodies are walked separately below.
                    StructuredIfThen
                    | StructuredIfThenElse
                    | StructuredForLoop { .. }
                    | StructuredBlock
                    | Region { .. } => false,
                    // Pure value/builtin/arith ops and READS (loads, carrier
                    // read, buffer length): no observable or cross-thread effect.
                    Literal
                    | Copy
                    | LocalInvocationId
                    | GlobalInvocationId
                    | WorkgroupId
                    | SubgroupLocalId
                    | SubgroupSize
                    | LoopIndex { .. }
                    | LoopCarrier { .. }
                    | LoadGlobal
                    | LoadShared
                    | LoadConstant
                    | BufferLength
                    | BinOpKind(_)
                    | UnOpKind(_)
                    | Fma
                    | MatrixMma { .. }
                    | Select
                    | Cast { .. }
                    | SubgroupBallot
                    | SubgroupShuffle
                    | SubgroupBroadcast
                    | SubgroupReduce { .. } => false,
                };
                if effecting {
                    return true;
                }
            }
            b.child_bodies.iter().any(walk)
        }
        walk(&self.body)
    }
}

/// Iterator returned by [`KernelDescriptor::ops_iter`].
pub struct KernelOpsIter<'a> {
    /// Stack of (body, next_op_index) frames. Pushed as we descend
    /// into child bodies; popped when a body is exhausted.
    stack: Vec<(&'a KernelBody, usize)>,
}

impl<'a> Iterator for KernelOpsIter<'a> {
    type Item = &'a KernelOp;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (body, idx) = self.stack.last_mut()?;
            if let Some(op) = body.ops.get(*idx) {
                *idx += 1;
                return Some(op);
            }
            // Body exhausted  -  push children and pop self.
            let body = *body;
            self.stack.pop();
            for child in body.child_bodies.iter().rev() {
                self.stack.push((child, 0));
            }
        }
    }
}

#[cfg(test)]
mod desc_helper_tests {
    use super::*;
    use vyre_foundation::ir::DataType;

    fn build(ops: Vec<KernelOp>, child_bodies: Vec<KernelBody>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops,
                child_bodies,
                literals: vec![LiteralValue::U32(7)],
            },
        }
    }

    fn literal_op() -> KernelOp {
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        }
    }

    fn full_scan_intents() -> DescriptorIntentSet {
        DescriptorIntentSet::new(vec![
            DescriptorIntent::new(DescriptorIntentKind::LiteralPrefilter, 11)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::AutomataTransition, 12)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::Verifier, 13)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::OutputCompaction, 14)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::RelationSeed, 15)
                .with_binding_slot(0)
                .with_relation_arity(2),
            DescriptorIntent::new(DescriptorIntentKind::StreamingState, 16)
                .with_binding_slot(0)
                .with_stream_state_bytes(64),
        ])
    }

    #[test]
    fn descriptor_intents_cover_scan_pipeline_and_route_to_strategies() {
        let d = build(vec![literal_op()], vec![]);
        let intents = full_scan_intents();
        let evidence = intents.validate_for_descriptor(&d).unwrap();

        assert!(evidence.covers_full_scan_pipeline());
        assert_ne!(evidence.strategy_digest, 0);
        assert_eq!(
            DescriptorIntentKind::LiteralPrefilter.strategy(),
            DescriptorIntentStrategy::Prefilter
        );
        assert_eq!(
            DescriptorIntentKind::AutomataTransition.strategy(),
            DescriptorIntentStrategy::Automata
        );
        assert_eq!(
            DescriptorIntentKind::Verifier.strategy(),
            DescriptorIntentStrategy::Verifier
        );
        assert_eq!(
            DescriptorIntentKind::OutputCompaction.strategy(),
            DescriptorIntentStrategy::Compaction
        );
        assert_eq!(
            DescriptorIntentKind::RelationSeed.strategy(),
            DescriptorIntentStrategy::RelationSeed
        );
        assert_eq!(
            DescriptorIntentKind::StreamingState.strategy(),
            DescriptorIntentStrategy::Streaming
        );
    }

    #[test]
    fn scan_construct_intent_mappings_cover_tiers_and_verifier_fragments() {
        let mut tiers = std::collections::BTreeSet::new();
        let mut classes = std::collections::BTreeSet::new();
        for mapping in SCAN_CONSTRUCT_INTENT_MAPPINGS {
            assert!(
                !mapping.required_intents.is_empty(),
                "Fix: scan construct `{}` must map to at least one descriptor intent",
                mapping.construct_id
            );
            tiers.insert(mapping.tier);
            for class in mapping.classes {
                classes.insert(*class);
            }
            if mapping.requires_verifier_fragment() {
                assert!(
                    mapping.verifier_fragment_id.is_some(),
                    "Fix: verifier-required scan construct `{}` must name a verifier fragment",
                    mapping.construct_id
                );
                assert!(
                    mapping
                        .required_intents
                        .contains(&DescriptorIntentKind::Verifier),
                    "Fix: verifier-required scan construct `{}` must require DescriptorIntentKind::Verifier",
                    mapping.construct_id
                );
            }
        }

        for tier in [
            "supported",
            "rejected",
            "approximated",
            "accelerator-only",
            "verifier-required",
        ] {
            assert!(
                tiers.contains(tier),
                "Fix: scan construct intent mappings must cover tier `{tier}`"
            );
        }
        for class in [
            ScanConstructIntentClass::Literal,
            ScanConstructIntentClass::Automata,
            ScanConstructIntentClass::Verifier,
            ScanConstructIntentClass::Derivative,
            ScanConstructIntentClass::ExternalAccelerator,
        ] {
            assert!(
                classes.contains(&class),
                "Fix: scan construct intent mappings must cover class `{class:?}`"
            );
        }
    }

    #[test]
    fn scan_construct_intent_lookup_returns_shared_contract_rows() {
        let lookaround = scan_construct_intent_mapping("lookaround_prefilter_constructs")
            .expect("Fix: lookaround construct mapping must exist");
        assert!(lookaround.includes_class(ScanConstructIntentClass::Literal));
        assert!(lookaround.includes_class(ScanConstructIntentClass::Verifier));
        assert_eq!(lookaround.verifier_fragment_id, Some("lookaround-verifier"));

        let derivative = scan_construct_intent_mapping("derivative_regex_constructs")
            .expect("Fix: derivative construct mapping must exist");
        assert!(derivative.includes_class(ScanConstructIntentClass::Derivative));
        assert!(derivative
            .required_intents
            .contains(&DescriptorIntentKind::AutomataTransition));

        assert!(scan_construct_intent_mapping("unknown_construct").is_none());
    }

    #[test]
    fn descriptor_intents_survive_rewrites_through_envelope() {
        let d = build(vec![literal_op()], vec![]);
        let intents = full_scan_intents();
        let envelope = IntentAnnotatedDescriptor::try_new(d.clone(), intents.clone()).unwrap();
        let rewritten = d.with_id("k.rewritten");

        let preserved = envelope
            .preserve_intents_after_rewrite(rewritten)
            .unwrap();

        assert_eq!(preserved.intents, intents);
        assert_eq!(preserved.descriptor.id, "k.rewritten");
        assert!(preserved.evidence().unwrap().covers_full_scan_pipeline());
    }

    #[test]
    fn descriptor_intents_reject_invalid_references_and_payloads() {
        let d = build(vec![literal_op()], vec![]);
        let missing_slot = DescriptorIntentSet::new(vec![
            DescriptorIntent::new(DescriptorIntentKind::Verifier, 17).with_binding_slot(99),
        ]);
        assert_eq!(
            missing_slot.validate_for_descriptor(&d).unwrap_err(),
            DescriptorIntentError::UnknownBindingSlot {
                kind: DescriptorIntentKind::Verifier,
                slot: 99
            }
        );

        let missing_relation_arity = DescriptorIntentSet::new(vec![DescriptorIntent::new(
            DescriptorIntentKind::RelationSeed,
            18,
        )
        .with_binding_slot(0)]);
        assert_eq!(
            missing_relation_arity
                .validate_for_descriptor(&d)
                .unwrap_err(),
            DescriptorIntentError::MissingRelationArity
        );

        let missing_stream_bytes = DescriptorIntentSet::new(vec![DescriptorIntent::new(
            DescriptorIntentKind::StreamingState,
            19,
        )
        .with_binding_slot(0)]);
        assert_eq!(
            missing_stream_bytes.validate_for_descriptor(&d).unwrap_err(),
            DescriptorIntentError::MissingStreamingStateBytes
        );
    }

    #[test]
    fn summary_includes_all_counts() {
        let d = build(vec![], vec![]);
        let s = d.summary();
        assert!(s.contains("k:"));
        assert!(s.contains("0 ops"));
        assert!(s.contains("1 bindings"));
        assert!(s.contains("0 child bodies"));
        assert!(s.contains("1 literals"));
        assert!(s.contains("[64, 1, 1]"));
    }

    #[test]
    fn summary_compact_terser_form() {
        let d = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![],
        );
        let s = d.summary_compact();
        assert_eq!(s, "k(1 ops, 1 bindings)");
    }

    #[test]
    fn unnamed_descriptor_uses_unnamed_label() {
        let mut d = build(vec![], vec![]);
        d.id = String::new();
        let s = d.summary();
        assert!(s.contains("<unnamed>"));
    }

    #[test]
    fn total_ops_recurses_into_child_bodies() {
        let child = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(5)],
        };
        let parent_ops = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        }];
        let d = build(parent_ops, vec![child]);
        assert_eq!(d.body.ops.len(), 1); // shallow
        assert_eq!(d.total_ops(), 3); // 1 parent + 2 child
    }

    #[test]
    fn body_at_empty_path_returns_parent() {
        let d = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(7),
            }],
            vec![],
        );
        let body = d.body_at(&[]).unwrap();
        assert_eq!(body.ops.len(), 1);
        assert_eq!(body.ops[0].result, Some(7));
    }

    #[test]
    fn body_at_descends_into_children() {
        let grandchild = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(99),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        };
        let child = KernelBody {
            ops: vec![],
            child_bodies: vec![grandchild],
            literals: vec![],
        };
        let d = build(vec![], vec![child]);
        // Path [0]: first child of parent  -  empty body with one grandchild.
        let b = d.body_at(&[0]).unwrap();
        assert!(b.ops.is_empty());
        // Path [0, 0]: grandchild  -  has the Literal with result 99.
        let b = d.body_at(&[0, 0]).unwrap();
        assert_eq!(b.ops[0].result, Some(99));
    }

    #[test]
    fn body_at_out_of_range_returns_none() {
        let d = build(vec![], vec![]);
        assert!(d.body_at(&[5]).is_none());
        assert!(d.body_at(&[0, 0]).is_none());
    }

    #[test]
    fn body_count_includes_parent_plus_recursive_children() {
        let nested = KernelBody {
            ops: vec![],
            child_bodies: vec![KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![],
        };
        let d = build(vec![], vec![nested]);
        // Parent (1) + first child (1) + grandchild (1) = 3.
        assert_eq!(d.body_count(), 3);
    }

    #[test]
    fn body_count_flat_kernel_is_one() {
        let d = build(vec![], vec![]);
        assert_eq!(d.body_count(), 1);
    }

    #[test]
    fn max_body_depth_flat_is_zero() {
        let d = build(vec![], vec![]);
        assert_eq!(d.max_body_depth(), 0);
    }

    #[test]
    fn max_body_depth_one_if_is_one() {
        let child = KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        };
        let d = build(vec![], vec![child]);
        assert_eq!(d.max_body_depth(), 1);
    }

    #[test]
    fn max_body_depth_two_levels() {
        let grandchild = KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        };
        let child = KernelBody {
            ops: vec![],
            child_bodies: vec![grandchild],
            literals: vec![],
        };
        let d = build(vec![], vec![child]);
        assert_eq!(d.max_body_depth(), 2);
    }

    #[test]
    fn total_ops_zero_for_empty_kernel() {
        let d = build(vec![], vec![]);
        assert_eq!(d.total_ops(), 0);
    }

    #[test]
    fn is_empty_true_when_no_ops() {
        let d = build(vec![], vec![]);
        assert!(d.is_empty());
    }

    #[test]
    fn is_empty_false_when_parent_has_ops() {
        let d = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![],
        );
        assert!(!d.is_empty());
        assert_eq!(d.total_ops(), 1);
    }

    #[test]
    fn is_empty_false_when_child_has_ops() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(1)],
        };
        let d = build(vec![], vec![child]);
        assert!(!d.is_empty());
        assert_eq!(d.total_ops(), 1);
    }

    #[test]
    fn has_side_effects_true_with_store() {
        let d = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 0],
                    result: None,
                },
            ],
            vec![],
        );
        assert!(d.has_side_effects());
    }

    #[test]
    fn has_side_effects_false_with_only_arithmetic() {
        let d = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                    operands: vec![0, 0],
                    result: Some(1),
                },
            ],
            vec![],
        );
        assert!(!d.has_side_effects());
    }

    #[test]
    fn has_side_effects_true_for_async_and_indirect_dispatch_ops() {
        // Regression: AsyncLoad writes shared memory other threads read,
        // AsyncWait is a sync point, and IndirectDispatch reconfigures the grid
        // — all cross-thread/dispatch effects (like the already-listed Barrier /
        // AsyncStore), so a descriptor containing one is NOT droppable. They
        // were omitted from the side-effecting set before the exhaustive-match
        // change, which would have let a "drop pure descriptor" caller drop one.
        for kind in [
            KernelOpKind::AsyncLoad { tag: "t".into() },
            KernelOpKind::AsyncWait { tag: "t".into() },
            KernelOpKind::IndirectDispatch { count_offset: 0 },
        ] {
            let d = build(
                vec![KernelOp {
                    kind: kind.clone(),
                    operands: vec![0],
                    result: None,
                }],
                vec![],
            );
            assert!(
                d.has_side_effects(),
                "{kind:?} is a cross-thread/dispatch effect and must not be droppable"
            );
            assert!(!d.is_pure(), "{kind:?} must not be classified pure");
        }
    }

    #[test]
    fn ops_iter_visits_parent_then_children_in_order() {
        let child0 = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(11),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(1)],
        };
        let child1 = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(20),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(2)],
        };
        let d = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            vec![child0, child1],
        );
        let visited: Vec<u32> = d.ops_iter().map(|o| o.result.unwrap()).collect();
        // Parent ops (0, 1) first, then child0 (10, 11), then child1 (20).
        assert_eq!(visited, vec![0, 1, 10, 11, 20]);
    }

    #[test]
    fn ops_iter_count_matches_total_ops() {
        let child = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        };
        let d = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![child],
        );
        assert_eq!(d.ops_iter().count(), d.total_ops());
    }

    #[test]
    fn memory_class_predicates() {
        assert!(MemoryClass::Global.is_global_visibility());
        assert!(MemoryClass::Constant.is_global_visibility());
        assert!(!MemoryClass::Shared.is_global_visibility());
        assert!(!MemoryClass::Scratch.is_global_visibility());

        assert!(MemoryClass::Global.is_writable());
        assert!(MemoryClass::Shared.is_writable());
        assert!(MemoryClass::Scratch.is_writable());
        assert!(!MemoryClass::Constant.is_writable());
    }

    #[test]
    fn binding_visibility_readable_writable() {
        assert!(BindingVisibility::ReadOnly.is_readable());
        assert!(!BindingVisibility::ReadOnly.is_writable());
        assert!(!BindingVisibility::WriteOnly.is_readable());
        assert!(BindingVisibility::WriteOnly.is_writable());
        assert!(BindingVisibility::ReadWrite.is_readable());
        assert!(BindingVisibility::ReadWrite.is_writable());
    }

    #[test]
    fn dispatch_total_threads_multiplies_dims() {
        let d = build(vec![], vec![]);
        assert_eq!(d.dispatch_total_threads(), 64); // build() uses Dispatch::new(64, 1, 1)

        let mut d2 = build(vec![], vec![]);
        d2.dispatch = Dispatch::new(8, 8, 4);
        assert_eq!(d2.dispatch_total_threads(), 256);
    }

    #[test]
    fn with_id_preserves_everything_else() {
        let d = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![],
        );
        let renamed = d.with_id("renamed");
        assert_eq!(renamed.id, "renamed");
        assert_eq!(d.id, "k"); // original unchanged
        assert_eq!(renamed.body.ops.len(), d.body.ops.len());
        assert_eq!(renamed.bindings, d.bindings);
        assert_eq!(renamed.dispatch, d.dispatch);
    }

    #[test]
    fn dispatch_total_threads_saturates_on_overflow() {
        let mut d = build(vec![], vec![]);
        d.dispatch = Dispatch::new(u32::MAX, u32::MAX, u32::MAX);
        // Saturating multiplication means we get u32::MAX rather than wrap.
        assert_eq!(d.dispatch_total_threads(), u32::MAX);
    }

    #[test]
    fn find_op_by_id_in_parent() {
        let d = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(42),
                },
            ],
            vec![],
        );
        let op = d.find_op_by_id(42).expect("Fix: found");
        assert_eq!(op.result, Some(42));
        assert!(d.find_op_by_id(99).is_none());
    }

    #[test]
    fn find_op_by_id_finds_in_child() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(100),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        };
        let d = build(vec![], vec![child]);
        assert!(d.find_op_by_id(100).is_some());
    }

    #[test]
    fn ops_iter_empty_descriptor_yields_none() {
        let d = build(vec![], vec![]);
        assert!(d.ops_iter().next().is_none());
    }

    #[test]
    fn is_pure_inverse_of_has_side_effects() {
        let pure_kernel = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![],
        );
        assert!(pure_kernel.is_pure());
        assert!(!pure_kernel.has_side_effects());

        let impure = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 0],
                    result: None,
                },
            ],
            vec![],
        );
        assert!(!impure.is_pure());
        assert!(impure.has_side_effects());
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    fn binding(slot: u32, element: DataType, mc: MemoryClass) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: element,
            element_count: None,
            memory_class: mc,
            visibility: BindingVisibility::ReadWrite,
            name: format!("b{slot}"),
        }
    }

    #[test]
    fn empty_descriptor_round_trips_serde_byte_stable() {
        let k = KernelDescriptor {
            id: "test".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let json1 = serde_json::to_string(&k).unwrap();
        let parsed: KernelDescriptor = serde_json::from_str(&json1).unwrap();
        let json2 = serde_json::to_string(&parsed).unwrap();
        assert_eq!(json1, json2);
        assert_eq!(k, parsed);
    }

    #[test]
    fn one_store_kernel_round_trips_byte_stable() {
        let k = KernelDescriptor {
            id: "store_one".into(),
            bindings: BindingLayout {
                slots: vec![binding(0, DataType::U32, MemoryClass::Global)],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let json1 = serde_json::to_string(&k).unwrap();
        let parsed: KernelDescriptor = serde_json::from_str(&json1).unwrap();
        let json2 = serde_json::to_string(&parsed).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn binop_kind_carries_full_vyre_spec_op() {
        let op = KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::SaturatingAdd),
            operands: vec![0, 1],
            result: Some(2),
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: KernelOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, parsed);
        // Confirm the variant survives  -  serde_json round-trip preserves it.
        match parsed.kind {
            KernelOpKind::BinOpKind(BinOp::SaturatingAdd) => {}
            other => panic!("lost BinOp variant: {other:?}"),
        }
    }

    #[test]
    fn unop_kind_carries_full_vyre_spec_op() {
        let op = KernelOp {
            kind: KernelOpKind::UnOpKind(UnOp::InverseSqrt),
            operands: vec![5],
            result: Some(6),
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: KernelOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, parsed);
        match parsed.kind {
            KernelOpKind::UnOpKind(UnOp::InverseSqrt) => {}
            other => panic!("lost UnOp variant: {other:?}"),
        }
    }

    #[test]
    fn atomic_carries_op_and_ordering() {
        let op = KernelOp {
            kind: KernelOpKind::Atomic {
                op: AtomicOp::CompareExchange,
                ordering: MemoryOrdering::AcqRel,
            },
            operands: vec![0, 1, 2, 3],
            result: Some(4),
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: KernelOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, parsed);
    }

    #[test]
    fn nested_if_then_body_round_trips() {
        let inner = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Barrier {
                    ordering: MemoryOrdering::SeqCst,
                },
                operands: vec![],
                result: None,
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let outer = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0],
                    result: None,
                },
            ],
            child_bodies: vec![inner],
            literals: vec![LiteralValue::Bool(true)],
        };
        let k = KernelDescriptor {
            id: "if_then".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: outer,
        };
        let json1 = serde_json::to_string(&k).unwrap();
        let parsed: KernelDescriptor = serde_json::from_str(&json1).unwrap();
        let json2 = serde_json::to_string(&parsed).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn for_loop_with_var_name_round_trips() {
        let body = KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        };
        let outer = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 0],
                    result: None,
                },
            ],
            child_bodies: vec![body],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(64)],
        };
        let k = KernelDescriptor {
            id: "for_i".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: outer,
        };
        let json = serde_json::to_string(&k).unwrap();
        let parsed: KernelDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(k, parsed);
    }

    #[test]
    fn async_load_wait_carry_tag() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::AsyncLoad {
                        tag: "chunk-0".into(),
                    },
                    operands: vec![0, 1, 0, 1],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::AsyncWait {
                        tag: "chunk-0".into(),
                    },
                    operands: vec![],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(16)],
        };
        let k = KernelDescriptor {
            id: "async".into(),
            bindings: BindingLayout {
                slots: vec![
                    binding(0, DataType::U32, MemoryClass::Global),
                    binding(1, DataType::U32, MemoryClass::Shared),
                ],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body,
        };
        let json = serde_json::to_string(&k).unwrap();
        let parsed: KernelDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(k, parsed);
    }

    #[test]
    fn cast_op_preserves_target_dtype() {
        let op = KernelOp {
            kind: KernelOpKind::Cast {
                target: DataType::F16,
            },
            operands: vec![3],
            result: Some(4),
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: KernelOp = serde_json::from_str(&json).unwrap();
        match parsed.kind {
            KernelOpKind::Cast {
                target: DataType::F16,
            } => {}
            other => panic!("lost cast target: {other:?}"),
        }
    }

    #[test]
    fn binding_carries_full_data_type() {
        // Confirm a parametric DataType (Vec) round-trips through binding.
        let b = BindingSlot {
            slot: 5,
            element_type: DataType::Vec {
                element: Box::new(DataType::F32),
                count: 4,
            },
            element_count: Some(64),
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "v4f32".into(),
        };
        let json = serde_json::to_string(&b).unwrap();
        let parsed: BindingSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(b, parsed);
    }

    #[test]
    fn literal_value_eq_treats_nan_as_distinct_via_bits() {
        let nan1 = LiteralValue::F32(f32::NAN);
        let nan2 = LiteralValue::F32(f32::NAN);
        // PartialEq for f32 treats NaN as not equal to itself; our derive
        // inherits that, so two NaNs are never equal.
        assert_ne!(nan1, nan2);
    }

    #[test]
    fn region_op_round_trips_with_generator_name() {
        let op = KernelOp {
            kind: KernelOpKind::Region {
                generator: "vyre.libs.nn.gqa_attention".into(),
            },
            operands: vec![0],
            result: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: KernelOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, parsed);
    }

    #[test]
    fn dispatch_constructor_preserves_axes() {
        let d = Dispatch::new(64, 4, 2);
        assert_eq!(d.workgroup_size, [64, 4, 2]);
    }
}
