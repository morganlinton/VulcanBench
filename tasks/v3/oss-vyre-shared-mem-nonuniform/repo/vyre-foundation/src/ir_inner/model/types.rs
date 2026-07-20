//! Core type definitions for the vyre IR.
//!
//! These public types are defined by `vyre-spec` so that backend
//! conformance can be proved without depending on `vyre`.
//!
//! # Examples
//!
//! ```
//! use vyre::ir::{DataType, BufferAccess, BinOp};
//!
//! // Element type for a U32 buffer
//! let elem = DataType::U32;
//!
//! // Read-write access for an output buffer
//! let access = BufferAccess::ReadWrite;
//!
//! // The arithmetic operator used inside an Expr::BinOp
//! let op = BinOp::Add;
//! ```
//!
//! # Wire Contract
//!
//! `DataType::Bool` buffer elements occupy one canonical 32-bit word in stable
//! storage payloads. `0` means false and any non-zero word means true; producers
//! must not pack multiple booleans into bitsets under the `Bool` type. The wire
//! type tag is one byte, but the value payload follows the 32-bit scalar ABI.
//! Packed-bit encodings belong in explicit integer buffers such as `Vec<u32>`.

/// Re-export of frozen IR types from `vyre-spec`.
///
/// These types are the vocabulary of every vyre program: data types,
/// buffer access modes, binary and unary operators, function calling
/// conventions, and operation signatures. Because they live in the spec
/// crate, frontends and backends can depend on them without pulling in
/// the full compiler.
pub use vyre_spec::{
    AtomicOp, BinOp, BufferAccess, CollectiveOp, CommGroup, Convention, DataType, OpSignature,
    SubgroupReduceOp, UnOp,
};
