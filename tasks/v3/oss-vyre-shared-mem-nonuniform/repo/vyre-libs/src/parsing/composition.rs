use crate::region::wrap_child;
use vyre::ir::Node;
use vyre_foundation::ir::model::expr::GeneratorRef;

#[cfg_attr(
    not(any(feature = "c-parser", feature = "python-parser")),
    allow(dead_code)
)]
pub(crate) fn child_phase(parent_op_id: &str, phase_op_id: &str, body: Vec<Node>) -> Node {
    wrap_child(
        phase_op_id,
        GeneratorRef {
            name: parent_op_id.to_string(),
        },
        body,
    )
}
