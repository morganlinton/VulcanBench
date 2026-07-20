/// Fact category compared between clang and vyrec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParityFactCategory {
    /// Preprocessor facts: include graph, conditionals, macro definitions, expansions, provenance,
    /// and diagnostics.
    Preprocessor,
    /// Lexical facts: token kind, spelling, span, literal facts, line table, and expansion frame.
    Lexer,
    /// Parser facts: declarations, declarators, statements, expressions, initializers, attributes,
    /// and GNU extension structure.
    Parser,
    /// Semantic facts: scopes, symbols, references, redeclarations, types, conversions, constants,
    /// lvalue/rvalue rules, and diagnostics.
    SemanticAnalysis,
    /// ABI/layout facts: sizes, alignments, offsets, bitfields, enum representation, and function
    /// ABI evidence.
    AbiLayout,
    /// Object evidence facts: object sections, row counts, schema IDs, checksums, and decoder
    /// validation.
    ObjectEvidence,
    /// Performance facts: wall time, launches, transfers, allocations, occupancy evidence,
    /// resident-graph reuse, and megakernel queue metrics.
    Performance,
    /// GPU-residency facts: accidental host-reference escapes, intermediate readbacks,
    /// synchronization, and GPU-required test execution.
    GpuResidency,
}
