//! Subgroup-cooperative NFA scan.
//!
//! Composes [`vyre_primitives::nfa::subgroup_nfa::nfa_step`] semantics
//! into a full scan loop that walks an input byte stream, advances
//! NFA state across bytes, and emits `(pattern_id, start, end)` hits
//! into `hit_buf` whenever an accept state fires.
//!
//! # Encoding (matches `subgroup_nfa`)
//!
//! - `state_word` (per-lane u32): bits of the active-state set this
//!   lane owns. Lane `k` holds states `k*32 .. k*32+32`.
//! - `nfa_transition` (ReadOnly, u32): lane-major
//!   `[num_states × 256 × LANES_PER_SUBGROUP]`. Entry
//!   `trans[src * 256 * LANES + byte * LANES + lane]` is the u32 of
//!   destination bits that lane `lane` is responsible for, reached
//!   from state `src` on byte `byte`. Lane-major layout is required
//!   by [`subgroup_nfa::nfa_step`]; the composition must not diverge
//!   from the primitive's contract (VYRE_MEM_LAYOUT CRITICAL-2).
//! - `nfa_epsilon` (ReadOnly, u32): lane-major
//!   `[num_states × LANES_PER_SUBGROUP]`. All zero for literal-only
//!   pattern sets.
//!
//! # Current literal-only scope
//!
//! This module supports byte-literal pattern NFAs. Regex syntax belongs
//! in a grammar-to-NFA compiler layer that produces the same transition
//! and epsilon tables before calling this scan kernel.

use std::sync::Arc;

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};

use vyre_primitives::nfa::subgroup_nfa::{LANES_PER_SUBGROUP, MAX_STATES_PER_SUBGROUP};

/// Canonical op id for the end-to-end scan kernel.
pub const OP_ID: &str = "vyre-libs::matching::nfa_scan";

/// Compile a set of patterns into a scan Program.
///
/// See module docs for buffer encoding. Hit buffer layout is
/// `[counter, p0, s0, e0, p1, s1, e1, …]`  -  slot 0 is an atomic
/// counter, each match does `atomic_add(counter, 1)` and writes its
/// `(pattern_id, start, end)` triple at `1 + 3*slot`.
///
/// `input_len` is the program's input-buffer **capacity** (max bytes
/// the program is sized to handle in one dispatch). The actual
/// haystack byte count is read at runtime from a dedicated
/// `haystack_len` storage buffer so a single compiled program can
/// dispatch any haystack between zero and the declared capacity
/// without recompile.
///
/// Invalid pattern plans lower to an explicit trap program.
#[must_use]
#[allow(clippy::vec_init_then_push)]
pub fn nfa_scan(patterns: &[&str], input_buf: &str, hit_buf: &str, input_len: u32) -> Program {
    try_nfa_scan(patterns, input_buf, hit_buf, input_len).unwrap_or_else(|error| {
        crate::builder::invalid_output_program(
            OP_ID,
            hit_buf,
            DataType::U32,
            format!("Fix: {error}"),
        )
    })
}

/// Fallible NFA scan builder.
///
/// # Errors
///
/// Returns an actionable error when the compiled pattern set exceeds
/// one subgroup's state budget. Callers should shard via [`plan_shards`].
#[allow(clippy::vec_init_then_push)]
pub fn try_nfa_scan(
    patterns: &[&str],
    input_buf: &str,
    hit_buf: &str,
    input_len: u32,
) -> Result<Program, String> {
    let plan = try_compile(patterns)
        .map_err(|error| error.to_string())?
        .for_input_len(input_len);
    nfa_scan_with_plan(&plan, false, input_buf, hit_buf, input_len)
}

/// Canonical buffer name for the runtime-supplied haystack byte count.
/// Mirrors `classic_ac_bounded_ranges_program`'s `haystack_len` slot so
/// every scan kernel in `vyre-libs::matching` exposes the same
/// out-of-band length input.
pub const HAYSTACK_LEN_BUF: &str = "nfa_haystack_len";

/// Canonical buffer name for the runtime-supplied per-workgroup cursor
/// bound. Each workgroup walks bytes from `WorkgroupId(0)` up to
/// `min(haystack_len, WorkgroupId(0) + max_scan_bytes)`. Set to
/// `u32::MAX` for unbounded scans (legacy behavior - every workgroup
/// walks to the end of the haystack, giving O(N²) total work).
///
/// The bound exists because the entry-anchored cursor design dispatches
/// one workgroup per byte and each workgroup walks until accept OR
/// end-of-haystack - for a 62 MiB input that's ~1.9e15 transition-table
/// loads total, fundamentally O(N²). When the consumer knows the longest
/// possible match (e.g. `[MN].{22-24}\..{6-8}\..{27-38}` cannot exceed
/// 73 bytes), passing that bound flips the kernel to O(N × bound), which
/// for high-volume detector regexes drops the 62 MiB-shard cost from
/// ~30 s to a few milliseconds. Patterns whose longest possible match
/// would exceed `bound` (e.g. PEM blocks) need either a larger bound,
/// a literal-bookend kernel (BEGIN/END), or a different scan strategy.
pub const MAX_SCAN_BYTES_BUF: &str = "nfa_max_scan_bytes";

/// Build an NFA scan kernel from a precompiled plan.
///
/// Regex and other grammar frontends produce the same transition/epsilon table
/// shape as literal compilation, but their state graph is not recoverable from
/// Emit the per-peer-lane `k` block of a lane-major subgroup gather (transition or
/// epsilon), with the inner per-bit walk as a RUNTIME `loop_for` bounded by a
/// compile-time count instead of 32 unrolled `if_then` nodes.
///
/// `subgroup_shuffle` still requires a compile-time peer lane, so the caller keeps
/// `k` unrolled; only the inner bit index `bit` is made dynamic. The emitted block is
/// O(1) in `num_states`, so the whole scan shader is O(LANES) rather than
/// O(num_states) — the size the descriptor optimizer (`vyre-lower`) and naga emitter
/// scale on. A 771-state bare-`xor` NFA previously unrolled into thousands of nodes
/// and made `verify_then_optimize` run for minutes; this keeps it flat.
///
/// Semantics are identical to the unrolled form: for every `src_state = k*32 + bit`
/// with `src_state < num_states`, when peer bit `bit` is set, OR the `table` row for
/// `src_state` (this lane's slice) into `accum`. Index is
/// `src_state * row_stride + byte_term? + lane`. The accumulation is OR (idempotent,
/// order-independent), so the loop form preserves the unroll's result exactly.
#[allow(clippy::too_many_arguments)]
fn push_lane_major_gather(
    out: &mut Vec<Node>,
    k: u32,
    num_states: u32,
    peer_src: &str,
    peer_name: &str,
    accum: &str,
    table: &str,
    row_stride: u32,
    byte_term: Option<&Expr>,
    lane: &Expr,
) {
    out.push(Node::let_bind(
        peer_name,
        Expr::subgroup_shuffle(Expr::var(peer_src), Expr::u32(k)),
    ));
    let base = k * 32;
    if base >= num_states {
        // This peer lane owns no live states — nothing to gather. Matches the
        // compile-time `if src_state >= num_states { continue }` in the old unroll.
        return;
    }
    let inner_count = (num_states - base).min(32);
    let bit_var = format!("{peer_name}_bit");
    let src_state = Expr::add(Expr::u32(base), Expr::var(&bit_var));
    let row = Expr::mul(src_state, Expr::u32(row_stride));
    let idx = match byte_term {
        Some(bt) => Expr::add(Expr::add(row, bt.clone()), lane.clone()),
        None => Expr::add(row, lane.clone()),
    };
    let guard = Expr::ne(
        Expr::bitand(
            Expr::shr(Expr::var(peer_name), Expr::var(&bit_var)),
            Expr::u32(1),
        ),
        Expr::u32(0),
    );
    let assign = Node::assign(
        accum,
        Expr::bitor(Expr::var(accum), Expr::load(table, idx)),
    );
    out.push(Node::loop_for(
        &bit_var,
        Expr::u32(0),
        Expr::u32(inner_count),
        vec![Node::if_then(guard, vec![assign])],
    ));
}

/// the source strings. This builder keeps the executable scan program tied to
/// the actual compiled plan instead of rebuilding a literal-only plan.
///
/// # Errors
///
/// Returns an actionable error when the plan exceeds one subgroup's state
/// budget.
#[allow(clippy::vec_init_then_push)]
pub fn nfa_scan_with_plan(
    plan: &NfaPlan,
    has_epsilon: bool,
    input_buf: &str,
    hit_buf: &str,
    input_len: u32,
) -> Result<Program, String> {
    let plan = plan.clone().for_input_len(input_len);
    if plan.num_states > MAX_STATES_PER_SUBGROUP as u32 {
        return Err(format!(
            "NFA state count {} exceeds MAX_STATES_PER_SUBGROUP {}. \
             Fix: use `plan_shards` to split the pattern set across dispatches.",
            plan.num_states, MAX_STATES_PER_SUBGROUP
        ));
    }
    // input_len == 0 is legal: the byte loop runs 0 times and the
    // hit buffer stays empty. This is the natural answer for an
    // empty haystack; consumers should not special-case it at the
    // call site.

    let lane = Expr::LocalId { axis: 0 };
    let start = Expr::WorkgroupId { axis: 0 };
    let lane_u32 = || lane.clone();
    let start_u32 = || start.clone();
    let num_states = plan.num_states;
    let accepts = plan.accept_states.clone();
    let accept_state_ids = plan.accept_state_ids.clone();
    let accept_start_anchored = plan.accept_start_anchored.clone();
    let accept_end_anchored = plan.accept_end_anchored.clone();

    // Runtime haystack byte count. Read once per workgroup-bound
    // dispatch from the `nfa_haystack_len` uniform so a program
    // compiled for a 256-MiB capacity can scan any haystack length
    // from 0 .. capacity without recompiling. The constant
    // `plan.input_len` only sizes the input buffer declaration.
    let haystack_len_expr = || Expr::load(HAYSTACK_LEN_BUF, Expr::u32(0));

    // Per-workgroup cursor cap. The cursor loop runs from
    // `start = WorkgroupId(0)` to `min(haystack_len, start + max_scan_bytes)`.
    // Without this bound, each workgroup walks to the end of the haystack
    // (O(N) per workgroup × N workgroups = O(N²)). With a bound matched
    // to the longest possible pattern match, total work drops to O(N × bound).
    // Pass `u32::MAX` for unbounded scans to preserve legacy semantics.
    let max_scan_bytes_expr = || Expr::load(MAX_SCAN_BYTES_BUF, Expr::u32(0));
    // `min(haystack_len, start + max_scan_bytes)` - saturating add to
    // avoid wraparound when `start + max_scan_bytes > u32::MAX`. The
    // saturation produces u32::MAX, which then loses the `min` race
    // against `haystack_len`, so the cursor still stops at the real
    // haystack tail when the bound would have run past it.
    let scan_end_expr = || {
        let sum = Expr::select(
            Expr::lt(Expr::add(start_u32(), max_scan_bytes_expr()), start_u32()),
            // wrap detected → clamp to u32::MAX
            Expr::u32(u32::MAX),
            Expr::add(start_u32(), max_scan_bytes_expr()),
        );
        Expr::select(
            Expr::lt(sum.clone(), haystack_len_expr()),
            sum,
            haystack_len_expr(),
        )
    };

    // Per-cursor body. Runs inside the byte loop.
    let mut cursor_body: Vec<Node> = Vec::new();
    cursor_body.push(Node::let_bind(
        "byte",
        crate::scan::builders::load_packed_byte_expr(input_buf, Expr::var("cursor")),
    ));
    cursor_body.push(Node::let_bind("next_state", Expr::u32(0)));

    // Transition. Lane-major gather matching `subgroup_nfa::nfa_step`:
    //   for peer lane k in 0..LANES:
    //     peer = subgroup_shuffle(state_word, k)
    //     for bit i in 0..32:
    //       src = k*32 + i
    //       if src < num_states && ((peer >> i) & 1) != 0:
    //         next_state |= trans[src*256*LANES + byte*LANES + lane]
    //
    // target-text subgroup_shuffle requires compile-time peer so we unroll
    // k. We also unroll i (identical pattern to the primitive) so
    // each byte step is a straight-line block the optimiser can fold.
    let byte_term = Expr::mul(Expr::var("byte"), Expr::u32(LANES_PER_SUBGROUP as u32));
    for k in 0..LANES_PER_SUBGROUP as u32 {
        push_lane_major_gather(
            &mut cursor_body,
            k,
            num_states,
            "state_word",
            &format!("peer_{k}"),
            "next_state",
            "nfa_transition",
            256 * LANES_PER_SUBGROUP as u32,
            Some(&byte_term),
            &lane,
        );
    }

    // Epsilon closure  -  only when the pattern set has ε edges.
    // OR is idempotent so a fixed `num_states` iteration count
    // reaches fixpoint.
    if has_epsilon {
        let eps_iters = num_states.clamp(1, 32);
        let mut eps_body: Vec<Node> = Vec::new();
        for k in 0..LANES_PER_SUBGROUP as u32 {
            push_lane_major_gather(
                &mut eps_body,
                k,
                num_states,
                "next_state",
                &format!("eps_peer_{k}"),
                "next_state",
                "nfa_epsilon",
                LANES_PER_SUBGROUP as u32,
                None,
                &lane,
            );
        }
        cursor_body.push(Node::loop_for(
            "eps_iter",
            Expr::u32(0),
            Expr::u32(eps_iters),
            eps_body,
        ));
    }

    cursor_body.push(Node::assign("state_word", Expr::var("next_state")));

    // Per-cursor accept emission. Fixes the post-loop-only bug the
    // PHASE3_SCAN audit flagged  -  intermediate matches were lost.
    // Slot 0 of hit_buf is the atomic counter; each match claims
    // the next `(pattern_id, start, end)` triple via atomic_add(1).
    let max_hits = 10_000u32;
    for (accept_idx, (&accept_state, &(pattern_id, _pattern_len))) in
        accept_state_ids.iter().zip(&accepts).enumerate()
    {
        let word_idx = accept_state / 32;
        let bit_offset = accept_state % 32;
        let mut accept_guard = Expr::ne(
            Expr::bitand(
                Expr::var("state_word"),
                Expr::shl(Expr::u32(1), Expr::u32(bit_offset)),
            ),
            Expr::u32(0),
        );
        if accept_start_anchored
            .get(accept_idx)
            .copied()
            .unwrap_or(false)
        {
            accept_guard = Expr::and(accept_guard, Expr::eq(start_u32(), Expr::u32(0)));
        }
        if accept_end_anchored
            .get(accept_idx)
            .copied()
            .unwrap_or(false)
        {
            accept_guard = Expr::and(
                accept_guard,
                Expr::eq(
                    Expr::add(Expr::var("cursor"), Expr::u32(1)),
                    haystack_len_expr(),
                ),
            );
        }
        cursor_body.push(Node::if_then(
            Expr::eq(lane_u32(), Expr::u32(word_idx)),
            vec![Node::if_then(
                accept_guard,
                vec![
                    Node::let_bind(
                        "slot_idx",
                        Expr::atomic_add(hit_buf, Expr::u32(0), Expr::u32(1)),
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("slot_idx"), Expr::u32(max_hits)),
                        vec![
                            Node::let_bind(
                                "triple_base",
                                Expr::add(
                                    Expr::u32(1),
                                    Expr::mul(Expr::var("slot_idx"), Expr::u32(3)),
                                ),
                            ),
                            Node::store(hit_buf, Expr::var("triple_base"), Expr::u32(pattern_id)),
                            Node::store(
                                hit_buf,
                                Expr::add(Expr::var("triple_base"), Expr::u32(1)),
                                start_u32(),
                            ),
                            Node::store(
                                hit_buf,
                                Expr::add(Expr::var("triple_base"), Expr::u32(2)),
                                Expr::add(Expr::var("cursor"), Expr::u32(1)),
                            ),
                        ],
                    ),
                ],
            )],
        ));
    }

    // Top-level body: seed state 0 in lane 0, then loop over input.
    let mut body: Vec<Node> = Vec::new();
    body.push(Node::let_bind(
        "state_word",
        Expr::select(
            Expr::eq(lane_u32(), Expr::u32(0)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    // Regex-compiled plans connect the shared entry to pattern starts
    // via ε-edges. Close ε from the seeded entry before consuming the
    // first byte so transition reads include those starts.
    if has_epsilon {
        let eps_iters = num_states.clamp(1, 32);
        let mut initial_eps_body: Vec<Node> = Vec::new();
        for k in 0..LANES_PER_SUBGROUP as u32 {
            push_lane_major_gather(
                &mut initial_eps_body,
                k,
                num_states,
                "state_word",
                &format!("init_eps_peer_{k}"),
                "state_word",
                "nfa_epsilon",
                LANES_PER_SUBGROUP as u32,
                None,
                &lane,
            );
        }
        body.push(Node::loop_for(
            "init_eps_iter",
            Expr::u32(0),
            Expr::u32(eps_iters),
            initial_eps_body,
        ));
    }
    body.push(Node::loop_for(
        "cursor",
        start_u32(),
        scan_end_expr(),
        cursor_body,
    ));

    let num_hit_slots = 1 + 10_000 * 3;
    let input_words = plan.input_len.div_ceil(4).max(1);
    let buffers = vec![
        BufferDecl::storage(input_buf, 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(input_words),
        BufferDecl::storage("nfa_transition", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(num_states * 256 * LANES_PER_SUBGROUP as u32),
        BufferDecl::storage("nfa_epsilon", 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(num_states * LANES_PER_SUBGROUP as u32),
        BufferDecl::storage(hit_buf, 3, BufferAccess::ReadWrite, DataType::U32)
            .with_count(num_hit_slots),
        BufferDecl::storage(HAYSTACK_LEN_BUF, 4, BufferAccess::ReadOnly, DataType::U32)
            .with_count(1),
        BufferDecl::storage(MAX_SCAN_BYTES_BUF, 5, BufferAccess::ReadOnly, DataType::U32)
            .with_count(1),
    ];

    Ok(Program::wrapped(
        buffers,
        [LANES_PER_SUBGROUP as u32, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::and(
                    Expr::lt(lane_u32(), Expr::u32(LANES_PER_SUBGROUP as u32)),
                    Expr::lt(start_u32(), haystack_len_expr()),
                ),
                body,
            )]),
        }],
    ))
}

mod alloc;
mod plan;
mod shards;
mod tables;

pub use plan::{compile, try_compile, NfaCompileError, NfaPlan};
pub use shards::plan_shards;
pub use tables::{
    build_epsilon_table, build_transition_table, build_transition_table_lane_major,
    try_build_epsilon_table, try_build_transition_table, try_build_transition_table_lane_major,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_literal_pattern_counts_states() {
        let plan = compile(&["abc"]);
        assert_eq!(plan.num_states, 4);
        assert_eq!(plan.accept_states.len(), 1);
    }

    #[test]
    fn compile_two_patterns_share_entry_state() {
        let plan = compile(&["ab", "cd"]);
        assert_eq!(plan.num_states, 5);
        assert_eq!(plan.accept_states.len(), 2);
    }

    #[test]
    fn try_compile_matches_legacy_plan_without_truncating_fields() {
        let plan = try_compile(&["ab", "", "xyz"])
            .expect("Fix: small NFA pattern set must compile fallibly");

        assert_eq!(plan.num_states, 6);
        assert_eq!(plan.accept_states, vec![(0, 2), (1, 0), (2, 3)]);
        assert_eq!(plan.accept_state_ids, vec![2, 0, 5]);
        assert_eq!(plan.accept_start_anchored, vec![false; 3]);
        assert_eq!(plan.accept_end_anchored, vec![false; 3]);
    }

    #[test]
    fn fallible_table_builders_match_legacy_table_shapes() {
        let patterns = ["abc", "de"];
        assert_eq!(
            try_build_transition_table(&patterns)
                .expect("Fix: fallible transition table should build"),
            build_transition_table(&patterns)
        );
        assert_eq!(
            try_build_transition_table_lane_major(&patterns)
                .expect("Fix: fallible lane-major transition table should build"),
            build_transition_table_lane_major(&patterns)
        );
        assert_eq!(
            try_build_epsilon_table(&patterns).expect("Fix: fallible epsilon table should build"),
            build_epsilon_table(&patterns)
        );
    }

    #[test]
    fn nfa_compile_and_tables_use_checked_allocation_paths() {
        let root = include_str!("nfa.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: nfa.rs must contain production section");
        let production = [
            root,
            include_str!("nfa/alloc.rs"),
            include_str!("nfa/plan.rs"),
            include_str!("nfa/tables.rs"),
        ]
        .join("\n");

        assert!(
            production.contains("pub fn try_compile")
                && production.contains("u32::try_from(p.len())")
                && production.contains("u32::try_from(pid)")
                && production.contains("checked_add(len)")
                && production.contains("try_build_transition_table")
                && production.contains("try_reserve_vec_to_capacity")
                && !production.contains("p.len() as u32")
                && !production.contains("pid as u32")
                && !production.contains("next_state += len")
                && !production.contains("vec![0_u32;"),
            "Fix: NFA compilation must not truncate pattern ids, pattern lengths, state counts, or allocate tables through infallible zero-vector construction."
        );
    }

    #[test]
    fn transition_table_has_lane_major_size() {
        let t = build_transition_table(&["abc", "de"]);
        let plan = compile(&["abc", "de"]);
        assert_eq!(
            t.len(),
            (plan.num_states as usize) * 256 * LANES_PER_SUBGROUP,
            "transition table must be lane-major [num_states × 256 × LANES_PER_SUBGROUP] \
             to match subgroup_nfa::nfa_step contract (VYRE_MEM_LAYOUT CRITICAL-2)",
        );
    }

    #[test]
    fn transition_table_encodes_first_character_in_dst_lane() {
        // "abc": states are entry=0, 1('a'-consumed), 2('b'-consumed), 3('c'-consumed).
        // 0 ->'a'-> 1 means dst=1 is held in lane 0 bit 1.
        let t = build_transition_table(&["abc"]);
        let idx = 0 * 256 * LANES_PER_SUBGROUP + (b'a' as usize) * LANES_PER_SUBGROUP + 0;
        assert_eq!(t[idx], 1_u32 << 1, "0 -a-> 1 should set lane-0 bit-1");
    }

    #[test]
    fn transition_table_spans_correct_dst_lane_when_dst_gte_32() {
        // 33 patterns of length 1 produces state_cursor 1..=33, so
        // one transition lands in dst_lane 1 (dst state 32 → lane 1 bit 0).
        let pats: Vec<String> = (0..33)
            .map(|i| format!("{}", char::from(b'a' + i)))
            .collect();
        let refs: Vec<&str> = pats.iter().map(String::as_str).collect();
        let t = build_transition_table(&refs);
        let plan = compile(&refs);
        // Dst state 32 is reached from entry (state 0) on byte ('a' + 32) = '!' + …  -  find it by search.
        // Any entry at lane 1 should be non-zero.
        let has_lane1 = (0..256)
            .map(|byte| t[0 * 256 * LANES_PER_SUBGROUP + byte * LANES_PER_SUBGROUP + 1])
            .any(|v| v != 0);
        assert!(
            has_lane1,
            "dst states ≥32 must populate lane ≥1 (plan has {} states)",
            plan.num_states
        );
    }

    #[test]
    fn transition_table_encodes_every_byte_independently() {
        let t = build_transition_table(&["xy"]);
        let x_idx = 0 * 256 * LANES_PER_SUBGROUP + (b'x' as usize) * LANES_PER_SUBGROUP + 0;
        let y_idx = 0 * 256 * LANES_PER_SUBGROUP + (b'y' as usize) * LANES_PER_SUBGROUP + 0;
        assert_ne!(t[x_idx], 0);
        assert_eq!(t[y_idx], 0, "entry does not take 'y' directly");
    }

    #[test]
    fn epsilon_table_has_lane_major_size() {
        let n = compile(&["abc"]).num_states as usize;
        assert_eq!(build_epsilon_table(&["abc"]).len(), n * LANES_PER_SUBGROUP,);
    }

    #[test]
    fn epsilon_table_all_zero_for_literals() {
        let t = build_epsilon_table(&["abc"]);
        assert!(t.iter().all(|&w| w == 0));
    }

    #[test]
    fn plan_shards_fit_within_limit() {
        let big: Vec<String> = (0..12).map(|_| "a".repeat(100)).collect();
        let refs: Vec<&str> = big.iter().map(String::as_str).collect();
        let shards = plan_shards(&refs);
        for s in &shards {
            let sum: usize = s.iter().map(|p| p.len()).sum();
            assert!(sum < MAX_STATES_PER_SUBGROUP);
        }
        assert!(shards.len() >= 2);
    }

    #[test]
    fn lane_major_transition_table_has_correct_size() {
        let t = build_transition_table_lane_major(&["abc", "de"]);
        let plan = compile(&["abc", "de"]);
        let padded = LANES_PER_SUBGROUP * (plan.num_states as usize).div_ceil(LANES_PER_SUBGROUP);
        assert_eq!(
            t.len(),
            padded * 256 * LANES_PER_SUBGROUP,
            "lane-major table must be padded to LANES multiple per byte row"
        );
    }

    #[test]
    fn lane_major_transition_table_encodes_same_edges_as_flat() {
        let patterns = &["abc", "xyz"];
        let flat = build_transition_table(patterns);
        let lm = build_transition_table_lane_major(patterns);
        let plan = compile(patterns);
        let num_states = plan.num_states as usize;
        let padded = LANES_PER_SUBGROUP * num_states.div_ceil(LANES_PER_SUBGROUP);

        // Every (src, byte, lane) entry must match between the two layouts.
        for src in 0..num_states {
            for byte in 0..256 {
                for lane in 0..LANES_PER_SUBGROUP {
                    let flat_idx =
                        src * 256 * LANES_PER_SUBGROUP + byte * LANES_PER_SUBGROUP + lane;
                    let lm_idx = lane * padded * 256 + byte * padded + src;
                    assert_eq!(
                        flat[flat_idx], lm[lm_idx],
                        "mismatch at src={src} byte={byte} lane={lane}"
                    );
                }
            }
        }
    }

    #[test]
    fn plan_shards_empty_on_empty_input() {
        let empty: &[&str] = &[];
        assert!(plan_shards(empty).is_empty());
    }

    #[test]
    fn nfa_scan_emits_valid_program_with_expected_buffers() {
        let p = nfa_scan(&["abc"], "input", "hits", 16);
        assert_eq!(p.workgroup_size, [LANES_PER_SUBGROUP as u32, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"input"));
        assert!(names.contains(&"nfa_transition"));
        assert!(names.contains(&"nfa_epsilon"));
        assert!(names.contains(&"hits"));
    }

    #[test]
    fn nfa_scan_transition_buffer_has_primitive_compatible_count() {
        let p = nfa_scan(&["abc"], "input", "hits", 16);
        let trans = p
            .buffers
            .iter()
            .find(|b| b.name() == "nfa_transition")
            .expect("Fix: nfa_transition buffer; restore this invariant before continuing.");
        let plan = compile(&["abc"]);
        assert_eq!(
            trans.count,
            plan.num_states * 256 * LANES_PER_SUBGROUP as u32,
            "buffer count must match lane-major [num_states × 256 × LANES] layout \
             that subgroup_nfa::nfa_step consumes",
        );
    }

    #[test]
    fn nfa_scan_epsilon_buffer_has_primitive_compatible_count() {
        let p = nfa_scan(&["abc"], "input", "hits", 16);
        let eps = p
            .buffers
            .iter()
            .find(|b| b.name() == "nfa_epsilon")
            .expect("Fix: nfa_epsilon buffer; restore this invariant before continuing.");
        let plan = compile(&["abc"]);
        assert_eq!(eps.count, plan.num_states * LANES_PER_SUBGROUP as u32);
    }

    #[test]
    fn try_nfa_scan_rejects_over_budget_patterns_with_result_error() {
        let big: Vec<String> = (0..12).map(|_| "a".repeat(100)).collect();
        let refs: Vec<&str> = big.iter().map(String::as_str).collect();
        let error = try_nfa_scan(&refs, "input", "hits", 16)
            .expect_err("Fix: over-budget NFA must return an error contract");
        assert!(
            error.contains("MAX_STATES_PER_SUBGROUP") && error.contains("plan_shards"),
            "Fix: NFA error must name the state budget and sharding remedy: {error}"
        );
    }

    #[test]
    fn nfa_scan_accepts_zero_input_len() {
        // Contract: input_len == 0 produces a valid empty-result
        // Program, so callers can route empty haystacks through the
        // same dispatch builder as non-empty inputs.
        let prog = nfa_scan(&["abc"], "input", "hits", 0);
        let names: Vec<&str> = prog.buffers().iter().map(|b| b.name()).collect();
        assert!(names.contains(&"input"));
        assert!(names.contains(&"hits"));
    }

    #[test]
    fn nfa_plan_input_len_is_attachable() {
        let plan = compile(&["abc"]).for_input_len(64);
        assert_eq!(plan.input_len, 64);
    }
}

/// Benchmark-only helpers for NFA transition-table layout comparison.
///
/// Gated behind the `bench` feature so normal consumers do not pay
/// compile-time cost for code that is only exercised by Criterion.
#[cfg(feature = "bench")]
pub mod bench {
    pub use super::build_transition_table;
    pub use super::build_transition_table_lane_major;
    pub use super::compile;
    pub use vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP;

    use vyre_primitives::nfa::subgroup_nfa::MAX_EPSILON_ITERS;

    /// Reference-oracle NFA step using the **lane-major** transition table.
    ///
    /// Layout: `lane * padded_num_states * 256 + byte * padded_num_states + src_state`.
    /// Mirrors the semantics of `vyre_primitives::nfa::subgroup_nfa::cpu_step`
    /// but indexes into the lane-major table produced by
    /// [`build_transition_table_lane_major`].
    pub fn reference_step_lane_major(
        state: &[u32],
        byte: u8,
        transition: &[u32],
        epsilon: &[u32],
        num_states: usize,
    ) -> Vec<u32> {
        assert_eq!(state.len(), LANES_PER_SUBGROUP);
        let padded_states = LANES_PER_SUBGROUP * num_states.div_ceil(LANES_PER_SUBGROUP);
        assert_eq!(
            transition.len(),
            padded_states * 256 * LANES_PER_SUBGROUP,
            "lane-major transition table size mismatch"
        );
        assert_eq!(
            epsilon.len(),
            num_states * LANES_PER_SUBGROUP,
            "epsilon table size mismatch"
        );

        let mut acc = vec![0_u32; LANES_PER_SUBGROUP];
        for (k, &peer) in state.iter().enumerate() {
            for i in 0..32 {
                let src_state = k * 32 + i;
                if src_state >= num_states {
                    break;
                }
                if (peer >> i) & 1 == 0 {
                    continue;
                }
                for (lane, slot) in acc.iter_mut().enumerate() {
                    let idx =
                        lane * padded_states * 256 + (byte as usize) * padded_states + src_state;
                    *slot |= transition[idx];
                }
            }
        }

        // Epsilon closure  -  real fixpoint. Same logic as flat layout;
        // epsilon table is not transposed.
        for _ in 0..MAX_EPSILON_ITERS as usize {
            let prev = acc.clone();
            for (k, &peer) in prev.iter().enumerate() {
                for i in 0..32 {
                    let src_state = k * 32 + i;
                    if src_state >= num_states {
                        break;
                    }
                    if (peer >> i) & 1 == 0 {
                        continue;
                    }
                    for (lane, slot) in acc.iter_mut().enumerate() {
                        let idx = src_state * LANES_PER_SUBGROUP + lane;
                        *slot |= epsilon[idx];
                    }
                }
            }
            if acc == prev {
                break;
            }
        }
        acc
    }
}
