//! Single-input segmentation for the megakernel scan.
//!
//! Today a work-item is `(file_idx, rule_idx)` and each lane scans the WHOLE
//! file for one rule (`dispatcher.rs`: `file_idx = claim / rule_count`). For one
//! large file that leaves occupancy bounded by `rule_count`, with every busy lane
//! walking all N bytes sequentially — the reason the GPU loses to Hyperscan on a
//! single 8 MiB scan. Splitting each file into many overlapping windows turns the
//! work-item into `(segment_idx, rule_idx)`, so a single file saturates the whole
//! device (see `docs/GPU_OOM_SEGMENTATION.md`).
//!
//! ## Soundness (why overlapping windows are exact)
//!
//! For an Aho-Corasick / failure-function DFA, the state after consuming the byte
//! at offset `i` is a function of at most the last `overlap` bytes (the longest
//! pattern), independent of the state the scan started in. So a window that
//! begins scanning `overlap` bytes BEFORE the region it owns reaches the exact
//! state a full-file scan would have at `emit_start`. Each window therefore:
//!   * scans `[scan_start, emit_end)` from state 0 — the `[scan_start, emit_start)`
//!     prefix is warm-up only, it emits nothing; and
//!   * emits only matches whose END offset lies in `[emit_start, emit_end)`.
//!
//! The emit ranges tile `[0, file_len)` exactly — contiguous, gap-free, and
//! disjoint — so every match is produced by exactly one window: no double count,
//! no miss. `plan_segments` is the host-side planner; the kernel reads the
//! resulting table to derive `(file_idx, scan_start, emit_start, emit_end)` from
//! a claim, and guards emission with `end >= emit_start && end < emit_end`.

/// One scan window of a file.
///
/// All offsets are file-relative bytes. Invariant:
/// `scan_start <= emit_start < emit_end` and `scan_start == emit_start - overlap`
/// clamped at 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// Index into the file table this window belongs to.
    pub file_idx: u32,
    /// File-relative offset where scanning begins (DFA-state warm-up start).
    pub scan_start: u32,
    /// First file-relative offset this window OWNS: a match whose end offset is
    /// in `[emit_start, emit_end)` is emitted here; an end `< emit_start` belongs
    /// to the previous window's owned region.
    pub emit_start: u32,
    /// Exclusive end of both the owned (emit) range and the scan range.
    pub emit_end: u32,
}

/// Number of `u32` words per segment in the device segment table.
pub const SEGMENT_WORDS: usize = 4;

impl Segment {
    /// Total bytes this window scans (warm-up prefix + owned region).
    #[must_use]
    pub const fn scan_len(&self) -> u32 {
        self.emit_end - self.scan_start
    }

    /// Bytes this window owns (emits matches for).
    #[must_use]
    pub const fn emit_len(&self) -> u32 {
        self.emit_end - self.emit_start
    }

    /// The device-ABI words for this segment, in the exact order the kernel
    /// decodes them from `segments[seg_idx * SEGMENT_WORDS ..]`:
    /// `[file_idx, scan_start, emit_start, emit_end]` (offsets file-relative —
    /// the kernel adds `file_offsets[file_idx]` to reach the packed haystack).
    #[must_use]
    pub const fn abi_words(&self) -> [u32; SEGMENT_WORDS] {
        [
            self.file_idx,
            self.scan_start,
            self.emit_start,
            self.emit_end,
        ]
    }
}

/// Build the flat device segment table (`segment_count * SEGMENT_WORDS` u32s) for
/// a batch of files at the given window geometry — the buffer the segmented
/// megakernel binds and decodes a claim's `(file_idx, scan_start, emit_start,
/// emit_end)` from. Row order matches [`plan_segments`], so `seg_idx` indexes it
/// directly. See [`plan_segments`] for the soundness of the tiling.
///
/// # Panics
/// Panics if `seg_len == 0` (via [`plan_segments`]).
#[must_use]
pub fn segment_table(file_lens: &[u32], seg_len: u32, overlap: u32) -> Vec<u32> {
    let segments = plan_segments(file_lens, seg_len, overlap);
    let mut words = Vec::with_capacity(segments.len() * SEGMENT_WORDS);
    for seg in &segments {
        words.extend_from_slice(&seg.abi_words());
    }
    words
}

/// Plan the scan windows for a batch of files.
///
/// `seg_len` is the owned (emit) width per window and MUST be positive;
/// `overlap` is the warm-up width and should equal the catalog's longest pattern
/// length so each window converges to the correct DFA state before its owned
/// region. A file of length `L` yields `ceil(L / seg_len)` windows; a zero-length
/// file yields none (it can match nothing).
///
/// # Panics
/// Panics if `seg_len == 0` (a zero-width owned region cannot tile a file).
#[must_use]
pub fn plan_segments(file_lens: &[u32], seg_len: u32, overlap: u32) -> Vec<Segment> {
    assert!(
        seg_len > 0,
        "segment owned-width (seg_len) must be positive"
    );
    let mut segments = Vec::new();
    for (file_idx, &len) in file_lens.iter().enumerate() {
        // File count is bounded to u32 upstream (FileMetadata::size_bytes etc.).
        let file_idx = file_idx as u32;
        let mut emit_start = 0u32;
        while emit_start < len {
            let emit_end = emit_start.saturating_add(seg_len).min(len);
            let scan_start = emit_start.saturating_sub(overlap);
            segments.push(Segment {
                file_idx,
                scan_start,
                emit_start,
                emit_end,
            });
            emit_start = emit_end;
        }
    }
    segments
}

/// Number of windows `plan_segments` will produce for `file_lens` at `seg_len`,
/// without allocating the table — for sizing the device work queue
/// (`queue_len = segment_count * rule_count`).
///
/// # Panics
/// Panics if `seg_len == 0`.
#[must_use]
pub fn segment_count(file_lens: &[u32], seg_len: u32) -> u64 {
    assert!(
        seg_len > 0,
        "segment owned-width (seg_len) must be positive"
    );
    let seg_len = u64::from(seg_len);
    file_lens
        .iter()
        .map(|&len| u64::from(len).div_ceil(seg_len))
        .sum()
}

/// Dense byte-DFA columns per state in a [`BatchRuleProgram`] transition table
/// (`transitions[state * 256 + byte] -> next_state`).
const DFA_BYTE_COLUMNS: usize = 256;

/// Maximum reachable off-diagonal product-automaton pairs [`dfa_sync_distance`]
/// will explore before conservatively reporting "not provably bounded" (`None`).
/// Bounds the analysis at ~`BUDGET * 256` edge steps per rule so a pathologically
/// large DFA cannot stall catalog compilation; secret-token DFAs are orders of
/// magnitude smaller and always analyze exactly.
const PRODUCT_PAIR_BUDGET: usize = 1_000_000;

/// Outcome of the [`dfa_sync_class`] synchronization-distance analysis. The three
/// arms are operationally distinct for the caller's diagnostics: only
/// [`SyncClass::Bounded`] is GPU-segmentable, but a [`SyncClass::UnboundedCycle`]
/// (the DFA genuinely never re-synchronizes — e.g. a `.*` body that must remember
/// unbounded context) can NEVER move to GPU, whereas a [`SyncClass::BudgetExceeded`]
/// (the analysis hit [`PRODUCT_PAIR_BUDGET`] before proving bounded/unbounded)
/// MIGHT segment with a larger budget — the catalog builder logs the split so the
/// over-rejection from budget vs. true unbounded memory is never conflated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncClass {
    /// The DFA synchronizes within this many bytes; a warm-up `overlap >= d`
    /// reconstructs the exact full-scan state at `emit_start`. GPU-segmentable
    /// iff `d <= overlap`.
    Bounded(u32),
    /// A cycle of off-diagonal product pairs is reachable: two scans starting at
    /// different states never provably converge ⇒ infinite memory ⇒ whole-file.
    UnboundedCycle,
    /// The reachable product automaton exceeded [`PRODUCT_PAIR_BUDGET`] before the
    /// analysis terminated. Conservatively treated as not-segmentable, but
    /// DISTINCT from `UnboundedCycle`: a larger budget could still prove it
    /// bounded. Surfaced separately so the budget can be tuned to the real
    /// distribution instead of silently capping recall onto the host path.
    BudgetExceeded,
}

impl SyncClass {
    /// The synchronization distance when proven bounded, else `None` (the legacy
    /// [`dfa_sync_distance`] contract — `UnboundedCycle` and `BudgetExceeded` both
    /// collapse to "not provably segmentable").
    #[must_use]
    pub const fn bounded(self) -> Option<u32> {
        match self {
            Self::Bounded(d) => Some(d),
            Self::UnboundedCycle | Self::BudgetExceeded => None,
        }
    }
}

/// SOUNDNESS GATE for tuned segmentation: the DFA's finite-memory /
/// **synchronization distance** — the smallest `O` such that for every state
/// reachable from the start and every string `w` with `|w| >= O`, `δ*(0, w) ==
/// δ*(q, w)`. After `O` common bytes the state no longer depends on where the
/// scan started, so a window that warms up over `overlap >= O` bytes
/// reconstructs the exact full-scan state at `emit_start`. Returns `None` when
/// the distance is UNBOUNDED (infinite memory) — the rule cannot be segmented
/// and must be scanned whole-file (one segment), fail-safe and logged.
///
/// Why not "longest start→accept path": an unanchored search DFA self-loops at
/// the start on non-matching bytes, so that path is always infinite, and
/// branching/overlapping patterns make longest-path ≠ synchronization distance
/// regardless (proved by the `*_sync_distance_*` tests). The correct analysis is
/// a PAIRWISE PRODUCT automaton over UNORDERED state pairs `{a, b}` (the
/// convergence relation is symmetric): a diagonal pair `{a, a}` is absorbing
/// (the runs have met and stay met). Seeding from every reachable `{0, q}` and
/// following common-byte transitions `{a,b} →_c {δ(a,c), δ(b,c)}`, the rule has
/// finite memory iff NO cycle of off-diagonal pairs is reachable; then `O` is the
/// longest off-diagonal path before the diagonal is first reached (0 when the
/// only reachable seed is the diagonal `{0,0}`).
///
/// `transitions` is the dense `state * 256 + byte -> next_state` table of a
/// `BatchRuleProgram` (`vyre_runtime::megakernel::rule_catalog`). The accept
/// table is deliberately NOT a parameter: synchronization depends only on the
/// transition structure, not on which states accept.
///
/// # Panics
/// Panics if `transitions.len() < state_count * 256` (a malformed table the
/// device decode would also read out of bounds); callers pass validated
/// [`BatchRuleProgram`] tables.
/// Thin wrapper over [`dfa_sync_class`] returning just the bounded distance
/// (`None` for `UnboundedCycle` or `BudgetExceeded`). Callers that need to tell
/// "genuinely unbounded" from "budget-capped" — e.g. for diagnostics — call
/// [`dfa_sync_class`] directly.
#[must_use]
pub fn dfa_sync_distance(transitions: &[u32], state_count: u32) -> Option<u32> {
    dfa_sync_class(transitions, state_count).bounded()
}

/// Classify a dense byte-DFA by its bounded synchronization distance.
///
/// Returns [`SyncClass::Bounded`] when every reachable start state converges
/// within a finite overlap, [`SyncClass::UnboundedCycle`] when some off-diagonal
/// state pair can keep diverging forever, and [`SyncClass::BudgetExceeded`] when
/// the product-automaton analysis exceeds its defensive work budget.
///
/// # Panics
/// Panics if `transitions.len() < state_count * 256`.
#[must_use]
pub fn dfa_sync_class(transitions: &[u32], state_count: u32) -> SyncClass {
    let n = state_count as usize;
    if n <= 1 {
        // 0 states: nothing to scan. 1 state: every byte self-loops, so all
        // start states already coincide — synchronization distance 0.
        return SyncClass::Bounded(0);
    }
    assert!(
        transitions.len() >= n * DFA_BYTE_COLUMNS,
        "transition table shorter than state_count * 256"
    );
    let delta = |s: usize, b: usize| -> usize {
        let t = transitions[s * DFA_BYTE_COLUMNS + b] as usize;
        // A malformed out-of-range target would index out of bounds on-device;
        // clamp into range so the analysis stays total (it can only make the
        // bound LARGER / more conservative, never spuriously "synchronized").
        if t < n { t } else { 0 }
    };

    // States reachable from the start (the only states a real scan can be in,
    // hence the only second coordinates worth seeding). Restricting to these
    // avoids spurious off-diagonal cycles between unreachable states.
    let mut reachable = vec![false; n];
    reachable[0] = true;
    let mut stack = vec![0usize];
    while let Some(s) = stack.pop() {
        for b in 0..DFA_BYTE_COLUMNS {
            let t = delta(s, b);
            if !reachable[t] {
                reachable[t] = true;
                stack.push(t);
            }
        }
    }

    // Canonical unordered off-diagonal pair key (lo < hi).
    let key = |a: usize, b: usize| -> (usize, usize) { if a < b { (a, b) } else { (b, a) } };

    // BFS the product automaton from every reachable seed {0, q}, collecting the
    // reachable OFF-DIAGONAL pairs and their off-diagonal successor edges. A
    // diagonal successor is a "terminal" exit (the runs have met).
    use std::collections::HashMap;
    let mut index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut off_succ: Vec<Vec<usize>> = Vec::new();
    let mut frontier: Vec<(usize, usize)> = Vec::new();
    let intern = |pair: (usize, usize),
                  index: &mut HashMap<(usize, usize), usize>,
                  off_succ: &mut Vec<Vec<usize>>,
                  frontier: &mut Vec<(usize, usize)>|
     -> usize {
        if let Some(&id) = index.get(&pair) {
            return id;
        }
        let id = off_succ.len();
        index.insert(pair, id);
        off_succ.push(Vec::new());
        frontier.push(pair);
        id
    };
    for q in 0..n {
        if reachable[q] && q != 0 {
            intern(key(0, q), &mut index, &mut off_succ, &mut frontier);
        }
    }
    let mut head = 0;
    while head < frontier.len() {
        // Conservative cost ceiling: the product automaton has up to `n^2 / 2`
        // off-diagonal pairs, so a large DFA could explode the BFS. A rule whose
        // reachable product exceeds the budget is treated as NOT provably
        // bounded (return `None`) — the caller scans it whole-file. This can only
        // FORGO a segmentation opportunity, never produce an unsound one, so it
        // is a safe (recall-preserving) ceiling, not a silent correctness
        // fallback. Typical secret DFAs are far below it and analyze exactly.
        if off_succ.len() > PRODUCT_PAIR_BUDGET {
            return SyncClass::BudgetExceeded;
        }
        let (a, b) = frontier[head];
        let id = head;
        head += 1;
        for byte in 0..DFA_BYTE_COLUMNS {
            let na = delta(a, byte);
            let nb = delta(b, byte);
            if na == nb {
                continue; // diagonal exit — the runs have synchronized
            }
            let succ = intern(key(na, nb), &mut index, &mut off_succ, &mut frontier);
            off_succ[id].push(succ);
        }
    }
    // Dedup successor edges so Kahn in-degrees / longest-path count each once.
    for succs in &mut off_succ {
        succs.sort_unstable();
        succs.dedup();
    }

    let pair_count = off_succ.len();
    if pair_count == 0 {
        // Only the diagonal {0,0} was reachable as a seed ⇒ already synchronized.
        return SyncClass::Bounded(0);
    }

    // Kahn topological sort over the off-diagonal subgraph; a remaining node ⇒ a
    // cycle of off-diagonal pairs ⇒ unbounded memory ⇒ not segmentable.
    let mut indegree = vec![0u32; pair_count];
    for succs in &off_succ {
        for &t in succs {
            indegree[t] += 1;
        }
    }
    let mut topo: Vec<usize> = Vec::new();
    let mut queue: Vec<usize> = (0..pair_count).filter(|&p| indegree[p] == 0).collect();
    while let Some(p) = queue.pop() {
        topo.push(p);
        for &t in &off_succ[p] {
            indegree[t] -= 1;
            if indegree[t] == 0 {
                queue.push(t);
            }
        }
    }
    if topo.len() != pair_count {
        return SyncClass::UnboundedCycle; // off-diagonal cycle ⇒ infinite memory
    }

    // Longest path to first diagonal, in reverse topological order so every
    // off-diagonal successor is finalized before its predecessor. Each step is
    // one byte; a pair with only diagonal successors has distance 1.
    let mut dist = vec![0u32; pair_count];
    for &p in topo.iter().rev() {
        let mut best = 0u32;
        for &t in &off_succ[p] {
            best = best.max(dist[t]);
        }
        dist[p] = 1 + best;
    }

    // O = the worst seed's distance (diagonal seed {0,0} contributes 0).
    let mut sync = 0u32;
    for q in 0..n {
        if reachable[q] && q != 0 {
            if let Some(&id) = index.get(&key(0, q)) {
                sync = sync.max(dist[id]);
            }
        }
    }
    SyncClass::Bounded(sync)
}

/// The minimum warm-up `overlap` that keeps intra-file segmentation EXACT for an
/// entire rule catalog: the maximum [`dfa_sync_distance`] over every rule.
/// Returns `None` when ANY rule has infinite memory (an unbounded-gap pattern) —
/// the catalog cannot be soundly segmented at a shorter window than the whole
/// file, so the caller MUST fall back to one segment per file (and should log
/// which rule forced it; that is a recall-preserving slow path, not a silent
/// fallback). An empty catalog returns `Some(0)` (nothing to warm up).
///
/// This is the single value the host needs to pick a sound `seg_len`: any
/// `seg_len` paired with `overlap >= this` produces byte-identical results to a
/// dense whole-file scan (proved transitively by `dfa_sync_distance`'s
/// `*_overlap_makes_segmentation_exact` proptest).
#[must_use]
pub fn catalog_sync_overlap(rules: &[vyre_runtime::megakernel::BatchRuleProgram]) -> Option<u32> {
    let mut overlap = 0u32;
    for rule in rules {
        let sync = dfa_sync_distance(&rule.transitions, rule.state_count)?;
        overlap = overlap.max(sync);
    }
    Some(overlap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeSet;
    use vyre_libs::scan::classic_ac::{classic_ac_compile, classic_ac_scan, ClassicAcAutomaton};

    // ---- Model DFA for the dense-vs-segmented scan parity oracle ----
    //
    // The GPU kernel walks a byte DFA: `state = transitions[state][byte]`, and at
    // each position emits the pattern ids accepted in the new state, reporting the
    // match END offset. The whole GPU-OOM segmentation rests on ONE claim: scanning
    // each window from state 0 over `[scan_start, emit_end)` and emitting only
    // matches whose END is in `[emit_start, emit_end)` yields the EXACT same
    // (pattern, end) set as one dense full-buffer scan — provided the warm-up
    // `overlap` is at least the longest pattern. This models that DFA with a
    // multi-pattern Aho-Corasick over short byte literals and proves the claim on
    // real automata (not just the offset tiling), so the WGSL kernel can mirror it.

    /// Minimal Aho-Corasick byte DFA: `goto`/`fail`/`out` over a literal set.
    struct AcDfa {
        goto: Vec<[i32; 256]>, // -1 = no edge
        fail: Vec<usize>,
        out: Vec<Vec<usize>>, // pattern ids accepted at this state
        max_len: u32,
    }

    impl AcDfa {
        fn build(patterns: &[&[u8]]) -> Self {
            let mut goto = vec![[-1i32; 256]];
            let mut out: Vec<Vec<usize>> = vec![Vec::new()];
            let mut max_len = 0u32;
            for (pid, pat) in patterns.iter().enumerate() {
                max_len = max_len.max(pat.len() as u32);
                let mut s = 0usize;
                for &b in pat.iter() {
                    let nx = goto[s][b as usize];
                    if nx == -1 {
                        let new = goto.len();
                        goto.push([-1i32; 256]);
                        out.push(Vec::new());
                        goto[s][b as usize] = new as i32;
                        s = new;
                    } else {
                        s = nx as usize;
                    }
                }
                out[s].push(pid);
            }
            // BFS failure links.
            let mut fail = vec![0usize; goto.len()];
            let mut queue = std::collections::VecDeque::new();
            for b in 0..256 {
                let t = goto[0][b];
                if t > 0 {
                    fail[t as usize] = 0;
                    queue.push_back(t as usize);
                } else if t == -1 {
                    goto[0][b] = 0;
                }
            }
            while let Some(r) = queue.pop_front() {
                for b in 0..256 {
                    let t = goto[r][b];
                    if t == -1 {
                        continue;
                    }
                    let t = t as usize;
                    queue.push_back(t);
                    let mut f = fail[r];
                    while goto[f][b] == -1 {
                        f = fail[f];
                    }
                    fail[t] = goto[f][b] as usize;
                    let merged = out[fail[t]].clone();
                    out[t].extend(merged);
                }
            }
            AcDfa {
                goto,
                fail,
                out,
                max_len,
            }
        }

        /// Step the failure-function DFA: from `state`, consume `byte`.
        fn step(&self, mut state: usize, byte: u8) -> usize {
            while self.goto[state][byte as usize] == -1 {
                state = self.fail[state];
            }
            self.goto[state][byte as usize] as usize
        }
    }

    /// Dense full-buffer scan: (pattern_id, end_offset) for every match.
    fn dense_scan(dfa: &AcDfa, text: &[u8]) -> BTreeSet<(usize, usize)> {
        let mut hits = BTreeSet::new();
        let mut state = 0usize;
        for (i, &b) in text.iter().enumerate() {
            state = dfa.step(state, b);
            for &pid in &dfa.out[state] {
                hits.insert((pid, i + 1)); // end = i+1
            }
        }
        hits
    }

    /// Segmented scan: exactly what the GPU kernel will do — each window scans
    /// `[scan_start, emit_end)` from state 0 and emits only matches whose END is
    /// in `[emit_start, emit_end)`.
    fn segmented_scan(
        dfa: &AcDfa,
        text: &[u8],
        seg_len: u32,
        overlap: u32,
    ) -> BTreeSet<(usize, usize)> {
        let mut hits = BTreeSet::new();
        for seg in plan_segments(&[text.len() as u32], seg_len, overlap) {
            let mut state = 0usize;
            for i in seg.scan_start..seg.emit_end {
                state = dfa.step(state, text[i as usize]);
                let end = i + 1;
                if end > seg.emit_start && end <= seg.emit_end {
                    for &pid in &dfa.out[state] {
                        hits.insert((pid, end as usize));
                    }
                }
            }
        }
        hits
    }

    /// Materialize an [`AcDfa`] into the dense `state * 256 + byte -> next_state`
    /// transition table the device (and [`dfa_sync_distance`]) consume, mirroring
    /// `BatchRuleProgram`. Returns `(transitions, state_count)`.
    fn materialize_dense(dfa: &AcDfa) -> (Vec<u32>, u32) {
        let n = dfa.goto.len();
        let mut transitions = vec![0u32; n * 256];
        for s in 0..n {
            for b in 0..256usize {
                transitions[s * 256 + b] = dfa.step(s, b as u8) as u32;
            }
        }
        (transitions, n as u32)
    }

    /// Brute-force δ*: run the dense table from `start` over `w`.
    fn run_dense(transitions: &[u32], start: usize, w: &[u8]) -> usize {
        let mut s = start;
        for &b in w {
            s = transitions[s * 256 + b as usize] as usize;
        }
        s
    }

    #[test]
    fn sync_distance_single_byte_literal_is_one() {
        // Unanchored "a": after one byte both runs agree (saw 'a' or didn't).
        let dfa = AcDfa::build(&[b"a"]);
        let (transitions, n) = materialize_dense(&dfa);
        assert_eq!(dfa_sync_distance(&transitions, n), Some(1));
    }

    #[test]
    fn sync_distance_two_byte_literal_is_two() {
        // Unanchored "ab": the pair {start, saw-a} only resolves after the 2nd
        // byte (hand-traced in the dfa_sync_distance doc reasoning).
        let dfa = AcDfa::build(&[b"ab"]);
        let (transitions, n) = materialize_dense(&dfa);
        assert_eq!(dfa_sync_distance(&transitions, n), Some(2));
    }

    #[test]
    fn sync_distance_trivial_single_state_is_zero() {
        // One state, every byte self-loops: all start states already coincide.
        let transitions = vec![0u32; 256];
        assert_eq!(dfa_sync_distance(&transitions, 1), Some(0));
    }

    #[test]
    fn sync_distance_unbounded_gap_pattern_is_none() {
        // "a.*b": state 0 (no 'a' yet), 1 (seen 'a', waiting 'b'), 2 (matched).
        // The pair {0,1} self-loops on any byte that is neither 'a' nor 'b'
        // (0→0, 1→1) — an off-diagonal cycle — so the 'a' and 'b' can be
        // arbitrarily far apart and the rule has infinite memory.
        let mut t = vec![0u32; 3 * 256];
        for b in 0..256usize {
            t[0 * 256 + b] = 0; // stay at start
            t[1 * 256 + b] = 1; // stay "seen a"
            t[2 * 256 + b] = 2; // absorbing accept
        }
        t[0 * 256 + b'a' as usize] = 1;
        t[1 * 256 + b'b' as usize] = 2;
        assert_eq!(dfa_sync_distance(&t, 3), None);
    }

    #[test]
    fn sync_distance_parity_dfa_is_none() {
        // Even/odd count of 'a': {even,odd} maps to {odd,even} on 'a' — an
        // off-diagonal 2-cycle — so the start state is never forgotten.
        let mut t = vec![0u32; 2 * 256];
        for b in 0..256usize {
            t[0 * 256 + b] = 0;
            t[1 * 256 + b] = 1;
        }
        t[0 * 256 + b'a' as usize] = 1;
        t[1 * 256 + b'a' as usize] = 0;
        assert_eq!(dfa_sync_distance(&t, 2), None);
    }

    #[test]
    fn sync_class_distinguishes_cycle_from_bounded() {
        // The diagnostic split the catalog builder relies on: a genuinely
        // unbounded DFA classifies as `UnboundedCycle` (NEVER segmentable,
        // regardless of budget), while a bounded literal classifies as
        // `Bounded(d)` with the exact distance. `BudgetExceeded` is the third,
        // distinct arm (a larger budget might still prove it bounded) — the
        // builder logs all three separately so budget-capping is never conflated
        // with true infinite memory.

        // "a.*b" off-diagonal self-loop ⇒ UnboundedCycle, not BudgetExceeded.
        let mut gap = vec![0u32; 3 * 256];
        for b in 0..256usize {
            gap[b] = 0;
            gap[256 + b] = 1;
            gap[512 + b] = 2;
        }
        gap[b'a' as usize] = 1;
        gap[256 + b'b' as usize] = 2;
        assert_eq!(dfa_sync_class(&gap, 3), SyncClass::UnboundedCycle);

        // Two-byte literal "ab" ⇒ Bounded(2) (matches the wrapper's value).
        let dfa = AcDfa::build(&[b"ab"]);
        let (transitions, n) = materialize_dense(&dfa);
        assert_eq!(dfa_sync_class(&transitions, n), SyncClass::Bounded(2));
        assert_eq!(dfa_sync_class(&transitions, n).bounded(), Some(2));

        // Single-state DFA ⇒ Bounded(0) (already synchronized).
        assert_eq!(dfa_sync_class(&vec![0u32; 256], 1), SyncClass::Bounded(0));
    }

    /// Build a `BatchRuleProgram` from a literal-set AC DFA, for catalog tests.
    fn rule_from_patterns(
        rule_idx: u32,
        pats: &[&[u8]],
    ) -> vyre_runtime::megakernel::BatchRuleProgram {
        let dfa = AcDfa::build(pats);
        let (transitions, n) = materialize_dense(&dfa);
        let mut accept = vec![0u32; n as usize];
        for s in 0..n as usize {
            if !dfa.out[s].is_empty() {
                accept[s] = 1;
            }
        }
        vyre_runtime::megakernel::BatchRuleProgram::new(rule_idx, transitions, accept, n)
            .expect("materialized AC DFA is a valid rule program")
    }

    #[test]
    fn catalog_sync_overlap_is_max_over_rules() {
        // "ab" needs overlap 2, "abc" needs 3 ⇒ the catalog needs 3.
        let r_ab = rule_from_patterns(0, &[b"ab"]);
        let r_abc = rule_from_patterns(1, &[b"abc"]);
        assert_eq!(catalog_sync_overlap(&[r_ab, r_abc]), Some(3));
        // Empty catalog: nothing to warm up.
        assert_eq!(catalog_sync_overlap(&[]), Some(0));
    }

    #[test]
    fn catalog_sync_overlap_is_none_if_any_rule_unbounded() {
        // A bounded rule plus one infinite-memory "a.*b" rule ⇒ the whole catalog
        // cannot be segmented (the caller must scan whole-file, fail-safe).
        let r_ok = rule_from_patterns(0, &[b"ab"]);
        let mut t_bad = vec![0u32; 3 * 256];
        for b in 0..256usize {
            t_bad[1 * 256 + b] = 1;
            t_bad[2 * 256 + b] = 2;
        }
        t_bad[0 * 256 + b'a' as usize] = 1;
        t_bad[1 * 256 + b'b' as usize] = 2;
        let r_bad = vyre_runtime::megakernel::BatchRuleProgram::new(1, t_bad, vec![0, 0, 1], 3)
            .expect("valid infinite-memory rule program");
        assert_eq!(catalog_sync_overlap(&[r_ok, r_bad]), None);
    }

    proptest! {
        /// THE soundness link: for a random multi-pattern AC DFA, the computed
        /// synchronization distance `O` is a SUFFICIENT warm-up — a segmented
        /// scan with `overlap = O` produces the EXACT dense match set for ANY
        /// segment width. This is what licenses tuning `seg_len` below the file
        /// length: `dfa_sync_distance` tells the host the minimum overlap that
        /// keeps segmentation exact. (AC DFAs over bounded literals always have
        /// finite memory, so `O` is always `Some`.)
        #[test]
        fn sync_distance_overlap_makes_segmentation_exact(
            patterns in proptest::collection::vec(
                proptest::collection::vec(b'a'..=b'd', 1..=6), 1..=4),
            text in proptest::collection::vec(b'a'..=b'd', 0..400),
            seg_len in 1u32..64,
        ) {
            let pat_refs: Vec<&[u8]> = patterns.iter().map(|p| p.as_slice()).collect();
            let dfa = AcDfa::build(&pat_refs);
            let (transitions, n) = materialize_dense(&dfa);
            let sync = dfa_sync_distance(&transitions, n)
                .expect("a bounded-literal AC DFA has finite memory");
            // The distance can never exceed the longest pattern (the AC state
            // depends only on that many trailing bytes).
            prop_assert!(sync <= dfa.max_len, "sync {} > max_len {}", sync, dfa.max_len);
            prop_assert_eq!(
                segmented_scan(&dfa, &text, seg_len, sync),
                dense_scan(&dfa, &text),
                "overlap = sync_distance({}) must make seg_len={} exact", sync, seg_len
            );
        }

        /// Directly the synchronization property the bound promises: after `O`
        /// common bytes, the state is independent of the (reachable) start state.
        #[test]
        fn sync_distance_converges_from_every_reachable_start(
            patterns in proptest::collection::vec(
                proptest::collection::vec(b'a'..=b'c', 1..=5), 1..=3),
            tail in proptest::collection::vec(b'a'..=b'c', 0..6),
            prefix in proptest::collection::vec(b'a'..=b'c', 0..8),
        ) {
            let pat_refs: Vec<&[u8]> = patterns.iter().map(|p| p.as_slice()).collect();
            let dfa = AcDfa::build(&pat_refs);
            let (transitions, n) = materialize_dense(&dfa);
            let sync = dfa_sync_distance(&transitions, n).expect("finite memory") as usize;
            // q = any state reachable by reading `prefix` from the start; w is a
            // string of length >= sync. Reading w from state 0 and from state q
            // must land in the same state.
            let q = run_dense(&transitions, 0, &prefix);
            let mut w = tail.clone();
            while w.len() < sync {
                w.push(b'a');
            }
            prop_assert_eq!(
                run_dense(&transitions, 0, &w),
                run_dense(&transitions, q, &w),
                "states diverged after sync={} common bytes (|w|={})", sync, w.len()
            );
        }
    }

    #[test]
    fn segmented_scan_matches_dense_on_a_known_case() {
        let dfa = AcDfa::build(&[b"aws", b"key", b"secret"]);
        let text = b"my aws key is secret and the aws secret key follows";
        // overlap >= max pattern len (6) guarantees parity.
        assert_eq!(
            segmented_scan(&dfa, text, 4, 6),
            dense_scan(&dfa, text),
            "segmented scan must equal dense scan with adequate warm-up"
        );
    }

    #[test]
    fn tiles_a_single_file_contiguously() {
        // 1000 bytes, owned width 256, warm-up 64 -> 4 windows tiling [0,1000).
        let segs = plan_segments(&[1000], 256, 64);
        assert_eq!(segs.len(), 4);
        assert_eq!(
            segs,
            vec![
                Segment {
                    file_idx: 0,
                    scan_start: 0,
                    emit_start: 0,
                    emit_end: 256
                },
                Segment {
                    file_idx: 0,
                    scan_start: 192,
                    emit_start: 256,
                    emit_end: 512
                },
                Segment {
                    file_idx: 0,
                    scan_start: 448,
                    emit_start: 512,
                    emit_end: 768
                },
                Segment {
                    file_idx: 0,
                    scan_start: 704,
                    emit_start: 768,
                    emit_end: 1000
                },
            ]
        );
        // Warm-up never reaches below 0, and every window scans at least its
        // owned region plus (up to) `overlap` bytes of context.
        assert_eq!(segs[0].scan_len(), 256); // first window has no room to warm up
        assert_eq!(segs[1].scan_len(), 256 + 64);
    }

    #[test]
    fn short_file_is_one_window_covering_everything() {
        let segs = plan_segments(&[100], 512, 64);
        assert_eq!(
            segs,
            vec![Segment {
                file_idx: 0,
                scan_start: 0,
                emit_start: 0,
                emit_end: 100
            }]
        );
    }

    #[test]
    fn zero_length_file_yields_no_window() {
        assert!(plan_segments(&[0], 256, 64).is_empty());
        // ...and is skipped between real files without shifting their indices.
        let segs = plan_segments(&[10, 0, 10], 256, 0);
        assert_eq!(
            segs.iter().map(|s| s.file_idx).collect::<Vec<_>>(),
            vec![0, 2]
        );
    }

    #[test]
    fn overlap_zero_means_scan_equals_emit() {
        let segs = plan_segments(&[800], 256, 0);
        for s in &segs {
            assert_eq!(s.scan_start, s.emit_start);
            assert_eq!(s.scan_len(), s.emit_len());
        }
    }

    #[test]
    fn segment_count_matches_planned_len() {
        let lens = [0u32, 1, 255, 256, 257, 4096, 8 * 1024 * 1024];
        assert_eq!(
            segment_count(&lens, 512),
            plan_segments(&lens, 512, 64).len() as u64
        );
    }

    #[test]
    fn segment_table_flattens_planned_segments_in_order() {
        let lens = [1000u32, 100];
        let segs = plan_segments(&lens, 256, 64);
        let table = segment_table(&lens, 256, 64);
        // One row of SEGMENT_WORDS per planned segment, same order.
        assert_eq!(table.len(), segs.len() * SEGMENT_WORDS);
        for (i, seg) in segs.iter().enumerate() {
            let row = &table[i * SEGMENT_WORDS..(i + 1) * SEGMENT_WORDS];
            assert_eq!(row, seg.abi_words(), "segment {i} ABI words mismatch");
            // Decode order is exactly [file_idx, scan_start, emit_start, emit_end].
            assert_eq!(row[0], seg.file_idx);
            assert_eq!(row[1], seg.scan_start);
            assert_eq!(row[2], seg.emit_start);
            assert_eq!(row[3], seg.emit_end);
        }
    }

    #[test]
    fn segment_table_first_row_is_file0_offset0() {
        // The first window of the first file always owns offset 0 with no warm-up.
        let table = segment_table(&[4096], 512, 64);
        assert_eq!(&table[..SEGMENT_WORDS], &[0, 0, 0, 512]);
    }

    proptest! {
        /// THE core soundness proof of the whole GPU-OOM approach, on REAL
        /// automata: over a random multi-pattern AC DFA and random text, a
        /// segmented scan with `overlap >= max_pattern_len` produces the EXACT
        /// same (pattern, end) match set as a dense full-buffer scan, for ANY
        /// segment width. A failure here means the kernel's emit-guard / warm-up
        /// would drop or duplicate a real match.
        #[test]
        fn segmented_scan_equals_dense_with_adequate_overlap(
            // 1-4 patterns of 1-6 bytes over a small alphabet (so matches are dense).
            patterns in proptest::collection::vec(
                proptest::collection::vec(b'a'..=b'd', 1..=6), 1..=4),
            text in proptest::collection::vec(b'a'..=b'd', 0..400),
            seg_len in 1u32..64,
            extra_overlap in 0u32..8,
        ) {
            let pat_refs: Vec<&[u8]> = patterns.iter().map(|p| p.as_slice()).collect();
            let dfa = AcDfa::build(&pat_refs);
            // Adequate warm-up = longest pattern (+ slack); this is the kernel's
            // contract (overlap is sized from the catalog's max pattern length).
            let overlap = dfa.max_len + extra_overlap;
            prop_assert_eq!(
                segmented_scan(&dfa, &text, seg_len, overlap),
                dense_scan(&dfa, &text),
                "segmented (seg_len={}, overlap={}) != dense", seg_len, overlap
            );
        }

        /// The flat table is exactly the planned segments' ABI words concatenated,
        /// and every emit_start/emit_end pair stays within its file length — the
        /// device decode can never read past the packed haystack for that file.
        #[test]
        fn segment_table_rows_are_in_bounds(
            lens in proptest::collection::vec(0u32..4000, 0..5),
            seg_len in 1u32..512,
            overlap in 0u32..128,
        ) {
            let table = segment_table(&lens, seg_len, overlap);
            prop_assert_eq!(table.len() % SEGMENT_WORDS, 0);
            for row in table.chunks_exact(SEGMENT_WORDS) {
                let (file_idx, scan_start, emit_start, emit_end) =
                    (row[0], row[1], row[2], row[3]);
                let file_len = lens[file_idx as usize];
                prop_assert!(scan_start <= emit_start);
                prop_assert!(emit_start < emit_end);
                prop_assert!(emit_end <= file_len, "row reads past file end");
            }
        }
    }

    proptest! {
        /// THE soundness oracle: per file, the windows' emit ranges tile
        /// `[0, file_len)` exactly — start at 0, end at len, contiguous, gap-free,
        /// disjoint — and each window's scan range is its emit range widened by
        /// exactly `min(overlap, emit_start)` of warm-up. A regression here breaks
        /// recall (a gap drops matches) or precision (an overlap double-counts).
        #[test]
        fn emit_ranges_tile_each_file_exactly(
            lens in proptest::collection::vec(0u32..5000, 0..6),
            seg_len in 1u32..1024,
            overlap in 0u32..256,
        ) {
            let segs = plan_segments(&lens, seg_len, overlap);
            for (file_idx, &len) in lens.iter().enumerate() {
                let fsegs: Vec<&Segment> =
                    segs.iter().filter(|s| s.file_idx == file_idx as u32).collect();

                if len == 0 {
                    prop_assert!(fsegs.is_empty(), "zero-length file must yield no window");
                    continue;
                }

                prop_assert_eq!(fsegs.first().unwrap().emit_start, 0, "first window must own offset 0");
                prop_assert_eq!(fsegs.last().unwrap().emit_end, len, "last window must reach file end");

                let mut cursor = 0u32;
                for s in &fsegs {
                    // contiguous + gap-free + disjoint emit tiling
                    prop_assert_eq!(s.emit_start, cursor, "gap or overlap between windows");
                    prop_assert!(s.emit_end > s.emit_start, "empty owned range");
                    prop_assert!(s.emit_end <= len, "window owns past file end");
                    // warm-up: scan starts exactly `min(overlap, emit_start)` earlier,
                    // clamped at 0, and the scan range ends at the emit end.
                    prop_assert_eq!(s.scan_start, s.emit_start.saturating_sub(overlap));
                    prop_assert!(s.scan_start <= s.emit_start, "warm-up cannot start after owned region");
                    prop_assert!(s.scan_len() >= s.emit_len(), "scan must cover the owned region");
                    cursor = s.emit_end;
                }
                prop_assert_eq!(cursor, len, "emit ranges must cover [0,len) with no remainder");

                // window count == ceil(len / seg_len)
                prop_assert_eq!(fsegs.len() as u32, len.div_ceil(seg_len));
            }
        }

        /// Every byte offset in `[0, file_len)` is owned by exactly one window
        /// (the dual of gap-free+disjoint, asserted pointwise on small files).
        #[test]
        fn every_offset_owned_exactly_once(
            len in 1u32..600,
            seg_len in 1u32..200,
            overlap in 0u32..128,
        ) {
            let segs = plan_segments(&[len], seg_len, overlap);
            for pos in 0..len {
                let owners = segs
                    .iter()
                    .filter(|s| pos >= s.emit_start && pos < s.emit_end)
                    .count();
                prop_assert_eq!(owners, 1, "offset {} owned by {} windows, expected 1", pos, owners);
            }
        }
    }

    // ---- Production-path COMBINED multi-pattern AC segmented oracle ----
    //
    // The oracle above (`segmented_scan` over the local `AcDfa`) proves the
    // offset tiling + emit-guard are sound for a model automaton. This second
    // oracle proves the SAME tiling is sound for the *production* combined
    // automaton the GPU megakernel will actually run: ONE `classic_ac_compile`
    // `CompiledDfa` over ALL patterns (dense `state*256+byte` transitions + the
    // flat `output_offsets`/`output_records` multi-emit), NOT N per-rule DFAs.
    //
    // This is the soundness contract the COMBINED-automaton kernel mirrors: a
    // single linear pass (`classic_ac_scan`) over the whole buffer must produce
    // the EXACT same `(pattern_id, end)` set as the union over `plan_segments`
    // windows, each walked from state 0 over `[scan_start, emit_end)` and
    // emitting only matches whose end byte index lies in `[emit_start, emit_end)`,
    // provided `overlap >= max_pattern_len`. Collapsing the geometry from
    // `(seg, rule)` to `(seg)` + this multi-emit is what removes the per-rule
    // brute-force multiplier that makes the megakernel lose to Hyperscan on a
    // literal-rich catalog (docs/GPU_OOM_SEGMENTATION.md END-TO-END finding).

    /// Combined-AC segmented scan over the production `CompiledDfa`. `end` is the
    /// 0-based byte index where the match ends — the SAME convention as
    /// [`classic_ac_scan`] (NOT the `i+1` convention of the model `segmented_scan`
    /// above), so the two are directly comparable.
    fn combined_segmented_scan(
        ac: &ClassicAcAutomaton,
        text: &[u8],
        seg_len: u32,
        overlap: u32,
    ) -> BTreeSet<(u32, u32)> {
        let dfa = &ac.dfa;
        let mut hits = BTreeSet::new();
        for seg in plan_segments(&[text.len() as u32], seg_len, overlap) {
            let mut state = 0u32;
            for i in seg.scan_start..seg.emit_end {
                state = dfa.transitions[(state as usize) * DFA_BYTE_COLUMNS + text[i as usize] as usize];
                // The loop bound already enforces `i < emit_end`; the window owns
                // a match ending at byte index `i` iff `i >= emit_start`. Bytes in
                // the `[scan_start, emit_start)` warm-up prefix advance state but
                // emit nothing, so windows tile the file with no miss / no dup.
                if i >= seg.emit_start {
                    let begin = dfa.output_offsets[state as usize] as usize;
                    let end = dfa.output_offsets[state as usize + 1] as usize;
                    for &pattern_id in &dfa.output_records[begin..end] {
                        hits.insert((pattern_id, i));
                    }
                }
            }
        }
        hits
    }

    #[test]
    fn combined_segmented_equals_linear_on_overlapping_patterns() {
        // he/she/his/hers on "ushers" — the canonical multi-emit case (one accept
        // state emits both "he" and "she" via failure links). A combined segmented
        // scan with overlap >= max_pattern_len must reproduce the full linear set.
        let ac = classic_ac_compile(&[b"he", b"she", b"his", b"hers"]);
        let text = b"ushers";
        let overlap = ac.dfa.max_pattern_len; // 4 ("hers"/"she")
        let linear: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, text).into_iter().collect();
        // Cross-check the linear oracle itself first (guards against a silent
        // regression in classic_ac_scan). In "ushers" (u0 s1 h2 e3 r4 s5),
        // classic_ac_scan reports the END byte index: "she"(pid1) and "he"(pid0)
        // both end at e=index 3; "hers"(pid3) ends at the final s=index 5.
        assert!(linear.contains(&(1, 3)), "linear must contain she@3: {linear:?}");
        assert!(linear.contains(&(0, 3)), "linear must contain he@3: {linear:?}");
        assert!(linear.contains(&(3, 5)), "linear must contain hers@5: {linear:?}");
        // Every small segment width must reproduce the linear set exactly.
        for seg_len in 1u32..=8 {
            let segmented = combined_segmented_scan(&ac, text, seg_len, overlap);
            assert_eq!(
                segmented, linear,
                "combined segmented (seg_len={seg_len}, overlap={overlap}) != linear classic_ac_scan"
            );
        }
    }

    proptest! {
        /// THE soundness proof for the COMBINED-automaton megakernel path: over a
        /// random multi-pattern set and random text, the production combined AC
        /// (`classic_ac_compile`) scanned in `plan_segments` windows with
        /// `overlap >= max_pattern_len` and the `output_records` multi-emit
        /// produces EXACTLY the linear `classic_ac_scan` `(pattern_id, end)` set,
        /// for ANY segment width. This is what licenses collapsing the kernel
        /// geometry from `(seg, rule)` (N per-rule passes) to `(seg)` (one combined
        /// pass) without dropping or duplicating a single match.
        #[test]
        fn combined_segmented_equals_linear_classic_ac_scan(
            patterns in proptest::collection::vec(
                proptest::collection::vec(b'a'..=b'd', 1..=6), 1..=5),
            text in proptest::collection::vec(b'a'..=b'd', 0..400),
            seg_len in 1u32..64,
            extra_overlap in 0u32..8,
        ) {
            let pat_refs: Vec<&[u8]> = patterns.iter().map(|p| p.as_slice()).collect();
            let ac = classic_ac_compile(&pat_refs);
            // The combined automaton's AC synchronization distance is <= its
            // longest pattern, so max_pattern_len (+ slack) is a sufficient warm-up.
            let overlap = ac.dfa.max_pattern_len + extra_overlap;
            let linear: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, &text).into_iter().collect();
            let segmented = combined_segmented_scan(&ac, &text, seg_len, overlap);
            prop_assert_eq!(
                segmented, linear,
                "combined segmented (seg_len={}, overlap={}) != linear", seg_len, overlap
            );
        }
    }
}
