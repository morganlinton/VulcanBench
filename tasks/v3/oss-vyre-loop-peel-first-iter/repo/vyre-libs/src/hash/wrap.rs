//! Shared Tier-3 hash wrapper plumbing.
//!
//! Hash compositions in this crate keep consumer-facing op ids and scoped
//! buffer names, but executable checksum bodies live in `vyre-primitives`.
//! This module prevents every hash wrapper from rebuilding the same region and
//! buffer declaration skeleton.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

use crate::buffer_names::scoped_generic_name;

/// Immutable wrapper descriptor for Tier-3 hash compositions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HashWrapperSpec {
    op_id: &'static str,
    primitive_op_id: &'static str,
    family_prefix: &'static str,
    output_words: u32,
}

impl HashWrapperSpec {
    /// Build a wrapper descriptor for one hash composition.
    #[must_use]
    pub(crate) const fn new(
        op_id: &'static str,
        primitive_op_id: &'static str,
        family_prefix: &'static str,
        output_words: u32,
    ) -> Self {
        Self {
            op_id,
            primitive_op_id,
            family_prefix,
            output_words,
        }
    }

    /// Scope conventional `(input, out)` buffers for this hash family.
    #[must_use]
    pub(crate) fn scoped_standard_buffers(&self, input: &str, out: &str) -> (String, String) {
        (
            scoped_input_buffer(self.family_prefix, input),
            scoped_output_buffer(self.family_prefix, out),
        )
    }

    /// Scope a conventional output with op-specific legacy aliases.
    #[must_use]
    pub(crate) fn scoped_output_buffer_with_aliases(&self, out: &str, aliases: &[&str]) -> String {
        scoped_output_buffer_with_aliases(self.family_prefix, out, aliases)
    }

    /// Wrap a primitive hash program with a fixed input count.
    #[must_use]
    pub(crate) fn wrap_static_count(
        &self,
        input: &str,
        out: &str,
        n: u32,
        primitive: Program,
    ) -> Program {
        self.wrap_with_count(input, out, Some(n), primitive)
    }

    /// Wrap a primitive hash program whose input count is resolved by the primitive.
    #[must_use]
    pub(crate) fn wrap_dynamic_count(&self, input: &str, out: &str, primitive: Program) -> Program {
        self.wrap_with_count(input, out, None, primitive)
    }

    fn wrap_with_count(
        &self,
        input: &str,
        out: &str,
        static_count: Option<u32>,
        primitive: Program,
    ) -> Program {
        wrap_hash_program(
            self.op_id,
            self.primitive_op_id,
            input,
            out,
            static_count,
            self.output_words,
            primitive,
        )
    }
}

/// Scope a conventional hash input buffer name under `family_prefix`.
pub(crate) fn scoped_input_buffer(family_prefix: &str, name: &str) -> String {
    scoped_generic_name(family_prefix, "input", name, &["input"])
}

/// Scope a conventional hash output buffer name under `family_prefix`.
pub(crate) fn scoped_output_buffer(family_prefix: &str, name: &str) -> String {
    scoped_generic_name(family_prefix, "out", name, &["out", "output"])
}

/// Scope a hash output buffer with op-specific legacy aliases.
pub(crate) fn scoped_output_buffer_with_aliases(
    family_prefix: &str,
    name: &str,
    aliases: &[&str],
) -> String {
    scoped_generic_name(family_prefix, "out", name, aliases)
}

/// Wrap a primitive hash program with optional static input length and an
/// arbitrary u32 output width.
#[must_use]
pub(crate) fn wrap_hash_program(
    op_id: &'static str,
    primitive_op_id: &'static str,
    input: &str,
    out: &str,
    static_count: Option<u32>,
    output_words: u32,
    primitive: Program,
) -> Program {
    let parent = GeneratorRef {
        name: op_id.to_string(),
    };
    let input_decl = match static_count {
        Some(n) => {
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n)
        }
        None => BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32),
    };
    Program::wrapped(
        vec![
            input_decl,
            BufferDecl::output(out, 1, DataType::U32).with_count(output_words),
        ],
        primitive.workgroup_size(),
        vec![crate::region::wrap_anonymous(
            op_id,
            vec![crate::region::wrap_child(
                primitive_op_id,
                parent,
                primitive.into_entry_vec(),
            )],
        )],
    )
}
