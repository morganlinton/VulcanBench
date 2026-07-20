//! [`CFrontendWorkspaceManifest`]  -  resident manifest header that the C
//! frontend megakernel reads/writes through the workspace buffer.

use vyre_foundation::ir::{BufferDecl, DataType};

use vyre_runtime::megakernel::{
    build_workspace_regions, MegakernelWorkspaceLayoutError, MegakernelWorkspaceRegionSpec,
};

use super::error::CFrontendWorkspaceError;
use super::{
    CFrontendRegionId, CFrontendWorkspaceLimits, CFrontendWorkspaceRegion,
    C_FRONTEND_CONDITIONAL_WORDS, C_FRONTEND_DIAGNOSTIC_WORDS, C_FRONTEND_MACRO_WORDS,
    C_FRONTEND_MANIFEST_WORDS, C_FRONTEND_PG_EDGE_WORDS, C_FRONTEND_TOKEN_WORDS,
    C_FRONTEND_VAST_ROW_WORDS, C_FRONTEND_WORKSPACE_BINDING, C_FRONTEND_WORKSPACE_BUFFER,
    C_FRONTEND_WORK_QUEUE_WORDS, MAX_C_FRONTEND_WORKSPACE_WORDS,
};

/// Checked manifest for a GPU-resident C frontend workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CFrontendWorkspaceManifest {
    /// Requested capacities used to build this manifest.
    pub limits: CFrontendWorkspaceLimits,
    /// Fixed manifest/header region.
    pub manifest: CFrontendWorkspaceRegion,
    /// Resident source byte region.
    pub source_bytes: CFrontendWorkspaceRegion,
    /// Token arena region.
    pub tokens: CFrontendWorkspaceRegion,
    /// Macro arena region.
    pub macros: CFrontendWorkspaceRegion,
    /// Conditional-stack arena region.
    pub conditionals: CFrontendWorkspaceRegion,
    /// VAST row arena region.
    pub vast_rows: CFrontendWorkspaceRegion,
    /// Semantic PG edge arena region.
    pub pg_edges: CFrontendWorkspaceRegion,
    /// Diagnostic arena region.
    pub diagnostics: CFrontendWorkspaceRegion,
    /// Parser work-queue region.
    pub work_queue: CFrontendWorkspaceRegion,
    total_words: u32,
}

impl CFrontendWorkspaceManifest {
    /// Build a checked resident workspace manifest.
    ///
    /// # Errors
    ///
    /// Returns [`CFrontendWorkspaceError`] when capacities are zero, region
    /// sizing overflows, or the total workspace exceeds the ABI cap.
    pub fn new(limits: CFrontendWorkspaceLimits) -> Result<Self, CFrontendWorkspaceError> {
        validate_non_zero(limits.source_bytes, CFrontendRegionId::SourceBytes)?;
        validate_non_zero(limits.token_capacity, CFrontendRegionId::Tokens)?;
        validate_non_zero(limits.macro_capacity, CFrontendRegionId::Macros)?;
        validate_non_zero(limits.conditional_capacity, CFrontendRegionId::Conditionals)?;
        validate_non_zero(limits.vast_row_capacity, CFrontendRegionId::VastRows)?;
        validate_non_zero(limits.pg_edge_capacity, CFrontendRegionId::PgEdges)?;
        validate_non_zero(limits.diagnostic_capacity, CFrontendRegionId::Diagnostics)?;
        validate_non_zero(limits.work_queue_capacity, CFrontendRegionId::WorkQueue)?;

        let specs = [
            MegakernelWorkspaceRegionSpec::fixed(
                CFrontendRegionId::Manifest,
                C_FRONTEND_MANIFEST_WORDS,
                1,
                C_FRONTEND_MANIFEST_WORDS,
            ),
            MegakernelWorkspaceRegionSpec::fixed(
                CFrontendRegionId::SourceBytes,
                limits.source_bytes.div_ceil(4),
                1,
                limits.source_bytes,
            ),
            MegakernelWorkspaceRegionSpec::record(
                CFrontendRegionId::Tokens,
                C_FRONTEND_TOKEN_WORDS,
                limits.token_capacity,
            ),
            MegakernelWorkspaceRegionSpec::record(
                CFrontendRegionId::Macros,
                C_FRONTEND_MACRO_WORDS,
                limits.macro_capacity,
            ),
            MegakernelWorkspaceRegionSpec::record(
                CFrontendRegionId::Conditionals,
                C_FRONTEND_CONDITIONAL_WORDS,
                limits.conditional_capacity,
            ),
            MegakernelWorkspaceRegionSpec::record(
                CFrontendRegionId::VastRows,
                C_FRONTEND_VAST_ROW_WORDS,
                limits.vast_row_capacity,
            ),
            MegakernelWorkspaceRegionSpec::record(
                CFrontendRegionId::PgEdges,
                C_FRONTEND_PG_EDGE_WORDS,
                limits.pg_edge_capacity,
            ),
            MegakernelWorkspaceRegionSpec::record(
                CFrontendRegionId::Diagnostics,
                C_FRONTEND_DIAGNOSTIC_WORDS,
                limits.diagnostic_capacity,
            ),
            MegakernelWorkspaceRegionSpec::record(
                CFrontendRegionId::WorkQueue,
                C_FRONTEND_WORK_QUEUE_WORDS,
                limits.work_queue_capacity,
            ),
        ];
        let regions = build_workspace_regions(&specs).map_err(map_layout_error)?;
        let [
            manifest,
            source_bytes,
            tokens,
            macros,
            conditionals,
            vast_rows,
            pg_edges,
            diagnostics,
            work_queue,
        ] = regions
            .try_into()
            .map_err(|_| CFrontendWorkspaceError::WordOverflow {
                region: CFrontendRegionId::Manifest,
                fix: "repair C frontend workspace region specs; manifest construction must emit exactly nine regions",
            })?;
        let total_words = work_queue
            .end_words()
            .ok_or(CFrontendWorkspaceError::WordOverflow {
                region: CFrontendRegionId::WorkQueue,
                fix: "reduce C frontend work-queue capacity or shard the resident parser workspace",
            })?;
        if total_words > MAX_C_FRONTEND_WORKSPACE_WORDS {
            return Err(CFrontendWorkspaceError::WorkspaceTooLarge {
                total_words,
                max_words: MAX_C_FRONTEND_WORKSPACE_WORDS,
                fix: "reduce C frontend capacities or split translation units across multiple resident workspaces",
            });
        }

        Ok(Self {
            limits,
            manifest,
            source_bytes,
            tokens,
            macros,
            conditionals,
            vast_rows,
            pg_edges,
            diagnostics,
            work_queue,
            total_words,
        })
    }

    /// Total u32 words in the resident workspace.
    #[must_use]
    pub const fn total_words(&self) -> u32 {
        self.total_words
    }

    /// Return all regions in on-wire order.
    #[must_use]
    pub const fn regions(&self) -> [CFrontendWorkspaceRegion; 9] {
        [
            self.manifest,
            self.source_bytes,
            self.tokens,
            self.macros,
            self.conditionals,
            self.vast_rows,
            self.pg_edges,
            self.diagnostics,
            self.work_queue,
        ]
    }

    /// Build the IR buffer declaration for this resident workspace.
    #[must_use]
    pub fn buffer_decl(&self) -> BufferDecl {
        BufferDecl::read_write(
            C_FRONTEND_WORKSPACE_BUFFER,
            C_FRONTEND_WORKSPACE_BINDING,
            DataType::U32,
        )
        .with_count(self.total_words)
        .with_pipeline_live_out(true)
    }
}

fn map_layout_error(
    error: MegakernelWorkspaceLayoutError<CFrontendRegionId>,
) -> CFrontendWorkspaceError {
    match error {
        MegakernelWorkspaceLayoutError::RecordWordsOverflow { region } => {
            CFrontendWorkspaceError::WordOverflow {
                region,
                fix: "reduce C frontend arena capacity so record_words * capacity fits u32",
            }
        }
        MegakernelWorkspaceLayoutError::OffsetOverflow { region } => {
            CFrontendWorkspaceError::WordOverflow {
                region,
                fix: "reduce C frontend arena capacity so region offsets fit u32",
            }
        }
    }
}

fn validate_non_zero(
    capacity: u32,
    region: CFrontendRegionId,
) -> Result<(), CFrontendWorkspaceError> {
    if capacity == 0 {
        Err(CFrontendWorkspaceError::ZeroCapacity { region })
    } else {
        Ok(())
    }
}
