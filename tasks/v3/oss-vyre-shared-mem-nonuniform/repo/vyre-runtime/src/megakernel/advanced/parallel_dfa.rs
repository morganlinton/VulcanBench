//! Parallel prefix-composition fragments for DFA scanning.
//!
//! Replaces scalar byte-by-byte `loop_for` ($O(N)$) with a subgroup-cooperative
//! block-stride prefix sum ($O(N/WG_SIZE)$).

use vyre_foundation::ir::{Expr, Node};

const DEFAULT_SUBGROUP_WIDTH: u32 = 32;
const ALPHABET_SIZE: u32 = 256;

/// Binding names and limits for subgroup DFA prefix composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParallelDfaBindings {
    /// Dense transition table buffer.
    pub transitions: &'static str,
    /// Haystack byte buffer.
    pub haystack: &'static str,
    /// Scratch table containing the current per-lane transition function.
    pub lane_prefix: &'static str,
    /// Scratch table used for the next prefix-composition stage.
    pub lane_next: &'static str,
    /// Output buffer receiving one state per active lane.
    pub out_state_by_lane: &'static str,
    /// Variable naming the first byte in the chunk.
    pub file_start: &'static str,
    /// Variable naming one-past-last byte in the file/chunk.
    pub file_end: &'static str,
    /// Variable naming the rule transition-table base.
    pub transition_base: &'static str,
    /// Variable naming the initial DFA state.
    pub initial_state: &'static str,
    /// Variable naming the DFA state count.
    pub state_count: &'static str,
    /// Subgroup width to compose. Must be a power of two for this fragment.
    pub subgroup_width: u32,
}

impl Default for ParallelDfaBindings {
    fn default() -> Self {
        Self {
            transitions: "transitions",
            haystack: "haystack",
            lane_prefix: "lane_prefix",
            lane_next: "lane_next",
            out_state_by_lane: "out_state_by_lane",
            file_start: "file_start",
            file_end: "file_end",
            transition_base: "transition_base",
            initial_state: "initial_state",
            state_count: "state_count",
            subgroup_width: DEFAULT_SUBGROUP_WIDTH,
        }
    }
}

/// Generate a subgroup prefix-composition DFA fragment using default names.
///
/// The caller supplies `lane_prefix` and `lane_next` scratch buffers sized at
/// `subgroup_width * state_count` `u32` entries.
#[must_use]
pub fn dfa_byte_scanner_parallel_composition() -> Vec<Node> {
    dfa_byte_scanner_parallel_composition_with(&ParallelDfaBindings::default())
}

/// Generate a concrete subgroup prefix-composition DFA fragment.
///
/// Every lane first builds the transition function for its byte, storing an
/// identity function for inactive lanes past `file_end`. The fixed doubling
/// stages then compose transition functions with subgroup shuffles and
/// workgroup barriers. The final per-lane state is written to
/// `out_state_by_lane[local_x]`.
#[must_use]
pub fn dfa_byte_scanner_parallel_composition_with(bindings: &ParallelDfaBindings) -> Vec<Node> {
    let mut nodes = vec![
        Node::let_bind("lane_id", Expr::invocation_local_x()),
        Node::let_bind(
            "lane_byte_pos",
            Expr::add(Expr::var(bindings.file_start), Expr::var("lane_id")),
        ),
        Node::let_bind(
            "lane_active",
            Expr::lt(Expr::var("lane_byte_pos"), Expr::var(bindings.file_end)),
        ),
        Node::let_bind(
            "lane_byte",
            Expr::select(
                Expr::var("lane_active"),
                Expr::cast(
                    vyre_foundation::ir::DataType::U32,
                    Expr::load(bindings.haystack, Expr::var("lane_byte_pos")),
                ),
                Expr::u32(0),
            ),
        ),
        Node::loop_for(
            "state",
            Expr::u32(0),
            Expr::var(bindings.state_count),
            vec![Node::store(
                bindings.lane_prefix,
                table_index("lane_id", bindings.state_count, Expr::var("state")),
                Expr::select(
                    Expr::var("lane_active"),
                    Expr::load(
                        bindings.transitions,
                        Expr::add(
                            Expr::var(bindings.transition_base),
                            Expr::add(
                                Expr::mul(Expr::var("state"), Expr::u32(ALPHABET_SIZE)),
                                Expr::var("lane_byte"),
                            ),
                        ),
                    ),
                    Expr::var("state"),
                ),
            )],
        ),
        Node::barrier(),
    ];

    let mut stride = 1;
    while stride < bindings.subgroup_width {
        append_prefix_stage(&mut nodes, bindings, stride);
        stride *= 2;
    }

    nodes.extend([
        Node::store(
            bindings.out_state_by_lane,
            Expr::var("lane_id"),
            Expr::load(
                bindings.lane_prefix,
                table_index(
                    "lane_id",
                    bindings.state_count,
                    Expr::var(bindings.initial_state),
                ),
            ),
        ),
        Node::barrier(),
    ]);
    nodes
}

fn append_prefix_stage(nodes: &mut Vec<Node>, bindings: &ParallelDfaBindings, stride: u32) {
    nodes.push(Node::loop_for(
        "state",
        Expr::u32(0),
        Expr::var(bindings.state_count),
        vec![
            Node::let_bind(
                "source_lane",
                Expr::select(
                    Expr::ge(Expr::var("lane_id"), Expr::u32(stride)),
                    Expr::sub(Expr::var("lane_id"), Expr::u32(stride)),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "previous_state",
                Expr::subgroup_shuffle(
                    Expr::load(
                        bindings.lane_prefix,
                        table_index("lane_id", bindings.state_count, Expr::var("state")),
                    ),
                    Expr::var("source_lane"),
                ),
            ),
            Node::let_bind(
                "composed_state",
                Expr::select(
                    Expr::ge(Expr::var("lane_id"), Expr::u32(stride)),
                    Expr::load(
                        bindings.lane_prefix,
                        table_index("lane_id", bindings.state_count, Expr::var("previous_state")),
                    ),
                    Expr::load(
                        bindings.lane_prefix,
                        table_index("lane_id", bindings.state_count, Expr::var("state")),
                    ),
                ),
            ),
            Node::store(
                bindings.lane_next,
                table_index("lane_id", bindings.state_count, Expr::var("state")),
                Expr::var("composed_state"),
            ),
        ],
    ));
    nodes.push(Node::barrier());
    nodes.push(Node::loop_for(
        "state",
        Expr::u32(0),
        Expr::var(bindings.state_count),
        vec![Node::store(
            bindings.lane_prefix,
            table_index("lane_id", bindings.state_count, Expr::var("state")),
            Expr::load(
                bindings.lane_next,
                table_index("lane_id", bindings.state_count, Expr::var("state")),
            ),
        )],
    ));
    nodes.push(Node::barrier());
}

fn table_index(lane_var: &str, state_count_var: &str, state: Expr) -> Expr {
    Expr::add(
        Expr::mul(Expr::var(lane_var), Expr::var(state_count_var)),
        state,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_dfa_fragment_has_prefix_barriers_and_output() {
        let nodes = dfa_byte_scanner_parallel_composition();
        assert!(
            nodes
                .iter()
                .filter(|node| matches!(
                    node,
                    Node::Barrier {
                        ordering: vyre::memory_model::MemoryOrdering::SeqCst
                    }
                ))
                .count()
                >= 2,
            "prefix composition must synchronize scratch-table stages"
        );
        assert!(
            stores_buffer(&nodes, "out_state_by_lane"),
            "fragment must publish per-lane states"
        );
    }

    fn stores_buffer(nodes: &[Node], name: &str) -> bool {
        nodes.iter().any(|node| match node {
            Node::Store { buffer, .. } => buffer.as_str() == name,
            Node::Block(body) | Node::Loop { body, .. } => stores_buffer(body, name),
            Node::Region { body, .. } => stores_buffer(body, name),
            Node::If {
                then, otherwise, ..
            } => stores_buffer(then, name) || stores_buffer(otherwise, name),
            _ => false,
        })
    }
}
