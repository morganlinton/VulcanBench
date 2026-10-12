//! Structural C11 parser passes.

/// Declaration specifier + declarator extraction.
pub mod declarations;
mod gnu_builtin_catalog;
/// GNU builtin recognition pass.
pub mod gnu_builtins;
/// `asm` / `__asm__` inline-assembly extraction.
pub mod inline_asm;
/// Function / struct / enum structural pass.
pub mod structure;
/// GPU statement-bound extraction used by AST windowing.
pub mod structure_statement;
/// Token stream to packed VAST rows.
pub mod vast;
mod vast_kinds;
