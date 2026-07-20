#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Procedural macros for the [`vyre`](https://docs.rs/vyre) GPU compute IR
//! compiler.
//!
//! This crate is compile-time only. Downstream users import from
//! `vyre::optimizer::vyre_pass` rather than depending on this crate directly.
//!
//! The macro surface includes op registration, AST registry generation,
//! pass registration, builder-field skipping, and algebraic-law derivation.

mod algebraic_laws;
mod ast_registry;
mod define_op;
mod parse_helpers;
mod pass;

use proc_macro::TokenStream;

/// Function-like `define_op!`  -  single-site op registration via inventory.
///
/// See [`define_op`](define_op/index.html) for the full argument contract.
#[proc_macro]
pub fn define_op(item: TokenStream) -> TokenStream {
    define_op::define_op_impl(item)
}

/// Generates the declarative IR AST core plus serialization and visitor traits.
#[proc_macro]
pub fn vyre_ast_registry(item: TokenStream) -> TokenStream {
    ast_registry::vyre_ast_registry_impl(item)
}

/// Marker attribute used by `vyre_ast_registry!` to skip a builder field.
#[proc_macro_attribute]
pub fn skip_builder(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Register a unit struct as a `vyre::optimizer::ProgramPass`.
#[proc_macro_attribute]
pub fn vyre_pass(args: TokenStream, item: TokenStream) -> TokenStream {
    pass::vyre_pass_impl(args, item)
}

/// Derive `vyre::AlgebraicLawProvider` from a `#[vyre(laws = [...])]` attribute.
#[proc_macro_derive(AlgebraicLaws, attributes(vyre))]
pub fn derive_algebraic_laws(item: TokenStream) -> TokenStream {
    algebraic_laws::derive_algebraic_laws_impl(item)
}

#[cfg(test)]
mod tests;
