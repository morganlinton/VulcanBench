//! IR-DAG → ProgramGraph encoder.
//!
//! Walks a `Program`'s entry tree depth-first and emits the canonical
//! 5-buffer ProgramGraph CSR ABI used by every Tier 2.5 graph
//! primitive. Each visited `Node` (top-level or inside any nested
//! scope  -  `If`, `Loop`, `Block`, `Region`) gets a graph-node id in
//! prefix-DFS order; a synthetic ROOT at index 0 fans out to every
//! always-kept Node so a forward BFS reaches every Node needed for
//! observable side effects.
//!
//! Edges:
//! - `ROOT_FRONTIER`: ROOT → side-effect-rooted Node (always kept).
//! - `USE_DEF`: using Node → definer Node, resolved through a
//!   lexically-scoped chain. Branches/loops/blocks/regions push fresh
//!   scope frames; the encoder walks innermost-first when resolving a
//!   `Var(name)` reference. The Loop induction variable is bound in
//!   the body's scope to the Loop wrapper itself.
//!
//! `Return` truncates its enclosing scope: only Nodes earlier than (and
//! including) the first `Return` in any scope body are encoded. This
//! matches `eliminate_unreachable` in the existing foundation DCE
//! pass.
//!
//! The decoder mirrors this traversal exactly. `apply_live_mask` walks
//! the original tree in the same DFS prefix order (consuming graph-id
//! assignments in lockstep with the encoder) and rebuilds the
//! Program, dropping Nodes whose graph-id is not in the live mask.
//! Scope wrappers (`If`/`Loop`/`Block`/`Region`) are always live by
//! the encoder's classification, so the decoder always recurses into
//! their bodies.

use rustc_hash::FxHashMap;
use std::sync::Arc;
use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_foundation::ir::{Expr, Ident, Node, Program};

/// Canonical edge-kind bits used by the encoder.
pub mod edge_kind {
    /// Edge from the synthetic ROOT to a side-effect-rooted Node.
    pub const ROOT_FRONTIER: u32 = 1 << 0;
    /// Edge from a using Node to the most-recent definer of a name in
    /// the current lexical scope chain.
    pub const USE_DEF: u32 = 1 << 1;
}

/// Canonical node-tag bits.
pub mod node_tag {
    /// Synthetic ROOT graph node.
    pub const ROOT: u32 = 1 << 0;
    /// Side-effect-rooted: always kept by DCE.
    pub const SIDE_EFFECT: u32 = 1 << 1;
    /// `Node::Let`: conditionally kept based on use-def reachability.
    pub const LET: u32 = 1 << 2;
    /// `Node::Return` terminator (always kept).
    pub const RETURN: u32 = 1 << 3;
}

/// Errors surfaced by the encoder.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EncodeError {
    /// Reserved for future variants the encoder does not yet handle.
    /// All currently-shipped Node and Expr variants are supported.
    Unsupported(&'static str),
}

/// Optional wrapping Region peeled by the encoder. The decoder uses
/// this to re-wrap the rewritten body in the same Region shell so
/// `Program::wrapped` round-trips cleanly through encode + decode.
#[derive(Debug, Clone)]
pub struct WrappingRegion {
    /// Generator id of the wrapping Region (preserved on decode).
    pub generator: Ident,
    /// Optional source-region metadata (preserved on decode).
    pub source_region: Option<GeneratorRef>,
}

/// Result of encoding a `Program` as a ProgramGraph.
#[derive(Debug, Clone)]
pub struct EncodedProgram {
    /// Total graph nodes (1 ROOT + every Node visited by the
    /// prefix-DFS encoder, including those inside nested scopes).
    pub node_count: u32,
    /// Total edges across all kinds.
    pub edge_count: u32,
    /// Per-node kind tag (BINDING_NODES). Bits from `node_tag`.
    pub nodes: Vec<u32>,
    /// CSR row pointers (BINDING_EDGE_OFFSETS). Length `node_count + 1`.
    pub edge_offsets: Vec<u32>,
    /// CSR column array (BINDING_EDGE_TARGETS).
    pub edge_targets: Vec<u32>,
    /// Per-edge kind bitmask (BINDING_EDGE_KIND_MASK).
    pub edge_kind_mask: Vec<u32>,
    /// Per-node tag bitmask (BINDING_NODE_TAGS). Same shape as `nodes`
    /// today; later passes that need finer-grained tags split these.
    pub node_tags: Vec<u32>,
    /// If the input was a `Program::wrapped`-style program with a
    /// single top-level Region, the encoder peeled the wrapper and
    /// records it here so the decoder can put it back unchanged.
    pub wrapping: Option<WrappingRegion>,
}

/// Synthetic graph-node id assigned to ROOT.
pub const ROOT_GRAPH_ID: u32 = 0;

/// Encode a `Program` as a ProgramGraph. Peels a single top-level
/// `Node::Region` wrapper if present (the canonical
/// `Program::wrapped` shape).
pub fn encode_program(program: &Program) -> Result<EncodedProgram, EncodeError> {
    let (body, wrapping): (&[Node], Option<WrappingRegion>) = match program.entry() {
        [Node::Region {
            generator,
            source_region,
            body,
        }] => (
            body.as_slice(),
            Some(WrappingRegion {
                generator: generator.clone(),
                source_region: source_region.clone(),
            }),
        ),
        entry => (entry, None),
    };

    let mut ctx = EncoderCtx::new();
    ctx.scope_stack.push(FxHashMap::default());
    ctx.encode_scope(body)?;
    let node_count = ctx.next_graph_id;
    let nodes = ctx.nodes;
    let node_tags = ctx.node_tags;
    let edges_by_source = ctx.edges_by_source;

    // Flatten edges into CSR.
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0u32);
    for source in 0..node_count {
        for &(target, kind) in &edges_by_source[source as usize] {
            edge_targets.push(target);
            edge_kind_mask.push(kind);
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    let edge_count = edge_targets.len() as u32;

    Ok(EncodedProgram {
        node_count,
        edge_count,
        nodes,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_tags,
        wrapping,
    })
}

/// Apply a live mask (one bit per graph_id, `live[0]` = ROOT, then
/// every Node in DFS prefix order) to `program`. Walks the IR tree in
/// the same order the encoder did and drops Nodes whose graph-id is
/// not live. Scope wrappers (If/Loop/Block/Region) are always kept
/// (they were classified `SIDE_EFFECT` at encode time); their bodies
/// recurse with the same counter discipline so live-mask alignment is
/// guaranteed.
pub fn apply_live_mask(program: &Program, encoded: &EncodedProgram, live: &[bool]) -> Program {
    apply_live_predicate(program, encoded, |graph_id| {
        live.get(graph_id as usize).copied().unwrap_or(false)
    })
}

/// Apply a packed live bitset (`bit graph_id` = live) to `program`.
/// This is the zero-expansion path for GPU DCE frontiers, which are
/// already returned as u32 bitsets.
pub fn apply_live_bitset_mask(
    program: &Program,
    encoded: &EncodedProgram,
    live_words: &[u32],
) -> Program {
    apply_live_predicate(program, encoded, |graph_id| {
        let idx = graph_id as usize;
        live_words
            .get(idx / 32)
            .map(|word| (word & (1u32 << (idx % 32))) != 0)
            .unwrap_or(false)
    })
}

fn apply_live_predicate<F>(program: &Program, encoded: &EncodedProgram, is_live: F) -> Program
where
    F: Fn(u32) -> bool,
{
    let body: Vec<Node> = match (program.entry(), &encoded.wrapping) {
        ([Node::Region { body, .. }], Some(_)) => body.as_ref().clone(),
        (entry, _) => entry.to_vec(),
    };

    let mut counter = 1u32; // 0 is ROOT; per-Node ids start at 1.
    let rebuilt = decode_scope_by(&body, &mut counter, &is_live);

    let new_entry = match &encoded.wrapping {
        Some(WrappingRegion {
            generator,
            source_region,
        }) => vec![Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(rebuilt),
        }],
        None => rebuilt,
    };
    program.with_rewritten_entry(new_entry)
}

// ---- Encoder context --------------------------------------------------------

struct EncoderCtx {
    nodes: Vec<u32>,
    node_tags: Vec<u32>,
    edges_by_source: Vec<Vec<(u32, u32)>>,
    scope_stack: Vec<FxHashMap<Ident, u32>>,
    next_graph_id: u32,
}

impl EncoderCtx {
    fn new() -> Self {
        let mut ctx = Self {
            nodes: Vec::new(),
            node_tags: Vec::new(),
            edges_by_source: Vec::new(),
            scope_stack: Vec::new(),
            next_graph_id: 0,
        };
        // Allocate ROOT (id 0).
        let root = ctx.alloc_id(node_tag::ROOT);
        debug_assert_eq!(root, ROOT_GRAPH_ID);
        ctx
    }

    fn alloc_id(&mut self, tag: u32) -> u32 {
        let id = self.next_graph_id;
        self.next_graph_id += 1;
        self.nodes.push(tag);
        self.node_tags.push(tag);
        self.edges_by_source.push(Vec::new());
        id
    }

    fn add_edge(&mut self, source: u32, target: u32, kind: u32) {
        self.edges_by_source[source as usize].push((target, kind));
    }

    fn lookup(&self, name: &Ident) -> Option<u32> {
        for frame in self.scope_stack.iter().rev() {
            if let Some(&id) = frame.get(name) {
                return Some(id);
            }
        }
        None
    }

    fn bind(&mut self, name: Ident, graph_id: u32) {
        if let Some(frame) = self.scope_stack.last_mut() {
            frame.insert(name, graph_id);
        }
    }

    fn push_scope(&mut self) {
        self.scope_stack.push(FxHashMap::default());
    }

    fn pop_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn encode_scope(&mut self, body: &[Node]) -> Result<(), EncodeError> {
        let prefix_len = reachable_prefix_len(body);
        let prefix = &body[..prefix_len];
        for node in prefix {
            self.encode_node(node)?;
        }
        Ok(())
    }

    fn encode_node(&mut self, node: &Node) -> Result<(), EncodeError> {
        let tag = classify_node(node);
        let my_id = self.alloc_id(tag);

        // Root frontier edges for always-kept Nodes.
        if tag & (node_tag::SIDE_EFFECT | node_tag::RETURN) != 0 {
            self.add_edge(ROOT_GRAPH_ID, my_id, edge_kind::ROOT_FRONTIER);
        }

        // Collect Var refs from this Node's *own* expressions (excluding
        // any nested-scope bodies, which are walked by recursion below).
        let mut var_buf: Vec<Ident> = Vec::new();
        collect_node_own_var_refs(node, &mut var_buf);
        for name in &var_buf {
            if let Some(definer) = self.lookup(name) {
                self.add_edge(my_id, definer, edge_kind::USE_DEF);
            }
        }

        // Register this Node as a definer of its name (Let/Assign).
        if let Some(name) = node_definition_name(node) {
            self.bind(name.clone(), my_id);
        }

        // Recurse into nested scope bodies. Each branch / loop body /
        // block / region body gets its own scope frame so a Let inside
        // does not leak out.
        match node {
            Node::If {
                cond: _,
                then,
                otherwise,
            } => {
                self.push_scope();
                self.encode_scope(then)?;
                self.pop_scope();
                self.push_scope();
                self.encode_scope(otherwise)?;
                self.pop_scope();
            }
            Node::Loop {
                var,
                from: _,
                to: _,
                body,
            } => {
                self.push_scope();
                // The induction variable is defined at body entry.
                // Bind it to the Loop wrapper itself; any Var(var)
                // inside the body resolves to my_id, and my_id is
                // ROOT-rooted so the var is always reachable while
                // the loop is live.
                self.bind(var.clone(), my_id);
                self.encode_scope(body)?;
                self.pop_scope();
            }
            Node::Block(body) => {
                self.push_scope();
                self.encode_scope(body)?;
                self.pop_scope();
            }
            Node::Region {
                generator: _,
                source_region: _,
                body,
            } => {
                self.push_scope();
                self.encode_scope(body.as_slice())?;
                self.pop_scope();
            }
            // Leaf-at-Node-level (no nested Node bodies to walk).
            Node::Let { .. }
            | Node::Assign { .. }
            | Node::Store { .. }
            | Node::Return
            | Node::Barrier { .. }
            | Node::IndirectDispatch { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Trap { .. }
            | Node::Resume { .. }
            | Node::Opaque(_) => {}
            // Future variants  -  non-exhaustive enum.
            _ => {
                return Err(EncodeError::Unsupported(
                    "Fix: encoder encountered an unknown Node variant; \
                     extend `encode.rs` to handle it before invoking gpu_dce.",
                ))
            }
        }
        Ok(())
    }
}

// ---- Decoder ----------------------------------------------------------------

fn decode_scope_by<F>(body: &[Node], counter: &mut u32, is_live: &F) -> Vec<Node>
where
    F: Fn(u32) -> bool,
{
    let prefix_len = reachable_prefix_len(body);
    let prefix = &body[..prefix_len];
    let mut out = Vec::with_capacity(prefix.len());
    for node in prefix {
        let my_id = *counter;
        *counter += 1;
        let alive = is_live(my_id);

        match node {
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let then_rebuilt = decode_scope_by(then, counter, is_live);
                let otherwise_rebuilt = decode_scope_by(otherwise, counter, is_live);
                if alive {
                    out.push(Node::If {
                        cond: cond.clone(),
                        then: then_rebuilt,
                        otherwise: otherwise_rebuilt,
                    });
                }
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                let body_rebuilt = decode_scope_by(body, counter, is_live);
                if alive {
                    out.push(Node::Loop {
                        var: var.clone(),
                        from: from.clone(),
                        to: to.clone(),
                        body: body_rebuilt,
                    });
                }
            }
            Node::Block(body) => {
                let body_rebuilt = decode_scope_by(body, counter, is_live);
                if alive {
                    out.push(Node::Block(body_rebuilt));
                }
            }
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                let body_rebuilt = decode_scope_by(body.as_slice(), counter, is_live);
                if alive {
                    out.push(Node::Region {
                        generator: generator.clone(),
                        source_region: source_region.clone(),
                        body: Arc::new(body_rebuilt),
                    });
                }
            }
            // Leaf Nodes: no recursion needed. Live-or-not decides
            // whether the clone lands in the rebuilt scope.
            other => {
                if alive {
                    out.push(other.clone());
                }
            }
        }
    }
    out
}

// ---- Classification + helpers ----------------------------------------------

/// Length of the prefix of `entry` up to and including the first
/// `Node::Return`, or `entry.len()` if no Return is present. Mirrors
/// `eliminate_unreachable` in the foundation CPU DCE.

pub fn reachable_prefix_len(entry: &[Node]) -> usize {
    for (i, node) in entry.iter().enumerate() {
        if matches!(node, Node::Return) {
            return i + 1;
        }
    }
    entry.len()
}

fn classify_node(node: &Node) -> u32 {
    match node {
        Node::Let { .. } => node_tag::LET,
        Node::Return => node_tag::RETURN,
        // Side-effect-rooted: always kept.
        Node::Assign { .. }
        | Node::Store { .. }
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_)
        // Nested wrappers are always kept too  -  their bodies are
        // walked recursively and may contain Lets that get dropped.
        | Node::If { .. }
        | Node::Loop { .. }
        | Node::Block(_)
        | Node::Region { .. } => node_tag::SIDE_EFFECT,
        // Future variants  -  fall through as side-effect (conservative;
        // over-keeping is safe for DCE). Unknown-variant shapes also
        // surface during encoding via `EncodeError::Unsupported`.
        _ => node_tag::SIDE_EFFECT,
    }
}

fn node_definition_name(node: &Node) -> Option<&Ident> {
    match node {
        Node::Let { name, .. } | Node::Assign { name, .. } => Some(name),
        _ => None,
    }
}

/// Collect every `Expr::Var(name)` referenced inside this Node's own
/// expressions, NOT recursing into nested Node bodies (those are
/// walked by the encoder separately, with their own scope frames).
fn collect_node_own_var_refs(node: &Node, out: &mut Vec<Ident>) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => collect_expr_var_refs(value, out),
        Node::Store { index, value, .. } => {
            collect_expr_var_refs(index, out);
            collect_expr_var_refs(value, out);
        }
        Node::If { cond, .. } => collect_expr_var_refs(cond, out),
        Node::Loop { from, to, .. } => {
            collect_expr_var_refs(from, out);
            collect_expr_var_refs(to, out);
        }
        Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
            collect_expr_var_refs(offset, out);
            collect_expr_var_refs(size, out);
        }
        Node::Trap { address, .. } => collect_expr_var_refs(address, out),
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Opaque(_)
        | Node::Block(_)
        | Node::Region { .. } => {}
        // Future variants: nothing collected; the unknown-variant
        // detection in `encode_node` surfaces the gap via
        // `EncodeError::Unsupported`.
        _ => {}
    }
}

/// Walk every sub-expression and push every `Expr::Var(name)` ident
/// into `out`.
pub fn collect_expr_var_refs(expr: &Expr, out: &mut Vec<Ident>) {
    match expr {
        Expr::Var(name) => out.push(name.clone()),
        Expr::Load { index, .. } => collect_expr_var_refs(index, out),
        Expr::BinOp { left, right, .. } => {
            collect_expr_var_refs(left, out);
            collect_expr_var_refs(right, out);
        }
        Expr::UnOp { operand, .. } => collect_expr_var_refs(operand, out),
        Expr::Call { args, .. } => {
            for arg in args {
                collect_expr_var_refs(arg, out);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_expr_var_refs(cond, out);
            collect_expr_var_refs(true_val, out);
            collect_expr_var_refs(false_val, out);
        }
        Expr::Cast { value, .. } => collect_expr_var_refs(value, out),
        Expr::Fma { a, b, c } => {
            collect_expr_var_refs(a, out);
            collect_expr_var_refs(b, out);
            collect_expr_var_refs(c, out);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            collect_expr_var_refs(index, out);
            if let Some(exp) = expected {
                collect_expr_var_refs(exp, out);
            }
            collect_expr_var_refs(value, out);
        }
        Expr::SubgroupBallot { cond } => collect_expr_var_refs(cond, out),
        Expr::SubgroupShuffle { value, lane } => {
            collect_expr_var_refs(value, out);
            collect_expr_var_refs(lane, out);
        }
        Expr::SubgroupReduce { value, .. } => collect_expr_var_refs(value, out),
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
        // Future Expr variants: silently no-op. The encoder walks
        // every Node; if a future Expr variant is the only one
        // referencing a name, that name will be unresolved and
        // potentially over-kept (conservative safe direction for
        // DCE). When a new Expr variant lands, add its arm here.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::graph::program_graph::{validate_program_graph, ProgramGraphShape};

    #[test]
    fn empty_program_encodes_to_root_only() {
        let p = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
        let encoded = encode_program(&p)
            .expect("Fix: empty wrapped program must encode for optimizer substrate tests");
        assert_eq!(encoded.node_count, 1);
        assert_eq!(encoded.edge_count, 0);
        assert!(encoded.wrapping.is_some());
    }

    #[test]
    fn single_store_with_var_creates_root_and_use_def_edges() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ];
        let p = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let encoded = encode_program(&p).expect("Fix: flat optimizer program must encode");
        assert_eq!(encoded.node_count, 3);
        assert_eq!(encoded.nodes[0], node_tag::ROOT);
        assert_eq!(encoded.nodes[1], node_tag::LET);
        assert_eq!(encoded.nodes[2], node_tag::SIDE_EFFECT);
        assert_eq!(encoded.edge_count, 2);
    }

    #[test]
    fn nested_if_encodes_branches_in_separate_scopes() {
        // let outer_x = 1;
        // if c { let inner = 2 } else { let inner = 3 }
        // store buf 0 outer_x
        let entry = vec![
            Node::let_bind("outer_x", Expr::u32(1)),
            Node::If {
                cond: Expr::var("c"),
                then: vec![Node::let_bind("inner", Expr::u32(2))],
                otherwise: vec![Node::let_bind("inner", Expr::u32(3))],
            },
            Node::store("buf", Expr::u32(0), Expr::var("outer_x")),
        ];
        let p = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let encoded = encode_program(&p).expect("Fix: nested optimizer program must encode");
        // ROOT(0) + outer_let(1) + if(2) + inner_then(3) + inner_else(4) + store(5)
        assert_eq!(encoded.node_count, 6);
    }

    #[test]
    fn loop_induction_var_is_reachable_inside_body() {
        // for i in 0..10 { store buf i 0 }
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(10),
            vec![Node::store("buf", Expr::var("i"), Expr::u32(0))],
        )];
        let p = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let encoded = encode_program(&p).expect("Fix: loop optimizer program must encode");
        // ROOT(0) + loop(1) + store(2)
        assert_eq!(encoded.node_count, 3);
        // The store should have a use-def edge into the loop wrapper
        // (loop's induction var is bound to the loop itself).
        let store_outgoing_start = encoded.edge_offsets[2] as usize;
        let store_outgoing_end = encoded.edge_offsets[3] as usize;
        let mut found = false;
        for idx in store_outgoing_start..store_outgoing_end {
            if encoded.edge_targets[idx] == 1 && encoded.edge_kind_mask[idx] == edge_kind::USE_DEF {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "store inside loop must reach loop wrapper via USE_DEF"
        );
    }

    #[test]
    fn return_truncates_inside_nested_scope() {
        // if c {
        //   store buf 0 7
        //   return
        //   store buf 0 99   // dead  -  past return
        // } else {}
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::store("buf", Expr::u32(0), Expr::u32(7)),
                Node::Return,
                Node::store("buf", Expr::u32(0), Expr::u32(99)),
            ],
            otherwise: vec![],
        }];
        let p = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let encoded = encode_program(&p).expect("Fix: nested return optimizer program must encode");
        // ROOT(0) + if(1) + store(2) + return(3)  -  the post-return
        // store is truncated by reachable_prefix_len.
        assert_eq!(encoded.node_count, 4);
    }

    #[test]
    fn encoded_program_passes_canonical_validation() {
        let entry = vec![
            Node::let_bind("a", Expr::u32(1)),
            Node::let_bind("b", Expr::var("a")),
            Node::store("buf", Expr::u32(0), Expr::var("b")),
        ];
        let p = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let encoded = encode_program(&p).expect("Fix: flat optimizer program must encode");
        let shape = ProgramGraphShape::new(encoded.node_count, encoded.edge_count);
        let mut padded_targets = encoded.edge_targets.clone();
        let mut padded_kinds = encoded.edge_kind_mask.clone();
        if padded_targets.is_empty() {
            padded_targets.push(0);
            padded_kinds.push(0);
        }
        validate_program_graph(
            shape,
            &encoded.nodes,
            &encoded.edge_offsets,
            &padded_targets,
            &padded_kinds,
            &encoded.node_tags,
        )
        .expect("Fix: encoded ProgramGraph must satisfy canonical wire invariants");
    }

    #[test]
    fn live_bitset_mask_matches_bool_mask_decoder() {
        let entry = vec![
            Node::let_bind("dead", Expr::u32(1)),
            Node::let_bind("live", Expr::u32(2)),
            Node::store("buf", Expr::u32(0), Expr::var("live")),
        ];
        let p = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let encoded = encode_program(&p).expect("Fix: flat optimizer program must encode");
        assert_eq!(encoded.node_count, 4);

        let bool_live = vec![true, false, true, true];
        let bitset_live = [0b1101u32];
        let bool_out = apply_live_mask(&p, &encoded, &bool_live);
        let bitset_out = apply_live_bitset_mask(&p, &encoded, &bitset_live);

        assert_eq!(bitset_out.entry(), bool_out.entry());
        match bitset_out.entry() {
            [Node::Region { body, .. }] => assert_eq!(
                body.as_ref(),
                &vec![
                    Node::let_bind("live", Expr::u32(2)),
                    Node::store("buf", Expr::u32(0), Expr::var("live")),
                ]
            ),
            other => panic!("expected wrapped root region, got {other:?}"),
        }
    }

    #[test]
    fn shadowed_let_uses_most_recent_definer() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::let_bind("x", Expr::u32(2)),
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ];
        let p = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let encoded = encode_program(&p).expect("Fix: flat optimizer program must encode");
        // ROOT(0) + let_x_1(1) + let_x_2(2) + store(3).
        // Store's USE_DEF edge must point at let_x_2 (id 2), not let_x_1.
        let store_start = encoded.edge_offsets[3] as usize;
        let store_end = encoded.edge_offsets[4] as usize;
        let mut found_use_def_target = None;
        for idx in store_start..store_end {
            if encoded.edge_kind_mask[idx] == edge_kind::USE_DEF {
                found_use_def_target = Some(encoded.edge_targets[idx]);
            }
        }
        assert_eq!(
            found_use_def_target,
            Some(2),
            "store must use the most recent shadow of x"
        );
    }
}
