//! Consumer-owned ABI glue for the GPU-resident C frontend workspace.
//!
//! C-language semantics belong in `vyre-libs`; this module owns only the
//! C translation-unit workspace contract and composes it through
//! `vyre-runtime`'s generic megakernel workspace seam.
//! GPU-resident C frontend workspace ABI for the parser megakernel.
//!
//! Module ownership:
//!  - `mod.rs`: constants + enums + handler + `manifest_word` + workspace limits/region types
//!  - `workspace.rs`: [`CFrontendWorkspaceManifest`] + impl
//!  - `nodes.rs`: builder functions emitting `Node` graphs
//!  - `error.rs`: error enum + phase-transition validators

mod error;
mod nodes;
mod workspace;

#[cfg(test)]
mod tests;

pub use error::{
    is_valid_c_frontend_phase_transition, validate_c_frontend_phase_transition,
    CFrontendWorkspaceError,
};
pub use nodes::{
    c_frontend_advance_phase_nodes, c_frontend_fault_nodes, c_frontend_phase_dispatch_nodes,
    c_frontend_phase_machine_guard_nodes, c_frontend_workspace_bootstrap_nodes,
};
pub use workspace::CFrontendWorkspaceManifest;

use vyre_foundation::ir::{Node, Program};
use vyre_runtime::megakernel::{
    build_program_sharded_with_workspace_adapter, MegakernelWorkspaceAdapter,
    MegakernelWorkspaceRegion, OpcodeHandler,
};

/// Binding used by the resident C frontend workspace.
///
/// Bindings `0..=3` are owned by the legacy megakernel control, ring,
/// debug-log, and IO queue buffers.
pub const C_FRONTEND_WORKSPACE_BINDING: u32 = 4;

/// Storage-buffer name used by C frontend megakernel IR nodes.
pub const C_FRONTEND_WORKSPACE_BUFFER: &str = "c_frontend_workspace";

/// Maximum resident workspace size accepted by the 0.6 protocol.
///
/// This caps manifest construction before a caller can create a huge static
/// buffer declaration that would be rejected later by a backend.
pub const MAX_C_FRONTEND_WORKSPACE_WORDS: u32 = 64 * 1024 * 1024;

/// Fixed manifest/header words at the start of the resident workspace.
pub const C_FRONTEND_MANIFEST_WORDS: u32 = 128;

/// Words per token arena record.
pub const C_FRONTEND_TOKEN_WORDS: u32 = 8;

/// Words per macro table record.
pub const C_FRONTEND_MACRO_WORDS: u32 = 12;

/// Words per conditional-stack record.
pub const C_FRONTEND_CONDITIONAL_WORDS: u32 = 4;

/// Words per VAST arena row.
pub const C_FRONTEND_VAST_ROW_WORDS: u32 = 8;

/// Words per semantic property-graph edge row.
pub const C_FRONTEND_PG_EDGE_WORDS: u32 = 8;

/// Words per resident diagnostic record.
pub const C_FRONTEND_DIAGNOSTIC_WORDS: u32 = 8;

/// Words per parser work-queue entry.
pub const C_FRONTEND_WORK_QUEUE_WORDS: u32 = 4;

/// Magic value written in the resident manifest header.
pub const C_FRONTEND_WORKSPACE_MAGIC: u32 = 0x5659_4346;

/// ABI version for the resident C frontend workspace.
pub const C_FRONTEND_WORKSPACE_ABI_VERSION: u32 = 1;

/// Manifest word indices reserved at the front of the workspace.
pub mod manifest_word {
    /// Magic word: [`super::C_FRONTEND_WORKSPACE_MAGIC`].
    pub const MAGIC: u32 = 0;
    /// ABI version word: [`super::C_FRONTEND_WORKSPACE_ABI_VERSION`].
    pub const ABI_VERSION: u32 = 1;
    /// Current phase id.
    pub const CURRENT_PHASE: u32 = 2;
    /// Requested next phase id.
    pub const REQUESTED_PHASE: u32 = 3;
    /// Non-zero when the megakernel has faulted the workspace.
    pub const STATUS: u32 = 4;
    /// Capacity diagnostic kind.
    pub const DIAGNOSTIC_KIND: u32 = 5;
    /// Region associated with the active capacity diagnostic.
    pub const DIAGNOSTIC_REGION: u32 = 6;
    /// Required words or records for the active capacity diagnostic.
    pub const DIAGNOSTIC_REQUIRED: u32 = 7;
    /// Available words or records for the active capacity diagnostic.
    pub const DIAGNOSTIC_CAPACITY: u32 = 8;
    /// Source byte count present in the resident source region.
    pub const SOURCE_BYTES: u32 = 9;
    /// Token count produced by the lexer.
    pub const TOKEN_COUNT: u32 = 10;
    /// Macro record count produced by directive handling.
    pub const MACRO_COUNT: u32 = 11;
    /// VAST row count produced by parsing.
    pub const VAST_ROW_COUNT: u32 = 12;
    /// PG edge count produced by semantic lowering.
    pub const PG_EDGE_COUNT: u32 = 13;
    /// Diagnostic record count produced by every phase.
    pub const DIAGNOSTIC_COUNT: u32 = 14;
    /// Work queue head cursor.
    pub const WORK_QUEUE_HEAD: u32 = 15;
    /// Work queue tail cursor.
    pub const WORK_QUEUE_TAIL: u32 = 16;
    /// Region table base. Each region occupies four words:
    /// `offset_words`, `words`, `record_words`, `capacity_records`.
    pub const REGION_TABLE_BASE: u32 = 32;
    /// Region-table words per region.
    pub const REGION_TABLE_ENTRY_WORDS: u32 = 4;
}

/// Resident C frontend phase identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CFrontendPhase {
    /// Workspace is resident and ready for the parser megakernel to claim.
    ResidentReady = 0,
    /// Source spans are normalized inside the resident workspace.
    Ingest = 1,
    /// Source bytes are lexed into token records.
    Lex = 2,
    /// Preprocessor directives are classified into resident metadata.
    DirectiveClassify = 3,
    /// Object-like and function-like macros are expanded.
    MacroExpand = 4,
    /// Conditional inclusion masks are resolved.
    ConditionalMask = 5,
    /// Identifiers are promoted to language/dialect keywords.
    KeywordPromote = 6,
    /// VAST rows are constructed.
    VastBuild = 7,
    /// Scope, label, type, and statement roles are classified.
    SemanticClassify = 8,
    /// Semantic property-graph edges are lowered.
    PgLower = 9,
    /// Resident artifacts and arena counts are validated.
    Validate = 10,
    /// The megakernel has completed the frontend path.
    Complete = 11,
    /// The megakernel detected a non-recoverable workspace fault.
    Fault = 12,
}

impl CFrontendPhase {
    /// Return the GPU-visible phase id.
    #[must_use]
    pub const fn id(self) -> u32 {
        self as u32
    }

    /// Decode a GPU-visible phase id.
    #[must_use]
    pub const fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::ResidentReady),
            1 => Some(Self::Ingest),
            2 => Some(Self::Lex),
            3 => Some(Self::DirectiveClassify),
            4 => Some(Self::MacroExpand),
            5 => Some(Self::ConditionalMask),
            6 => Some(Self::KeywordPromote),
            7 => Some(Self::VastBuild),
            8 => Some(Self::SemanticClassify),
            9 => Some(Self::PgLower),
            10 => Some(Self::Validate),
            11 => Some(Self::Complete),
            12 => Some(Self::Fault),
            _ => None,
        }
    }

    /// Return the next successful phase in the parser megakernel state machine.
    #[must_use]
    pub const fn next_success(self) -> Option<Self> {
        match self {
            Self::ResidentReady => Some(Self::Ingest),
            Self::Ingest => Some(Self::Lex),
            Self::Lex => Some(Self::DirectiveClassify),
            Self::DirectiveClassify => Some(Self::MacroExpand),
            Self::MacroExpand => Some(Self::ConditionalMask),
            Self::ConditionalMask => Some(Self::KeywordPromote),
            Self::KeywordPromote => Some(Self::VastBuild),
            Self::VastBuild => Some(Self::SemanticClassify),
            Self::SemanticClassify => Some(Self::PgLower),
            Self::PgLower => Some(Self::Validate),
            Self::Validate => Some(Self::Complete),
            Self::Complete | Self::Fault => None,
        }
    }
}

/// Resident workspace arena identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CFrontendRegionId {
    /// Fixed manifest/header region.
    Manifest = 0,
    /// Resident source bytes packed into u32 words.
    SourceBytes = 1,
    /// Lexer token arena.
    Tokens = 2,
    /// Macro definition and expansion arena.
    Macros = 3,
    /// Conditional inclusion stack arena.
    Conditionals = 4,
    /// VAST row arena.
    VastRows = 5,
    /// Semantic property-graph edge arena.
    PgEdges = 6,
    /// Diagnostic record arena.
    Diagnostics = 7,
    /// Internal parser work queue.
    WorkQueue = 8,
}

impl CFrontendRegionId {
    /// Return the GPU-visible region id.
    #[must_use]
    pub const fn id(self) -> u32 {
        self as u32
    }
}

/// Capacity diagnostic kinds written by the resident frontend path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CFrontendCapacityDiagnosticKind {
    /// No capacity diagnostic is active.
    None = 0,
    /// Workspace word layout overflowed before a manifest could be built.
    WorkspaceWords = 1,
    /// Source byte region is too small.
    SourceBytes = 2,
    /// Token arena is too small.
    Tokens = 3,
    /// Macro arena is too small.
    Macros = 4,
    /// Conditional-stack arena is too small.
    Conditionals = 5,
    /// VAST row arena is too small.
    VastRows = 6,
    /// Semantic PG edge arena is too small.
    PgEdges = 7,
    /// Diagnostic arena is too small.
    Diagnostics = 8,
    /// Internal work queue is too small.
    WorkQueue = 9,
    /// Phase transition request is illegal.
    PhaseTransition = 10,
}

impl CFrontendCapacityDiagnosticKind {
    /// Return the GPU-visible diagnostic id.
    #[must_use]
    pub const fn id(self) -> u32 {
        self as u32
    }
}

/// Resident phase handler spliced into the parser megakernel.
///
/// A handler body is GPU IR only. It may read/write the C frontend workspace,
/// publish diagnostics, and then use [`c_frontend_advance_phase_nodes`] to
/// move to the next phase. Absence of a handler leaves the phase pending; the
/// builder never fabricates parser completion.
#[derive(Debug, Clone, PartialEq)]
pub struct CFrontendPhaseHandler {
    /// Phase this handler owns.
    pub phase: CFrontendPhase,
    /// GPU IR body for this phase.
    pub body: Vec<Node>,
}

impl CFrontendPhaseHandler {
    /// Build a resident phase handler.
    #[must_use]
    pub fn new(phase: CFrontendPhase, body: Vec<Node>) -> Self {
        Self { phase, body }
    }
}

/// Capacity request used to construct the resident workspace manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CFrontendWorkspaceLimits {
    /// Resident source bytes available to the megakernel.
    pub source_bytes: u32,
    /// Maximum token records.
    pub token_capacity: u32,
    /// Maximum macro records.
    pub macro_capacity: u32,
    /// Maximum nested conditional records.
    pub conditional_capacity: u32,
    /// Maximum VAST rows.
    pub vast_row_capacity: u32,
    /// Maximum semantic PG edges.
    pub pg_edge_capacity: u32,
    /// Maximum diagnostic records.
    pub diagnostic_capacity: u32,
    /// Maximum internal work-queue entries.
    pub work_queue_capacity: u32,
}

impl CFrontendWorkspaceLimits {
    /// Conservative default capacity profile for focused tests and small TUs.
    #[must_use]
    pub const fn small_translation_unit() -> Self {
        Self {
            source_bytes: 64 * 1024,
            token_capacity: 16 * 1024,
            macro_capacity: 2 * 1024,
            conditional_capacity: 512,
            vast_row_capacity: 16 * 1024,
            pg_edge_capacity: 32 * 1024,
            diagnostic_capacity: 2 * 1024,
            work_queue_capacity: 16 * 1024,
        }
    }

    /// Build a checked resident workspace manifest.
    ///
    /// # Errors
    ///
    /// Returns [`CFrontendWorkspaceError`] when a capacity is zero, arithmetic
    /// overflows, or the total resident workspace exceeds the protocol cap.
    pub fn manifest(self) -> Result<CFrontendWorkspaceManifest, CFrontendWorkspaceError> {
        CFrontendWorkspaceManifest::new(self)
    }
}

/// One contiguous region inside the resident C frontend workspace.
pub type CFrontendWorkspaceRegion = MegakernelWorkspaceRegion<CFrontendRegionId>;

/// Runtime adapter that keeps the C frontend outside the generic megakernel
/// builder seam.
#[derive(Debug, Clone, Copy)]
pub struct CFrontendMegakernelWorkspace<'a> {
    manifest: &'a CFrontendWorkspaceManifest,
    handlers: &'a [CFrontendPhaseHandler],
}

impl<'a> CFrontendMegakernelWorkspace<'a> {
    /// Build an adapter for a resident C frontend workspace.
    #[must_use]
    pub const fn new(
        manifest: &'a CFrontendWorkspaceManifest,
        handlers: &'a [CFrontendPhaseHandler],
    ) -> Self {
        Self { manifest, handlers }
    }
}

impl MegakernelWorkspaceAdapter for CFrontendMegakernelWorkspace<'_> {
    fn buffer_decl(&self) -> vyre_foundation::ir::BufferDecl {
        self.manifest.buffer_decl()
    }

    fn bootstrap_nodes(&self) -> Vec<Node> {
        c_frontend_workspace_bootstrap_nodes(self.manifest)
    }

    fn guard_nodes(&self) -> Vec<Node> {
        c_frontend_phase_machine_guard_nodes()
    }

    fn dispatch_nodes(&self) -> Vec<Node> {
        c_frontend_phase_dispatch_nodes(self.handlers)
    }
}

/// Build the sharded megakernel IR with a resident C frontend workspace ABI.
///
/// This declares the parser workspace buffer that a self-orchestrating C
/// frontend megakernel path consumes after launch. It does not add host parser
/// semantics; language work must be implemented as megakernel IR against the
/// resident workspace.
#[must_use]
pub fn build_program_sharded_with_c_frontend_workspace(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
    manifest: &CFrontendWorkspaceManifest,
) -> Program {
    build_program_sharded_with_c_frontend_workspace_phases(
        workgroup_size_x,
        slot_count,
        opcodes,
        manifest,
        &[],
    )
}

/// Build the sharded megakernel IR with resident C frontend phase handlers.
///
/// This is the production composition point for the one-dispatch C frontend:
/// the CPU declares the resident workspace and launches the megakernel; parser
/// phases are explicit GPU IR handlers selected from manifest phase words.
#[must_use]
pub fn build_program_sharded_with_c_frontend_workspace_phases(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
    manifest: &CFrontendWorkspaceManifest,
    c_frontend_handlers: &[CFrontendPhaseHandler],
) -> Program {
    let adapter = CFrontendMegakernelWorkspace::new(manifest, c_frontend_handlers);
    build_program_sharded_with_workspace_adapter(workgroup_size_x, slot_count, opcodes, &adapter)
}
