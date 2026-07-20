//! Core IR data model.
//!
//! This module defines the pure Rust data structures that make up a vyre
//! program: expressions (`Expr`), statements (`Node`), buffer declarations
//! (`BufferDecl`), and the root `Program` container. These types are
//! serializable, validateable, and backend-agnostic.

/// Expression nodes that produce values.
///
/// `Expr` covers literals, variable references, arithmetic, comparisons,
/// buffer loads, and operation calls. Every expression has a statically
/// known type.
pub mod expr;

/// Opt-in bump arena for high-volume expression builders.
pub mod arena;

pub mod generated;
/// Statement nodes that execute effects.
///
/// `Node` covers variable declarations, assignments, control flow
/// (if/else, loops), and buffer stores. A `Program` is essentially a
/// sequence of nodes.
pub mod node;

/// Open node kind trait and built-in node structs.
pub mod node_kind;

/// Program structure and metadata.
///
/// `Program` is the top-level IR container. It holds buffer declarations,
/// the entry node list, and optional optimization hints.
pub mod program;

/// Core type definitions.
///
/// Re-exports frozen types from `vyre-spec`.
pub mod types;
