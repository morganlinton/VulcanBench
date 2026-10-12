use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::hash::{blake3_128, StableHash128};

/// Include-graph evidence model for GPU-resident preprocessing.
pub mod include_graph;
/// Token stream index helpers for resident C frontend outputs.
pub mod lex_index;
/// Decoders for semantic, syntax, ABI, and scope sections embedded in frontend objects.
pub mod object_decode;
mod object_io;
/// clang/vyrec parity report and release-gate types.
pub mod parity;
/// Normalized source-location and provenance model for parity comparison.
pub mod parity_location;
/// Source-slice minimization support for parity mismatches.
pub mod parity_minimize;
/// Differential preprocessing benchmark report model.
pub mod preprocess_benchmark;
mod raw_syntax;
/// Security-oriented indexes derived from decoded semantic ProgramGraph evidence.
pub mod security_index;
/// Function and call structure indexes derived from parser evidence.
pub mod structure_index;
/// Target, dialect, and compiler-predefine option model for the C frontend.
pub mod target;
mod word_decode;

pub use include_graph::{IncludeGraphEdge, IncludeGraphProof, IncludeGraphResidency};
pub use object_decode::{
    CObjectAbiLayout, CObjectAst, CObjectSemaScope, CObjectSemanticGraph, CObjectSymbolRef,
};
pub use parity::{
    compare_parity_facts, ParityComparableFact, ParityConstructStatus, ParityFactCategory,
    ParityFinding, ParityFindingKind, ParityGpuResidencyProof, ParityPerformanceProof,
    ParityPerformanceProofError, ParityReleaseDashboard, ParityReleaseReport,
    ParityUnsupportedConstruct,
};
pub use parity_location::{
    normalize_source_file, ParitySourcePoint, ParitySourceProvenance, ParitySourceSpan,
};
pub use parity_minimize::{
    ParityMinimizerConfig, ParityMismatchReproducer, ParitySourceFile, ParitySourceMinimizer,
};
pub use preprocess_benchmark::{
    PreprocessBenchmarkGpuCounters, PreprocessBenchmarkTranslationUnit,
    PreprocessDifferentialBenchmarkReport,
};
pub use target::{
    CCharSignedness, CCompilerPredefineProfile, CDialect, CEnvironment, CPredefineScope,
    CTargetAbi, CTargetArch, CTargetOptions,
};

/// Compiler invocation parameters passed from `vyrec`.
/// Strict separation of CLI args from the core pipeline configuration.
mod compile_options;
mod entrypoints;
mod parse_summary;
mod resident_syntax;

pub use compile_options::{CliMacroAction, VyreCompileOptions};
pub use entrypoints::{
    compile, parse_source, parse_syntax_source, parse_translation_unit,
    parse_translation_unit_bytes,
};
pub use parse_summary::CParseSummary;
pub use resident_syntax::{
    parse_prepared_resident_syntax, parse_syntax_batch_bytes, parse_syntax_bytes,
    pipeline_cache_snapshot, prepare_resident_syntax_bytes, PipelineCacheSnapshot,
    PreparedResidentSyntaxBytes, SyntaxBatchParseSummary, SyntaxParseSummary,
};
