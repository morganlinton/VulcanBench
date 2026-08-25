//! IR cleanup catalog.
//!
//! Cost-monotone-down structural simplifications: deterministic `BufferDecl`
//! order, empty Block collapse, Region body inline, singleton Region block
//! promotion, constant-condition If branch elimination, and self-assignment
//! Assign elimination.

/// Coalesce nested `If` whose outer body is exactly one inner `If` with
/// no else-arm into a single `If` whose condition is the conjunction
/// of the two (ROADMAP A23).
pub mod branch_coalesce;
/// Hoist a common observably-free `Let` prefix out of a divergent
/// `Node::If` (ROADMAP A18  -  cross-branch GVN entry point).
pub mod branch_value_hoist;
/// Sort `Program::buffers()` by `(binding, name)` for deterministic IR and
/// content-addressable cache stability.
pub mod buffer_decl_sort;
/// Drop `Node::Block(vec![])` markers from sibling sequences.
pub mod empty_block_collapse;
/// Replace `Node::If` whose condition is `LitBool(true|false)` with the
/// surviving arm wrapped in a `Node::Block`.
pub mod if_constant_branch_eliminate;
/// Drop `Node::Assign { name, value: Var(name) }` self-assignments.
pub mod noop_assign_eliminate;
/// Fuse adjacent compatible `Node::Region` pairs per the built-in
/// fusion table (ROADMAP H5  -  GEMM+activation foundation half).
pub mod region_fusion_hint;
/// Region-inline pass.
pub mod region_inline;
/// Region-inline transform engine  -  pure IR-to-IR fn body used by the
/// `RegionInlinePass` ProgramPass. Held as a sibling so callers can reuse
/// the raw transform without constructing a scheduler entry.
pub mod region_inline_engine;
/// Promote `Region { body: [Block(inner)] }` to `Region { body: inner }`.
pub mod region_promote_singleton_block;
/// Drop `Let(name, cheap_leaf)` and inline `cheap_leaf` at every use
/// site when `name` is never reassigned (ROADMAP A14  -  register-
/// pressure rematerialization).
pub mod rematerialize_cheap_let;
/// Hoist common side-effect-free branch tails after a divergent `If`.
pub mod tail_duplication;
