//! Middle-end compiler pipeline on GPU IR: CFG, codegen, ABI layout.
//!
//! C11-related lowering stages live in this module; they are built from vyre
//! IR and are gated at the crate level with the `c-parser` feature (see
//! `Cargo.toml`). The public submodules are flat (`cfg`, `object_writer`, …)
//! so file paths match Rust’s usual `mod` / `use` story.
#![allow(missing_docs)]

pub(crate) mod atomic_collect;
pub mod cfg;
pub mod object_writer;
pub mod regalloc;
pub mod stack_layout;
pub mod types_layout;
