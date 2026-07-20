//! NFA → DFA subset construction.
//!
//! Lowers a lane-major NFA bit-table (the shape `compile_regex_set` /
//! `nfa_scan_with_plan` emit) into the dense `state * 256 + byte → next_state`
//! [`CompiledDfa`] the [`crate::matching::dfa_compile`] family also produces.
//!
//! # Why this lives here
//!
//! Two GPU scan kernels exist in vyre-libs today:
//!
//! * `classic_ac_bounded_ranges_program` - consumes [`CompiledDfa`], does ONE
//!   transition-table load per input byte (`transitions[state * 256 + byte]`).
//!   O(1) per byte regardless of state count.
//! * `nfa_scan_with_plan` - consumes the lane-major NFA bit-table, walks a
//!   bit-vector state with ~LANES² subgroup_shuffle steps per byte. Necessary
//!   when an NFA cannot be subset-constructed under budget (state explosion),
//!   but expensive per byte.
//!
//! Regex sets (`compile_regex_set`) emit the second shape. For pattern sets
//! whose subset construction stays under a reasonable state cap, lowering to
//! the dense DFA lets the regex run through the dense kernel instead - same
//! throughput as a literal AC scan. This primitive is the bridge.
//!
//! # Algorithm
//!
//! Textbook subset construction. A DFA state is the set of NFA states the
//! automaton could be in. Start = ε-closure({entry NFA state}). For each DFA
//! state D, for each byte b: collect all NFA targets of (s, b) for s ∈ D,
//! take ε-closure, deduplicate against existing DFA states. Termination is
//! bounded by the caller-supplied state cap.
//!
//! Accept metadata: a DFA state accepts if any NFA state in its set is an
//! accept. `output_records[state]` enumerates every pattern_id whose accept
//! state is in the set, preserving multi-match semantics.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::hash::fnv1a::{fnv1a64_initial_state, fnv1a64_update_byte};
use crate::matching::dfa_compile::CompiledDfa;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::matching::nfa_to_dfa";

/// Lanes-per-subgroup the lane-major NFA tables are laid out for.
///
/// Contractually equal to `vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP`
/// (= 32). Hard-coded here so `matching::nfa_to_dfa` can compile without
/// the `feature = "nfa"` gate - this primitive only consumes the
/// lane-major bit-table layout, it doesn't invoke the NFA scan kernel.
/// The `layout_matches_nfa_module` test asserts the equality so a future
/// change in `subgroup_nfa::LANES_PER_SUBGROUP` produces a CI failure
/// here, not a silent layout mismatch at runtime.
const LANES: usize = 32;

/// Width of one NFA state-set bitset, in u32 words. `LANES × 32` bit
/// positions per word × bits per state covers the
/// `MAX_STATES_PER_SUBGROUP = 1024` cap.
const STATE_BITSET_WORDS: usize = LANES;

/// Per-NFA-state-set bitset. Bit `(lane * 32 + i)` set ⇔ NFA state
/// `lane * 32 + i` is live in this set.
type StateSet = [u32; STATE_BITSET_WORDS];

const EMPTY_SET: StateSet = [0u32; STATE_BITSET_WORDS];
/// Why a subset construction couldn't complete.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NfaToDfaError {
    /// Subset construction would create more than the caller-supplied
    /// `max_dfa_states` DFA states. State explosion has hit; the
    /// caller's options are raise the cap, shard the pattern set, or
    /// fall back to the NFA scan kernel.
    StateExplosion {
        /// Number of DFA states discovered before the cap was hit.
        produced: usize,
        /// Cap the caller passed.
        cap: usize,
    },
    /// One of the input bit-tables had a length inconsistent with the
    /// declared `num_states`.
    ShapeMismatch {
        /// Which table failed the length cross-check.
        reason: &'static str,
    },
}

impl fmt::Display for NfaToDfaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateExplosion { produced, cap } => write!(
                formatter,
                "NFA→DFA subset construction exceeded the {cap}-state cap after producing {produced} DFA states. Fix: raise the cap, shard the pattern set, or dispatch via the NFA scan kernel."
            ),
            Self::ShapeMismatch { reason } => {
                write!(formatter, "NFA bit-table shape mismatch: {reason}.")
            }
        }
    }
}

impl Error for NfaToDfaError {}

/// Caller-supplied NFA bit-tables, in the exact layout
/// `vyre_libs::scan::compile_regex_set` and `nfa_scan_with_plan` emit.
///
/// `transition_table[src * 256 * LANES + byte * LANES + lane]` is the
/// u32 bitmask of NFA states (`lane * 32 + i` for `i ∈ 0..32`) reachable
/// from `src` on `byte`.
///
/// `epsilon_table[src * LANES + lane]` is the same shape minus the byte
/// dimension.
///
/// `accept_state_ids[i]` is the NFA state that fires accept index `i`;
/// `accept_pattern_ids[i]` is the consumer's pattern id for that accept.
/// `max_pattern_len` is the max accepted match length and propagates
/// straight onto [`CompiledDfa::max_pattern_len`].
#[derive(Debug, Clone)]
pub struct NfaTables<'tables> {
    /// NFA state count.
    pub num_states: u32,
    /// Lane-major `[num_states × 256 × LANES]` u32 byte-transition table.
    pub transition_table: &'tables [u32],
    /// Lane-major `[num_states × LANES]` u32 epsilon-transition table.
    pub epsilon_table: &'tables [u32],
    /// One entry per accept; the NFA state id that accepts.
    pub accept_state_ids: &'tables [u32],
    /// One entry per accept; the consumer's pattern id for that accept.
    /// Must be the same length as `accept_state_ids`.
    pub accept_pattern_ids: &'tables [u32],
    /// Max match length over the pattern set. Forwarded to
    /// `CompiledDfa::max_pattern_len`; consumers (e.g. AC scan kernels
    /// with per-position replay windows) use it to bound work.
    pub max_pattern_len: u32,
}

/// Compile an NFA into the dense [`CompiledDfa`] via subset construction.
///
/// `max_dfa_states` is a hard cap on output state count - exceeding it
/// returns [`NfaToDfaError::StateExplosion`] rather than ballooning
/// memory. Typical regex sets (literal-ish + bounded character classes
/// + bounded repetition) produce a small constant multiple of the input
/// NFA state count; pathological alternations of large classes can blow
/// up exponentially and need either a higher cap or to stay on the NFA
/// scan path.
///
/// # Errors
/// * [`NfaToDfaError::ShapeMismatch`] if input table lengths disagree
///   with `num_states`.
/// * [`NfaToDfaError::StateExplosion`] when the cap is exceeded.
pub fn nfa_to_dfa(
    tables: &NfaTables<'_>,
    max_dfa_states: usize,
) -> Result<CompiledDfa, NfaToDfaError> {
    let n = tables.num_states as usize;
    if n > LANES * 32 {
        return Err(NfaToDfaError::ShapeMismatch {
            reason: "num_states exceeds LANES * 32 bit-set capacity",
        });
    }
    // Cap max_dfa_states to u32::MAX so DFA state IDs stored as u32 never wrap.
    // State IDs are used as array indices (transitions, accept, output_offsets),
    // so wrapping would alias existing states and corrupt the automaton silently.
    if max_dfa_states > u32::MAX as usize {
        return Err(NfaToDfaError::StateExplosion {
            produced: 0,
            cap: max_dfa_states,
        });
    }
    if tables.transition_table.len() != n * 256 * LANES {
        return Err(NfaToDfaError::ShapeMismatch {
            reason: "transition_table length != num_states * 256 * LANES",
        });
    }
    if tables.epsilon_table.len() != n * LANES {
        return Err(NfaToDfaError::ShapeMismatch {
            reason: "epsilon_table length != num_states * LANES",
        });
    }
    if tables.accept_state_ids.len() != tables.accept_pattern_ids.len() {
        return Err(NfaToDfaError::ShapeMismatch {
            reason: "accept_state_ids and accept_pattern_ids length disagree",
        });
    }
    // Validate every accept_state_id is within [0, num_states). Without this,
    // a state id >= num_states reaches test_bit() with lane = state/32 that
    // indexes outside the [u32; LANES] StateSet array, causing an OOB panic
    // instead of a structured ShapeMismatch error.
    for &id in tables.accept_state_ids {
        if id >= n as u32 {
            return Err(NfaToDfaError::ShapeMismatch {
                reason: "accept_state_ids entry >= num_states",
            });
        }
    }
    // accept[state] encodes the accepting pattern id as `pid + 1` (0 = no match),
    // so a pattern id of u32::MAX would wrap to 0 and silently hide the match.
    // Reject it upfront as a structured error rather than panicking deep in subset
    // construction (mirrors the accept_state_ids bound check above).
    for &pid in tables.accept_pattern_ids {
        if pid == u32::MAX {
            return Err(NfaToDfaError::ShapeMismatch {
                reason: "accept_pattern_ids entry u32::MAX cannot be encoded as pid+1; pattern ids must be <= u32::MAX - 1",
            });
        }
    }

    // Per-NFA-state ε-closure, precomputed once. Subset construction
    // looks up `epsilon_closure[s]` for every state in every byte step,
    // so the BFS cost stays bounded by num_states rather than reused
    // per DFA-state-transition.
    let epsilon_closures = build_epsilon_closures(n, tables.epsilon_table);

    // DFA state 0 = ε-closure of NFA entry state 0. Same convention as
    // `compile_regex_set`, where state 0 is the shared entry.
    let mut entry_set = EMPTY_SET;
    set_bit(&mut entry_set, 0);
    let start_set = closure_of_set(&entry_set, &epsilon_closures);

    let mut dfa_state_index: HashMap<StateSet, u32> = HashMap::new();
    let mut dfa_state_sets: Vec<StateSet> = Vec::new();
    let mut transitions: Vec<u32> = Vec::new();

    dfa_state_index.insert(start_set, 0);
    dfa_state_sets.push(start_set);
    transitions.extend(std::iter::repeat_n(0u32, 256));

    // Worklist-driven BFS over DFA states. We push the start state, then
    // for each unprocessed DFA state expand its 256 byte transitions -
    // adding any newly-discovered DFA state to the worklist. Stops when
    // every produced state has had its transitions filled in.
    let mut next_to_process: usize = 0;
    while next_to_process < dfa_state_sets.len() {
        let dfa_state_id = next_to_process;
        let current_set = dfa_state_sets[dfa_state_id];
        next_to_process += 1;

        for byte in 0u32..256 {
            let mut target_set = EMPTY_SET;
            // Walk only live NFA states in `current_set`; for each, OR
            // in the lane-major transition row for this byte. The row
            // already encodes "which states does s reach on b" so the
            // result is the union of byte-targets across all live s.
            for_each_set_bit(&current_set, |src_state| {
                let row_start = (src_state as usize) * 256 * LANES + (byte as usize) * LANES;
                for lane in 0..LANES {
                    target_set[lane] |= tables.transition_table[row_start + lane];
                }
            });
            // ε-close the union. Most NFA frontends connect alternation
            // / repetition via ε edges, so this step is what stitches
            // the pattern's full state graph back together.
            let closed = closure_of_set(&target_set, &epsilon_closures);
            let next_dfa_state = if closed == EMPTY_SET {
                // Reject - convention: state 0 is the start state and is
                // not a sink, so we model rejection as "stay at a dead
                // state". Allocate one dead state lazily the first time
                // it's needed.
                ensure_dead_state(
                    &mut dfa_state_index,
                    &mut dfa_state_sets,
                    &mut transitions,
                    max_dfa_states,
                )?
            } else if let Some(&existing) = dfa_state_index.get(&closed) {
                existing
            } else {
                if dfa_state_sets.len() >= max_dfa_states {
                    return Err(NfaToDfaError::StateExplosion {
                        produced: dfa_state_sets.len(),
                        cap: max_dfa_states,
                    });
                }
                // Safe: max_dfa_states <= u32::MAX (guarded at function entry), and
                // the StateExplosion guard above ensures len < max_dfa_states <= u32::MAX.
                let new_id = dfa_state_sets.len() as u32;
                dfa_state_index.insert(closed, new_id);
                dfa_state_sets.push(closed);
                transitions.extend(std::iter::repeat_n(0u32, 256));
                new_id
            };
            transitions[(dfa_state_id) * 256 + byte as usize] = next_dfa_state;
        }
    }

    // Accept + output_records: for each DFA state, walk every NFA accept
    // and emit the consumer's pattern_id if that accept's NFA state is
    // in the DFA state's bitset. Stable order in `accept_state_ids` →
    // stable order in `output_records` slice per state, which matches
    // the contract `dfa_compile` exposes.
    // Safe: max_dfa_states <= u32::MAX (guarded at function entry), and
    // dfa_state_sets.len() <= max_dfa_states at this point.
    let state_count = dfa_state_sets.len() as u32;
    let mut accept: Vec<u32> = vec![0; state_count as usize];
    let mut output_offsets: Vec<u32> = Vec::with_capacity(state_count as usize + 1);
    let mut output_records: Vec<u32> = Vec::new();
    output_offsets.push(0);
    for dfa_state_id in 0..state_count {
        let set = &dfa_state_sets[dfa_state_id as usize];
        let mut first_accept_pid: Option<u32> = None;
        for (i, &nfa_state) in tables.accept_state_ids.iter().enumerate() {
            if test_bit(set, nfa_state) {
                let pid = tables.accept_pattern_ids[i];
                if first_accept_pid.is_none() {
                    first_accept_pid = Some(pid);
                }
                output_records.push(pid);
            }
        }
        // accept[state] encodes the first accepting pattern id as `pid + 1` so that 0
        // means "no match". pid == u32::MAX (which would wrap to 0 and silently hide the
        // match) was already rejected with a structured ShapeMismatch at function entry,
        // so `pid + 1` cannot overflow here.
        accept[dfa_state_id as usize] = first_accept_pid.map(|pid| pid + 1).unwrap_or(0);
        // output_records.len() as u32: safe because max_dfa_states <= u32::MAX (guarded at
        // function entry), and each DFA state contributes at most num_states accept records,
        // so output_records.len() <= dfa_state_sets.len() * num_states <= u32::MAX * 1024.
        // That exceeds u32::MAX, so use a checked conversion to catch overflow.
        let output_offset = u32::try_from(output_records.len()).unwrap_or_else(|_| {
            panic!(
                "output_records length {} overflowed u32; the GPU kernel indexes output_records \
                 via u32 offsets so this would produce corrupt pattern ids. Fix: shard the \
                 pattern set or reduce num_states before calling nfa_to_dfa.",
                output_records.len()
            )
        });
        output_offsets.push(output_offset);
    }

    Ok(CompiledDfa {
        transitions,
        accept,
        state_count,
        max_pattern_len: tables.max_pattern_len,
        output_offsets,
        output_records,
    })
}

/// Stable content fingerprint for a compiled dense DFA.
///
/// The fingerprint intentionally covers only wire-relevant DFA fields, not
/// allocation identity or capacity. Scan pipelines can use it as a
/// content-addressed dedup key: identical automata compiled in different
/// places hash to the same value, while changes to transitions, accept
/// metadata, output records, state count, or maximum match length perturb the
/// key.
#[must_use]
pub fn dfa_fingerprint(dfa: &CompiledDfa) -> u64 {
    let mut hash = fnv1a64_initial_state();
    hash_u32(&mut hash, dfa.state_count);
    hash_u32(&mut hash, dfa.max_pattern_len);
    hash_u32_slice(&mut hash, &dfa.transitions);
    hash_u32_slice(&mut hash, &dfa.accept);
    hash_u32_slice(&mut hash, &dfa.output_offsets);
    hash_u32_slice(&mut hash, &dfa.output_records);
    hash
}

/// Wire-relevant byte size of a compiled dense DFA.
#[must_use]
pub fn dfa_wire_bytes(dfa: &CompiledDfa) -> usize {
    std::mem::size_of::<u32>()
        * (2 + dfa.transitions.len()
            + dfa.accept.len()
            + dfa.output_offsets.len()
            + dfa.output_records.len())
}

/// Result of inserting a DFA into a content-addressed dedup table.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DfaDedupResult {
    /// Stable content fingerprint for the DFA.
    pub fingerprint: u64,
    /// Canonical slot in the dedup table.
    pub canonical_index: usize,
    /// True when the DFA was not already present.
    pub inserted: bool,
}

/// Summary for a batch DFA canonicalization pass.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct DfaDedupStats {
    /// Number of input DFA plans submitted in this batch.
    pub input_count: usize,
    /// Number of DFA plans inserted as new canonical entries.
    pub inserted_count: usize,
    /// Number of input DFA plans resolved to existing canonical entries.
    pub duplicate_count: usize,
    /// Total number of canonical DFA plans retained after the batch.
    pub table_len_after: usize,
    /// Total wire-relevant bytes submitted in this batch.
    pub input_wire_bytes: usize,
    /// Wire-relevant bytes inserted as new canonical DFA plans in this batch.
    pub inserted_wire_bytes: usize,
    /// Wire-relevant bytes saved by resolving duplicates to canonical plans.
    pub saved_wire_bytes: usize,
}

/// Result of batch canonicalizing DFA plans.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct DfaDedupBatch {
    /// One canonicalization result per submitted DFA, in input order.
    pub results: Vec<DfaDedupResult>,
    /// Aggregate batch statistics.
    pub stats: DfaDedupStats,
}

impl DfaDedupBatch {
    /// Saved wire bytes in parts-per-million of submitted wire bytes.
    ///
    /// PPM keeps this metric deterministic and allocation-free, avoiding float
    /// drift across platforms while still giving planners a compact reuse
    /// efficiency signal.
    #[must_use]
    pub fn saved_wire_ppm(&self) -> u32 {
        saved_wire_ppm(self.stats.saved_wire_bytes, self.stats.input_wire_bytes)
    }
}

/// Collision-safe content-addressed table for compiled dense DFAs.
///
/// The fingerprint is a fast stable key, not a uniqueness proof. Buckets keep
/// every canonical DFA sharing the same fingerprint and compare full DFA
/// content before deduplicating, so a hash collision cannot alias two distinct
/// automata.
#[derive(Debug, Default, Clone)]
pub struct DfaDedupTable {
    buckets: HashMap<u64, Vec<usize>>,
    entries: Vec<CompiledDfa>,
}

impl DfaDedupTable {
    /// Number of unique DFA plans retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the table holds no DFA plans.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Read a canonical DFA by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&CompiledDfa> {
        self.entries.get(index)
    }

    /// Wire-relevant bytes retained by the canonical DFA table.
    #[must_use]
    pub fn canonical_wire_bytes(&self) -> usize {
        self.entries
            .iter()
            .map(dfa_wire_bytes)
            .fold(0usize, usize::saturating_add)
    }

    /// Insert `dfa`, returning its stable canonical slot.
    pub fn insert(&mut self, dfa: CompiledDfa) -> DfaDedupResult {
        let fingerprint = dfa_fingerprint(&dfa);
        if let Some(bucket) = self.buckets.get(&fingerprint) {
            for &candidate in bucket {
                if self
                    .entries
                    .get(candidate)
                    .map(|existing| dfa_content_eq(existing, &dfa))
                    .unwrap_or(false)
                {
                    return DfaDedupResult {
                        fingerprint,
                        canonical_index: candidate,
                        inserted: false,
                    };
                }
            }
        }

        let canonical_index = self.entries.len();
        self.entries.push(dfa);
        self.buckets
            .entry(fingerprint)
            .or_default()
            .push(canonical_index);
        DfaDedupResult {
            fingerprint,
            canonical_index,
            inserted: true,
        }
    }

    /// Insert many DFA plans and return stable canonical indices in input order.
    ///
    /// This is the high-throughput path for scan compilers that emit many
    /// automata in one planning wave. It avoids each caller re-implementing
    /// duplicate accounting and makes dedup evidence explicit.
    pub fn insert_many<I>(&mut self, dfas: I) -> DfaDedupBatch
    where
        I: IntoIterator<Item = CompiledDfa>,
    {
        let mut results = Vec::new();
        let mut inserted_count = 0usize;
        let mut duplicate_count = 0usize;
        let mut input_wire_bytes = 0usize;
        let mut inserted_wire_bytes = 0usize;
        let mut saved_wire_bytes = 0usize;
        for dfa in dfas {
            let wire_bytes = dfa_wire_bytes(&dfa);
            input_wire_bytes = input_wire_bytes.saturating_add(wire_bytes);
            let result = self.insert(dfa);
            if result.inserted {
                inserted_count += 1;
                inserted_wire_bytes = inserted_wire_bytes.saturating_add(wire_bytes);
            } else {
                duplicate_count += 1;
                saved_wire_bytes = saved_wire_bytes.saturating_add(wire_bytes);
            }
            results.push(result);
        }
        DfaDedupBatch {
            stats: DfaDedupStats {
                input_count: results.len(),
                inserted_count,
                duplicate_count,
                table_len_after: self.len(),
                input_wire_bytes,
                inserted_wire_bytes,
                saved_wire_bytes,
            },
            results,
        }
    }

    /// Merge another canonical DFA table into this one.
    ///
    /// This is the cross-shard path: independent scan planners can build local
    /// canonical tables, then merge them into a global content-addressed table
    /// without recompiling automata. Returned results map each source-table
    /// canonical DFA, in source order, to this table's canonical slot.
    pub fn merge_from(&mut self, other: &DfaDedupTable) -> DfaDedupBatch {
        self.insert_many(other.entries.iter().cloned())
    }
}

fn saved_wire_ppm(saved_wire_bytes: usize, input_wire_bytes: usize) -> u32 {
    if input_wire_bytes == 0 {
        return 0;
    }
    let ppm = (saved_wire_bytes as u128).saturating_mul(1_000_000) / (input_wire_bytes as u128);
    u32::try_from(ppm).unwrap_or(u32::MAX)
}

fn dfa_content_eq(left: &CompiledDfa, right: &CompiledDfa) -> bool {
    left.state_count == right.state_count
        && left.max_pattern_len == right.max_pattern_len
        && left.transitions == right.transitions
        && left.accept == right.accept
        && left.output_offsets == right.output_offsets
        && left.output_records == right.output_records
}

fn hash_u32_slice(hash: &mut u64, values: &[u32]) {
    hash_u64(hash, values.len() as u64);
    for &value in values {
        hash_u32(hash, value);
    }
}

fn hash_u32(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        *hash = fnv1a64_update_byte(*hash, byte);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash = fnv1a64_update_byte(*hash, byte);
    }
}

// ── bitset helpers ────────────────────────────────────────────────────

#[inline]
fn set_bit(set: &mut StateSet, state: u32) {
    let lane = (state / 32) as usize;
    let bit = state % 32;
    set[lane] |= 1u32 << bit;
}

#[inline]
fn test_bit(set: &StateSet, state: u32) -> bool {
    let lane = (state / 32) as usize;
    let bit = state % 32;
    (set[lane] & (1u32 << bit)) != 0
}

fn for_each_set_bit(set: &StateSet, mut f: impl FnMut(u32)) {
    for (lane, &word) in set.iter().enumerate() {
        let mut w = word;
        while w != 0 {
            let bit = w.trailing_zeros();
            f((lane as u32) * 32 + bit);
            w &= w - 1;
        }
    }
}

// ── ε-closure ─────────────────────────────────────────────────────────

/// Per-state ε-closure: for each NFA state s, the set of NFA states
/// reachable from s via zero or more ε edges (including s itself).
fn build_epsilon_closures(num_states: usize, epsilon_table: &[u32]) -> Vec<StateSet> {
    let mut closures = vec![EMPTY_SET; num_states];
    // Seed each closure with the state itself + its direct ε successors,
    // then run BFS until no new states are added. Standard fixpoint.
    for state in 0..num_states {
        let mut closure = EMPTY_SET;
        set_bit(&mut closure, state as u32);
        let mut frontier_word = EMPTY_SET;
        for lane in 0..LANES {
            frontier_word[lane] = epsilon_table[state * LANES + lane];
        }
        // Union direct ε successors into the closure to seed BFS.
        for lane in 0..LANES {
            closure[lane] |= frontier_word[lane];
        }
        // BFS: walk every newly-added state, OR in its ε successors,
        // continue until the closure stops growing.
        loop {
            let mut next_frontier = EMPTY_SET;
            for_each_set_bit(&frontier_word, |s| {
                for lane in 0..LANES {
                    let bits = epsilon_table[(s as usize) * LANES + lane];
                    let new_bits = bits & !closure[lane];
                    next_frontier[lane] |= new_bits;
                }
            });
            if next_frontier == EMPTY_SET {
                break;
            }
            for lane in 0..LANES {
                closure[lane] |= next_frontier[lane];
            }
            frontier_word = next_frontier;
        }
        closures[state] = closure;
    }
    closures
}

/// ε-close a state set: union the precomputed per-state closures of
/// every live state in `set`.
fn closure_of_set(set: &StateSet, per_state_closures: &[StateSet]) -> StateSet {
    let mut out = EMPTY_SET;
    for_each_set_bit(set, |state| {
        let closure = &per_state_closures[state as usize];
        for lane in 0..LANES {
            out[lane] |= closure[lane];
        }
    });
    out
}

// ── dead state ────────────────────────────────────────────────────────

/// Lazily allocate a single dead state that self-loops on every byte.
/// Avoids consuming `max_dfa_states` budget when the NFA has no
/// rejecting paths but lets us address "no transition" uniformly.
fn ensure_dead_state(
    index: &mut HashMap<StateSet, u32>,
    sets: &mut Vec<StateSet>,
    transitions: &mut Vec<u32>,
    max_dfa_states: usize,
) -> Result<u32, NfaToDfaError> {
    if let Some(&existing) = index.get(&EMPTY_SET) {
        return Ok(existing);
    }
    if sets.len() >= max_dfa_states {
        return Err(NfaToDfaError::StateExplosion {
            produced: sets.len(),
            cap: max_dfa_states,
        });
    }
    // Safe: the StateExplosion guard above ensures sets.len() < max_dfa_states,
    // and max_dfa_states <= u32::MAX is guarded at nfa_to_dfa entry.
    let dead_id = sets.len() as u32;
    index.insert(EMPTY_SET, dead_id);
    sets.push(EMPTY_SET);
    // Self-loops: every byte stays at the dead state. Use the id we
    // just assigned (not 0 - 0 is the start state).
    transitions.extend(std::iter::repeat_n(dead_id, 256));
    Ok(dead_id)
}

// ── tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Lock the layout-constant invariant: our local LANES must equal
    /// `vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP`. The nfa
    /// module is feature-gated, so only run the check when both features
    /// (matching is implicit here; nfa is opt-in) are on.
    #[cfg(feature = "nfa")]
    #[test]
    fn layout_matches_nfa_module() {
        assert_eq!(
            LANES,
            crate::nfa::subgroup_nfa::LANES_PER_SUBGROUP,
            "nfa_to_dfa's local LANES must mirror subgroup_nfa::LANES_PER_SUBGROUP - a drift means the bit-table layout in this primitive no longer matches what `compile_regex_set` / `nfa_scan_with_plan` emit. Fix: update LANES here and re-run the matching test suite."
        );
    }

    /// Build NFA tables for a single literal pattern "abc" by hand.
    /// Mirrors what `crate::nfa::nfa::compile` (vyre-libs side) would
    /// produce: 4 states (entry + 3 byte states), no ε edges.
    fn literal_abc_tables() -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
        // 4 states: 0=entry, 1=after 'a', 2=after 'ab', 3=after 'abc' (accept)
        let num_states = 4_usize;
        let mut transition = vec![0u32; num_states * 256 * LANES];
        for (src, b, dst) in [(0usize, b'a', 1u32), (1, b'b', 2), (2, b'c', 3)] {
            let dst_lane = (dst / 32) as usize;
            let dst_bit = 1u32 << (dst % 32);
            let idx = src * 256 * LANES + (b as usize) * LANES + dst_lane;
            transition[idx] |= dst_bit;
        }
        let epsilon = vec![0u32; num_states * LANES];
        let accept_state_ids = vec![3u32];
        let accept_pattern_ids = vec![0u32];
        (transition, epsilon, accept_state_ids, accept_pattern_ids)
    }

    struct GeneratedNfa {
        num_states: u32,
        transition: Vec<u32>,
        epsilon: Vec<u32>,
        accept_states: Vec<u32>,
        accept_pids: Vec<u32>,
        max_pattern_len: u32,
        primary_word: Vec<u8>,
        alternate_word: Vec<u8>,
    }

    impl GeneratedNfa {
        fn tables(&self) -> NfaTables<'_> {
            NfaTables {
                num_states: self.num_states,
                transition_table: &self.transition,
                epsilon_table: &self.epsilon,
                accept_state_ids: &self.accept_states,
                accept_pattern_ids: &self.accept_pids,
                max_pattern_len: self.max_pattern_len,
            }
        }
    }

    fn generated_nfa(seed: u32) -> GeneratedNfa {
        let num_states = 3 + (seed as usize % 4);
        let mut transition = vec![0u32; num_states * 256 * LANES];
        let mut epsilon = vec![0u32; num_states * LANES];
        let mut primary_word = Vec::with_capacity(num_states.saturating_sub(1));

        for src in 0..num_states.saturating_sub(1) {
            let byte = generated_byte(seed, src as u32);
            primary_word.push(byte);
            add_transition(&mut transition, src, byte, (src + 1) as u32);
            if src > 0 {
                add_transition(
                    &mut transition,
                    src,
                    generated_byte(seed ^ 0x5a5a_1337, src as u32),
                    src as u32,
                );
            }
        }

        let alternate_target = num_states.saturating_sub(2).max(1);
        let alternate_byte = generated_byte(seed ^ 0x9e37_79b9, 0);
        add_transition(&mut transition, 0, alternate_byte, alternate_target as u32);
        let alternate_word = vec![alternate_byte];

        if seed % 3 == 0 && num_states > 3 {
            add_epsilon(&mut epsilon, 1, 2);
        }
        if seed % 5 == 0 {
            add_epsilon(&mut epsilon, 0, 1);
        }
        if seed % 11 == 0 && num_states > 4 {
            add_epsilon(&mut epsilon, 2, (num_states - 1) as u32);
        }

        let mut accept_states = vec![(num_states - 1) as u32];
        let mut accept_pids = vec![seed % 31];
        if seed % 7 == 0 {
            accept_states.push(alternate_target as u32);
            accept_pids.push(100 + (seed % 97));
        }

        GeneratedNfa {
            num_states: num_states as u32,
            transition,
            epsilon,
            accept_states,
            accept_pids,
            max_pattern_len: primary_word.len() as u32,
            primary_word,
            alternate_word,
        }
    }

    fn generated_byte(seed: u32, lane: u32) -> u8 {
        let mixed = seed
            .wrapping_mul(1_664_525)
            .wrapping_add(lane.wrapping_mul(1_013_904_223))
            .wrapping_add(0x45d9_f3b);
        b'a' + (mixed % 23) as u8
    }

    fn add_transition(table: &mut [u32], src: usize, byte: u8, dst: u32) {
        let lane = (dst / 32) as usize;
        let bit = 1u32 << (dst % 32);
        let idx = src * 256 * LANES + (byte as usize) * LANES + lane;
        table[idx] |= bit;
    }

    fn add_epsilon(table: &mut [u32], src: usize, dst: u32) {
        let lane = (dst / 32) as usize;
        let bit = 1u32 << (dst % 32);
        table[src * LANES + lane] |= bit;
    }

    fn nfa_outputs_for(nfa: &GeneratedNfa, input: &[u8]) -> Vec<u32> {
        let closures = build_epsilon_closures(nfa.num_states as usize, &nfa.epsilon);
        let mut current = EMPTY_SET;
        set_bit(&mut current, 0);
        current = closure_of_set(&current, &closures);

        for &byte in input {
            let mut target = EMPTY_SET;
            for_each_set_bit(&current, |src_state| {
                let row_start = (src_state as usize) * 256 * LANES + (byte as usize) * LANES;
                for lane in 0..LANES {
                    target[lane] |= nfa.transition[row_start + lane];
                }
            });
            current = closure_of_set(&target, &closures);
        }

        let mut out = Vec::new();
        for (idx, &state) in nfa.accept_states.iter().enumerate() {
            if test_bit(&current, state) {
                out.push(nfa.accept_pids[idx]);
            }
        }
        out
    }

    fn dfa_outputs_for(dfa: &CompiledDfa, input: &[u8]) -> Vec<u32> {
        let mut state = 0usize;
        for &byte in input {
            state = dfa.transitions[state * 256 + byte as usize] as usize;
        }
        let start = dfa.output_offsets[state] as usize;
        let end = dfa.output_offsets[state + 1] as usize;
        dfa.output_records[start..end].to_vec()
    }

    #[test]
    fn generated_nfa_to_dfa_matches_reference_nfa_for_thousands_of_inputs() {
        let mut checked = 0usize;
        for seed in 0..1024u32 {
            let nfa = generated_nfa(seed);
            let dfa = nfa_to_dfa(&nfa.tables(), 4096)
                .expect("Fix: generated sparse NFA must stay inside the DFA cap");
            let mut mutated_primary = nfa.primary_word.clone();
            if let Some(last) = mutated_primary.last_mut() {
                *last = last.wrapping_add(1);
            }
            let primary_prefix =
                nfa.primary_word[..nfa.primary_word.len().saturating_sub(1)].to_vec();
            let mut reversed_primary = nfa.primary_word.clone();
            reversed_primary.reverse();
            let generated_noise = vec![
                generated_byte(seed, 9),
                generated_byte(seed, 10),
                generated_byte(seed, 11),
                generated_byte(seed, 12),
            ];
            let alternate_twice =
                [nfa.alternate_word.as_slice(), nfa.alternate_word.as_slice()].concat();
            let corpus = [
                Vec::new(),
                nfa.primary_word.clone(),
                primary_prefix,
                reversed_primary,
                mutated_primary,
                nfa.alternate_word.clone(),
                alternate_twice,
                generated_noise.clone(),
                vec![generated_byte(seed ^ 0xa5a5_a5a5, 1)],
                [generated_byte(seed, 13)].into(),
                [generated_byte(seed, 14), generated_byte(seed, 15)].into(),
                [nfa.alternate_word.as_slice(), nfa.primary_word.as_slice()].concat(),
                [nfa.primary_word.as_slice(), nfa.alternate_word.as_slice()].concat(),
                [generated_byte(seed, 16)]
                    .into_iter()
                    .chain(nfa.primary_word.iter().copied())
                    .collect(),
                nfa.primary_word
                    .iter()
                    .copied()
                    .chain([generated_byte(seed, 17)])
                    .collect(),
                [
                    nfa.alternate_word.as_slice(),
                    &generated_noise,
                    nfa.primary_word.as_slice(),
                ]
                .concat(),
            ];
            for input in corpus {
                assert_eq!(
                    dfa_outputs_for(&dfa, &input),
                    nfa_outputs_for(&nfa, &input),
                    "seed {seed} input {input:?} must produce identical accept records"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 16_384);
    }

    #[test]
    fn generated_malformed_nfa_shapes_report_structured_errors() {
        let mut checked = 0usize;
        for seed in 0..1024u32 {
            let nfa = generated_nfa(seed);

            let mut short_transition = nfa.transition.clone();
            short_transition.pop();
            assert!(matches!(
                nfa_to_dfa(
                    &NfaTables {
                        transition_table: &short_transition,
                        ..nfa.tables()
                    },
                    4096,
                ),
                Err(NfaToDfaError::ShapeMismatch { .. })
            ));
            checked += 1;

            let mut short_epsilon = nfa.epsilon.clone();
            short_epsilon.pop();
            assert!(matches!(
                nfa_to_dfa(
                    &NfaTables {
                        epsilon_table: &short_epsilon,
                        ..nfa.tables()
                    },
                    4096,
                ),
                Err(NfaToDfaError::ShapeMismatch { .. })
            ));
            checked += 1;

            let mut extra_pid = nfa.accept_pids.clone();
            extra_pid.push(seed);
            assert!(matches!(
                nfa_to_dfa(
                    &NfaTables {
                        accept_pattern_ids: &extra_pid,
                        ..nfa.tables()
                    },
                    4096,
                ),
                Err(NfaToDfaError::ShapeMismatch { .. })
            ));
            checked += 1;

            assert!(matches!(
                nfa_to_dfa(
                    &NfaTables {
                        num_states: 1025,
                        ..nfa.tables()
                    },
                    4096,
                ),
                Err(NfaToDfaError::ShapeMismatch { .. })
            ));
            checked += 1;
        }
        assert_eq!(checked, 4096);
    }

    #[test]
    fn generated_dfa_fingerprints_are_stable_and_content_addressed() {
        let mut checked = 0usize;
        for seed in 0..1024u32 {
            let nfa = generated_nfa(seed);
            let first = match nfa_to_dfa(&nfa.tables(), 4096) {
                Ok(dfa) => dfa,
                Err(err) => panic!("Fix: generated NFA must lower for seed {seed}: {err}"),
            };
            let second = match nfa_to_dfa(&nfa.tables(), 4096) {
                Ok(dfa) => dfa,
                Err(err) => {
                    panic!("Fix: generated NFA must lower on replay for seed {seed}: {err}")
                }
            };
            assert_eq!(
                dfa_fingerprint(&first),
                dfa_fingerprint(&second),
                "seed {seed} must produce a stable content fingerprint"
            );

            let mut changed = first.clone();
            changed.max_pattern_len = changed.max_pattern_len.wrapping_add(1);
            assert_ne!(
                dfa_fingerprint(&first),
                dfa_fingerprint(&changed),
                "seed {seed} max-pattern metadata must perturb the fingerprint"
            );

            if let Some(first_transition) = changed.transitions.first_mut() {
                *first_transition = first_transition.wrapping_add(1);
                assert_ne!(
                    dfa_fingerprint(&first),
                    dfa_fingerprint(&changed),
                    "seed {seed} transition metadata must perturb the fingerprint"
                );
            }
            checked += 3;
        }
        assert_eq!(checked, 3072);
    }

    #[test]
    fn generated_dfa_dedup_table_canonicalizes_repeated_automata() {
        let mut table = DfaDedupTable::default();
        let mut checked = 0usize;
        for seed in 0..1024u32 {
            let nfa = generated_nfa(seed);
            let first = nfa_to_dfa(&nfa.tables(), 4096).unwrap_or_else(|err| {
                panic!("Fix: generated NFA must lower for seed {seed}: {err}")
            });
            let replay = nfa_to_dfa(&nfa.tables(), 4096).unwrap_or_else(|err| {
                panic!("Fix: generated NFA must lower on replay for seed {seed}: {err}")
            });

            let first_result = table.insert(first.clone());
            let replay_result = table.insert(replay);
            assert!(
                first_result.inserted,
                "seed {seed} first insert must create a canonical DFA"
            );
            assert!(
                !replay_result.inserted,
                "seed {seed} replay insert must deduplicate"
            );
            assert_eq!(
                first_result.canonical_index, replay_result.canonical_index,
                "seed {seed} replay must resolve to the first canonical slot"
            );
            assert_eq!(
                first_result.fingerprint, replay_result.fingerprint,
                "seed {seed} replay must keep the same content fingerprint"
            );

            let mut changed = first;
            changed.max_pattern_len = changed.max_pattern_len.wrapping_add(1);
            let changed_result = table.insert(changed);
            assert!(
                changed_result.inserted,
                "seed {seed} changed DFA metadata must not deduplicate"
            );
            assert_ne!(
                changed_result.canonical_index, first_result.canonical_index,
                "seed {seed} changed DFA must get a distinct canonical slot"
            );
            checked += 3;
        }
        assert_eq!(checked, 3072);
        assert_eq!(table.len(), 2048);
    }

    #[test]
    fn generated_dfa_batch_dedup_preserves_input_order_and_stats() {
        let mut table = DfaDedupTable::default();
        let mut input = Vec::new();
        for seed in 0..512u32 {
            let nfa = generated_nfa(seed);
            let dfa = nfa_to_dfa(&nfa.tables(), 4096).unwrap_or_else(|err| {
                panic!("Fix: generated NFA must lower for seed {seed}: {err}")
            });
            input.push(dfa.clone());
            input.push(dfa.clone());
            let mut changed = dfa;
            changed.max_pattern_len = changed.max_pattern_len.wrapping_add(1);
            input.push(changed);
        }

        let batch = table.insert_many(input);
        assert_eq!(batch.stats.input_count, 1536);
        assert_eq!(batch.stats.inserted_count, 1024);
        assert_eq!(batch.stats.duplicate_count, 512);
        assert_eq!(batch.stats.table_len_after, 1024);
        assert_eq!(
            batch.stats.input_wire_bytes,
            batch.stats.inserted_wire_bytes + batch.stats.saved_wire_bytes
        );
        assert_eq!(
            batch.stats.inserted_wire_bytes,
            table.canonical_wire_bytes()
        );
        assert!(
            batch.stats.saved_wire_bytes > 0,
            "batch dedup must report saved wire bytes for replayed automata"
        );
        assert!(
            batch.saved_wire_ppm() > 0,
            "batch dedup must report a nonzero deterministic saved-byte ratio"
        );
        assert_eq!(batch.results.len(), 1536);

        for chunk in batch.results.chunks_exact(3) {
            assert!(chunk[0].inserted);
            assert!(!chunk[1].inserted);
            assert!(chunk[2].inserted);
            assert_eq!(chunk[0].canonical_index, chunk[1].canonical_index);
            assert_ne!(chunk[0].canonical_index, chunk[2].canonical_index);
            assert_eq!(chunk[0].fingerprint, chunk[1].fingerprint);
            assert_ne!(chunk[0].fingerprint, chunk[2].fingerprint);
        }
    }

    #[test]
    fn generated_dfa_table_merge_deduplicates_cross_shard_plans() {
        let mut left = DfaDedupTable::default();
        let mut right = DfaDedupTable::default();
        for seed in 0..256u32 {
            let nfa = generated_nfa(seed);
            let dfa = nfa_to_dfa(&nfa.tables(), 4096).unwrap_or_else(|err| {
                panic!("Fix: generated NFA must lower for seed {seed}: {err}")
            });
            left.insert(dfa.clone());
            right.insert(dfa);
        }
        for seed in 256..512u32 {
            let nfa = generated_nfa(seed);
            let dfa = nfa_to_dfa(&nfa.tables(), 4096).unwrap_or_else(|err| {
                panic!("Fix: generated NFA must lower for seed {seed}: {err}")
            });
            right.insert(dfa);
        }

        let before_len = left.len();
        let before_bytes = left.canonical_wire_bytes();
        let batch = left.merge_from(&right);

        assert_eq!(before_len, 256);
        assert_eq!(batch.stats.input_count, 512);
        assert_eq!(batch.stats.inserted_count, 256);
        assert_eq!(batch.stats.duplicate_count, 256);
        assert_eq!(batch.stats.table_len_after, 512);
        assert!(batch.stats.saved_wire_bytes > 0);
        assert!(batch.saved_wire_ppm() > 0);
        assert!(left.canonical_wire_bytes() > before_bytes);
        assert_eq!(left.len(), 512);

        for result in batch.results.iter().take(256) {
            assert!(!result.inserted);
            assert!(result.canonical_index < before_len);
        }
        for result in batch.results.iter().skip(256) {
            assert!(result.inserted);
            assert!(result.canonical_index >= before_len);
        }
    }

    #[test]
    fn literal_pattern_lowers_to_acceptor_dfa() {
        let (transition, epsilon, accepts, pids) = literal_abc_tables();
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &accepts,
            accept_pattern_ids: &pids,
            max_pattern_len: 3,
        };
        let dfa = nfa_to_dfa(&tables, 1024).expect("Fix: literal NFA must lower cleanly");
        assert!(
            dfa.state_count >= 4,
            "literal 'abc' needs at least entry + 3 progress states; got {}",
            dfa.state_count
        );
        // Trace 'a' 'b' 'c' from state 0 and assert the final state accepts.
        let s_a = dfa.transitions[0 * 256 + b'a' as usize];
        let s_ab = dfa.transitions[(s_a as usize) * 256 + b'b' as usize];
        let s_abc = dfa.transitions[(s_ab as usize) * 256 + b'c' as usize];
        // The accept field encodes pid+1, so pattern 0 → value 1. Asserting
        // != 0 only proves some pattern accepted; asserting == 1 proves the
        // correct pattern id is stored. A pid transposition or the pid+1 wrap
        // bug (finding #3: pid=u32::MAX→0) would pass != 0 but fail == 1.
        assert_eq!(
            dfa.accept[s_abc as usize],
            1,
            "DFA state after 'abc' must accept pattern 0 encoded as pid+1=1; \
             got {} (0=no match, n=pattern n-1 accepted)",
            dfa.accept[s_abc as usize]
        );
        // Verify output_records carries the correct pid for the full-match path.
        let rec_start = dfa.output_offsets[s_abc as usize] as usize;
        let rec_end = dfa.output_offsets[s_abc as usize + 1] as usize;
        assert_eq!(
            &dfa.output_records[rec_start..rec_end],
            &[0u32],
            "output_records for the abc-accept state must contain exactly [0] (pid=0)"
        );
        // Negative twin: 'a' 'b' 'x' must not accept.
        let s_x = dfa.transitions[(s_ab as usize) * 256 + b'x' as usize];
        assert_eq!(
            dfa.accept[s_x as usize], 0,
            "'abx' is not 'abc' - must not accept"
        );
    }

    #[test]
    fn empty_input_returns_one_state_dfa_with_dead_self_loop() {
        // 1 NFA state, no transitions, no accepts. The DFA we get back
        // must still have at least the start state and a dead state
        // such that every byte from start lands on the dead state.
        let transition = vec![0u32; 1 * 256 * LANES];
        let epsilon = vec![0u32; 1 * LANES];
        let tables = NfaTables {
            num_states: 1,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &[],
            accept_pattern_ids: &[],
            max_pattern_len: 0,
        };
        let dfa = nfa_to_dfa(&tables, 16).expect("Fix: trivial NFA must lower");
        let dead = dfa.transitions[0 * 256 + b'a' as usize];
        assert_eq!(
            dfa.transitions[dead as usize * 256 + b'a' as usize],
            dead,
            "dead state must self-loop on every byte"
        );
        assert_eq!(dfa.accept[dead as usize], 0, "dead state must not accept");
    }

    #[test]
    fn state_explosion_reports_structured_error() {
        // Force the cap to 1 → any reachable byte produces state 2 and
        // hits the cap. We just need the error variant, not a specific
        // exploding pattern.
        let (transition, epsilon, accepts, pids) = literal_abc_tables();
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &accepts,
            accept_pattern_ids: &pids,
            max_pattern_len: 3,
        };
        let err = nfa_to_dfa(&tables, 1).expect_err("cap=1 must trip state explosion");
        match err {
            NfaToDfaError::StateExplosion { cap, produced } => {
                assert_eq!(cap, 1);
                assert!(produced >= 1);
            }
            other => panic!("expected StateExplosion, got {other:?}"),
        }
    }

    #[test]
    fn shape_mismatch_caught_before_construction() {
        // num_states=4 declared but transition table sized for 1 - the
        // shape guard must catch this without panicking inside the loop.
        let transition = vec![0u32; 1 * 256 * LANES];
        let epsilon = vec![0u32; 1 * LANES];
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &[],
            accept_pattern_ids: &[],
            max_pattern_len: 0,
        };
        let err = nfa_to_dfa(&tables, 16)
            .expect_err("declared num_states != table length must error, not panic");
        match err {
            NfaToDfaError::ShapeMismatch { reason } => {
                assert!(reason.contains("transition_table"));
            }
            other => panic!("expected ShapeMismatch, got {other:?}"),
        }
    }

    /// Finding #4 (P0): accept_state_ids entries are not validated to be < num_states.
    /// Before the fix, nfa_state=1024 produced lane=32 which is OOB on StateSet=[u32;32],
    /// causing a panic instead of a structured ShapeMismatch error.
    #[test]
    fn accept_state_id_ge_num_states_is_shape_mismatch() {
        let (transition, epsilon, _, _) = literal_abc_tables();
        // Declare 4 states but put accept_state_id=1024 (way out of range).
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &[1024],
            accept_pattern_ids: &[0],
            max_pattern_len: 3,
        };
        let result = nfa_to_dfa(&tables, 64);
        assert!(
            matches!(result, Err(NfaToDfaError::ShapeMismatch { .. })),
            "accept_state_id >= num_states must return ShapeMismatch, not panic; got {result:?}"
        );
    }

    /// Finding #4: also verify accept_state_id exactly equal to num_states is rejected.
    #[test]
    fn accept_state_id_equal_to_num_states_is_shape_mismatch() {
        let (transition, epsilon, _, _) = literal_abc_tables();
        // num_states=4, so valid ids are 0..3; id=4 is OOB.
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &[4],
            accept_pattern_ids: &[0],
            max_pattern_len: 3,
        };
        let result = nfa_to_dfa(&tables, 64);
        assert!(
            matches!(result, Err(NfaToDfaError::ShapeMismatch { .. })),
            "accept_state_id == num_states must be rejected; got {result:?}"
        );
    }

    /// Finding #2+8 (P0/P1): max_dfa_states > u32::MAX must return StateExplosion
    /// rather than allowing dfa_state_sets.len() to grow past u32::MAX and wrap
    /// the as u32 cast to 0, aliasing existing DFA states and corrupting the automaton.
    #[test]
    fn max_dfa_states_above_u32_max_returns_state_explosion() {
        let (transition, epsilon, accepts, pids) = literal_abc_tables();
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &accepts,
            accept_pattern_ids: &pids,
            max_pattern_len: 3,
        };
        // (u32::MAX as usize) + 1 is the first value that exceeds the u32 domain.
        let result = nfa_to_dfa(&tables, (u32::MAX as usize) + 1);
        assert!(
            matches!(result, Err(NfaToDfaError::StateExplosion { .. })),
            "max_dfa_states > u32::MAX must return StateExplosion before any state is allocated; \
             got {result:?}"
        );
    }

    /// Finding #3 (P0): the accept field encodes pid+1 so that 0 means no match.
    /// For pattern 0, the encoded value must be exactly 1, not some other nonzero value.
    /// This test replaces the existing decoration test that only checked != 0.
    #[test]
    fn literal_abc_accept_field_is_exactly_one() {
        let (transition, epsilon, accepts, pids) = literal_abc_tables();
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &accepts,
            accept_pattern_ids: &pids,
            max_pattern_len: 3,
        };
        let dfa = nfa_to_dfa(&tables, 1024).expect("Fix: literal NFA must lower cleanly");
        let s_a = dfa.transitions[b'a' as usize];
        let s_ab = dfa.transitions[s_a as usize * 256 + b'b' as usize];
        let s_abc = dfa.transitions[s_ab as usize * 256 + b'c' as usize];
        // Pattern 0 is encoded as pid+1 = 0+1 = 1. Any other nonzero value means
        // the wrong pattern id is reported on the fast-path field.
        assert_eq!(
            dfa.accept[s_abc as usize], 1,
            "pattern 0 must be encoded as pid+1=1 in the accept fast-path field; \
             got {} (accept field encodes pid+1, 0=no match)",
            dfa.accept[s_abc as usize]
        );
        // Also verify output_records carries the correct pid for the full-match path.
        let start = dfa.output_offsets[s_abc as usize] as usize;
        let end = dfa.output_offsets[s_abc as usize + 1] as usize;
        assert_eq!(
            &dfa.output_records[start..end],
            &[0u32],
            "output_records for the abc-accept state must contain exactly [0] (pid=0)"
        );
    }

    /// VL-002 (P0): a caller-supplied accept_pattern_id of u32::MAX cannot be encoded
    /// as pid+1 (it would wrap to 0, silently hiding the match). nfa_to_dfa must reject
    /// it with a structured ShapeMismatch at function entry, NOT panic deep in subset
    /// construction (the prior behaviour) and NOT silently encode 0.
    #[test]
    fn accept_pattern_id_u32_max_returns_shape_mismatch_not_panic() {
        let (transition, epsilon, accepts, _pids) = literal_abc_tables();
        let pids = vec![u32::MAX; accepts.len()];
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &accepts,
            accept_pattern_ids: &pids,
            max_pattern_len: 3,
        };
        let result = nfa_to_dfa(&tables, 1024);
        assert!(
            matches!(&result, Err(NfaToDfaError::ShapeMismatch { reason }) if reason.contains("u32::MAX")),
            "accept_pattern_ids=[u32::MAX] must return ShapeMismatch naming u32::MAX, got {result:?}"
        );
    }

    /// VL-002 negative twin: pid = u32::MAX - 1 is the largest ENCODABLE pattern id;
    /// it must lower cleanly and encode as accept = (u32::MAX - 1) + 1 = u32::MAX, with
    /// the raw pid preserved in output_records.
    #[test]
    fn accept_pattern_id_u32_max_minus_one_is_valid() {
        let (transition, epsilon, accepts, _pids) = literal_abc_tables();
        let pids = vec![u32::MAX - 1; accepts.len()];
        let tables = NfaTables {
            num_states: 4,
            transition_table: &transition,
            epsilon_table: &epsilon,
            accept_state_ids: &accepts,
            accept_pattern_ids: &pids,
            max_pattern_len: 3,
        };
        let dfa = nfa_to_dfa(&tables, 1024)
            .expect("Fix: pid=u32::MAX-1 is encodable and must lower cleanly");
        let s_a = dfa.transitions[b'a' as usize];
        let s_ab = dfa.transitions[s_a as usize * 256 + b'b' as usize];
        let s_abc = dfa.transitions[s_ab as usize * 256 + b'c' as usize];
        assert_eq!(
            dfa.accept[s_abc as usize],
            u32::MAX,
            "pid=u32::MAX-1 must encode as pid+1=u32::MAX in the accept fast-path field, got {}",
            dfa.accept[s_abc as usize]
        );
        let start = dfa.output_offsets[s_abc as usize] as usize;
        let end = dfa.output_offsets[s_abc as usize + 1] as usize;
        assert_eq!(
            &dfa.output_records[start..end],
            &[u32::MAX - 1],
            "output_records must carry the raw pid u32::MAX-1, not the encoded value"
        );
    }
}
