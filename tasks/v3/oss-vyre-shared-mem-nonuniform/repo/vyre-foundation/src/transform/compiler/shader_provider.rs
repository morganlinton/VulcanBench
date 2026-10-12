//! Inventory-backed indirection for compiler-primitive shader sources.
//!
//! Law B forbids concrete target asset files under `vyre-foundation`. The
//! six compiler primitives (dataflow_fixpoint, dominator_tree,
//! recursive_descent, string_interner, typed_arena, visitor_walk) still
//! have GPU kernels, but those kernel assets live in concrete driver crates.
//!
//! This module declares the `CompilerPrimitiveShader` inventory
//! record and a fallback-free resolver. Driver crates submit records
//! through `inventory::submit!`; when a compiler primitive's CPU
//! reference needs the target-text source, it calls `source` which
//! walks the inventory and returns the registered string or `None`.

/// Inventory record pairing a compiler-primitive op id with target shader source.
///
/// Driver crates `inventory::submit!` one of these per compiler primitive.
/// The `source` fn is a plain `fn() -> &'static str` so every
/// `include_str!` lives inside the driver crate that actually ships the
/// asset.
pub struct CompilerPrimitiveShader {
    /// Stable compiler-primitive identifier, e.g. `"dominator_tree"`.
    pub op: &'static str,
    /// Emits the source string backing this primitive's GPU kernel.
    pub source: fn() -> &'static str,
}

inventory::collect!(CompilerPrimitiveShader);

/// Default provider that walks the inventory and returns the first matching
/// registration.
pub struct InventoryShaderProvider;

impl InventoryShaderProvider {
    /// Return the registered target-text source for the given compiler primitive,
    /// or `None` when no driver crate has registered one.
    #[inline]
    #[must_use]
    pub fn source(op: &str) -> Option<&'static str> {
        source(op)
    }
}

/// Resolve the target-text source for `op` using the default inventory provider.
#[must_use]
pub fn source(op: &str) -> Option<&'static str> {
    for shader in inventory::iter::<CompilerPrimitiveShader> {
        if shader.op == op {
            return Some((shader.source)());
        }
    }
    None
}
