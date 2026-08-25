//! Shared Region builder.
//!
//! Every public composition in `vyre-libs` routes its produced `Vec<Node>`
//! through `wrap` so optimizer passes treat the library call as an
//! opaque unit by default. Explicit inline passes can unroll the Region
//! at lower levels of the pipeline.
//!
//! The `generator` name is load-bearing  -  it's what shows up in
//! BackendError stack traces, conform certificates, and tracing spans.
//! Every library function uses its fully-qualified path as the
//! generator name so a consumer looking at a trace can grep exactly
//! where the IR came from.

use std::sync::Arc;
use vyre::ir::{Node, Program};
use vyre_foundation::composition::mark_self_exclusive_region;
use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};

/// Wrap a list of Nodes into a single `Node::Region`.
///
/// The `generator` argument is the stable identifier consumers see in
/// errors and traces. Convention: fully-qualified module path, e.g.
/// `"vyre-libs::nn::linear"`, `"vyre-libs::crypto::fnv1a"`.
///
/// The `source_region` argument is optional caller-provided span
/// information. Library functions pass `None`; a higher-level compiler
/// that tracks source positions can construct `GeneratorRef` with
/// line + column.
#[must_use]
pub fn wrap(generator: &str, body: Vec<Node>, source_region: Option<GeneratorRef>) -> Node {
    Node::Region {
        generator: Ident::from(generator),
        source_region,
        body: Arc::new(body),
    }
}

/// Construct a Region with no source-region annotation. Convenience
/// shortcut for the common library-call case where the caller isn't
/// tracking source positions.
#[must_use]
pub fn wrap_anonymous(generator: &str, body: Vec<Node>) -> Node {
    wrap(generator, body, None)
}

/// Construct a Region whose `source_region` names the composing parent op.
#[must_use]
pub fn wrap_child(generator: &str, parent: GeneratorRef, body: Vec<Node>) -> Node {
    wrap(generator, body, Some(parent))
}

/// Clone the entry regions from `program` and mark them as children of
/// `parent_op_id` without changing their generator names.
///
/// Primitive builders already stamp their own `Node::Region.generator`.
/// Tier-3 wrappers use this helper when composing those Programs so
/// `print-composition` still shows the primitive generator while audits
/// can count the region body as parent-owned composition.
#[must_use]
pub fn reparent_program_children(program: &Program, parent_op_id: &str) -> Vec<Node> {
    let parent = GeneratorRef {
        name: parent_op_id.to_string(),
    };
    program
        .entry()
        .iter()
        .cloned()
        .map(|node| reparent_entry_node(node, &parent))
        .collect()
}

/// Wrap an already-built primitive [`Program`] under a parent op id.
///
/// This preserves the primitive child regions for composition tracing while
/// giving the higher-level library op its own stable generator boundary.
#[must_use]
pub fn tag_program(parent_op_id: &str, program: Program) -> Program {
    let generator = if program.is_non_composable_with_self() {
        mark_self_exclusive_region(parent_op_id)
    } else {
        parent_op_id.to_string()
    };
    Program::wrapped(
        program.buffers().to_vec(),
        program.workgroup_size(),
        vec![wrap_anonymous(
            &generator,
            reparent_program_children(&program, parent_op_id),
        )],
    )
    .with_non_composable_with_self(program.is_non_composable_with_self())
}

fn reparent_entry_node(node: Node, parent: &GeneratorRef) -> Node {
    match node {
        Node::Region {
            generator, body, ..
        } => Node::Region {
            generator: if generator.as_ref() == Program::ROOT_REGION_GENERATOR {
                Ident::from(format!("inline::{}", parent.name))
            } else {
                generator
            },
            source_region: Some(parent.clone()),
            body,
        },
        other => wrap(
            &format!("inline::{}", parent.name),
            vec![other],
            Some(parent.clone()),
        ),
    }
}
