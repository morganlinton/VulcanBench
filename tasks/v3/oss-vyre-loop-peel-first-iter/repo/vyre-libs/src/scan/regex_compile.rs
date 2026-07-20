//! Regex AST → `NfaPlan` frontend.
//!
//! `nfa::compile` ships a literal-only NFA (one byte per state). This
//! module is its regex-aware counterpart: parse a regex string with
//! `regex-syntax`, lower the AST into a Thompson NFA over byte
//! transitions, emit the same `(NfaPlan, transition_table,
//! epsilon_table)` triple the literal compiler produces.
//!
//! # Why a separate module instead of widening `nfa::compile`
//!
//! The literal compiler is hot-path simple  -  every byte is a single
//! state. Bolting alternation / repetition / character classes onto it
//! would either bloat the literal path or fork the construction code.
//! The lego-block fix is a SECOND construction module that emits the
//! SAME output shape, so every downstream component (`nfa_scan`
//! Program, `mega_scan::build`, `RulePipeline`) works unmodified.
//!
//! # Supported regex subset
//!
//! Targets the ~85% of vyre's expected detector regex shapes:
//!
//!   - Concatenation (default)
//!   - Alternation `a|b`
//!   - Character classes `[abc]`, `[a-z]`, `[^abc]`
//!   - Builtin escapes `\d \D \w \W \s \S` (ASCII semantics)
//!   - Bounded repetition `*`, `+`, `?`, `{n}`, `{n,m}`
//!   - Text anchors `^` and `$`
//!   - Escape literals `\.`, `\\`, `\(`, `\[`
//!
//! Explicitly NOT supported (returns `RegexCompileError::Unsupported`):
//!
//!   - Backreferences `\1` (NFA cannot represent)
//!   - Word-boundary and line-boundary lookarounds
//!   - Unicode character classes outside the ASCII range

use regex_syntax::hir::{Class, Hir, HirKind, Look, Repetition};

use crate::scan::nfa::NfaPlan;

const LANES: usize = vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP;

/// Failure modes for [`compile_regex_set`]. Variants are non-exhaustive
/// so future regex features can be added without a breaking change.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RegexCompileError {
    /// `regex-syntax` rejected the pattern. Carries the parser's own
    /// diagnostic so callers can forward it.
    Parse {
        /// Index into the input slice that failed to parse.
        pattern_index: usize,
        /// `regex-syntax`'s error message.
        message: String,
    },
    /// The pattern uses a regex feature this GPU NFA frontend does not
    /// support. Callers must reject or rewrite the detector into supported
    /// GPU-NFA rule data.
    Unsupported {
        /// Index into the input slice that uses the unsupported feature.
        pattern_index: usize,
        /// One-line description of what isn't supported (e.g. "anchors").
        feature: &'static str,
    },
    /// The compiled NFA exceeds `LANES * 32` states (the lane-major
    /// transition table addresses states with one bit per lane).
    /// Mitigation: split the pattern set across multiple pipelines.
    TooManyStates {
        /// Number of states the AST would have produced.
        states: usize,
        /// Per-pipeline maximum.
        cap: usize,
    },
    /// Pattern count does not fit the GPU ABI's `u32` pattern id field.
    PatternCountOverflow {
        /// Number of patterns supplied by the caller.
        count: usize,
    },
    /// A compiled regex match length does not fit the `u32` match ABI.
    MatchLengthOverflow {
        /// Index into the input slice that produced the oversized match.
        pattern_index: usize,
        /// Longest matched byte length for the pattern.
        len: usize,
    },
    /// Transition or epsilon table word count overflowed host `usize`.
    TableWordCountOverflow {
        /// Table being built.
        table: &'static str,
    },
    /// Compiler staging allocation failed.
    StorageReserveFailed {
        /// Scratch vector being reserved.
        field: &'static str,
        /// Requested target capacity.
        requested: usize,
        /// Allocator failure details.
        message: String,
    },
}

impl std::fmt::Display for RegexCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse {
                pattern_index,
                message,
            } => write!(
                f,
                "regex_compile: pattern {pattern_index} parse error: {message}. \
                 Fix: review the regex syntax."
            ),
            Self::Unsupported {
                pattern_index,
                feature,
            } => write!(
                f,
                "regex_compile: pattern {pattern_index} uses unsupported feature `{feature}`. \
                 Fix: rewrite the detector into supported GPU-NFA syntax or split it into GPU-compatible rules."
            ),
            Self::TooManyStates { states, cap } => write!(
                f,
                "regex_compile: NFA needs {states} states; per-pipeline cap is {cap}. \
                 Fix: split the pattern set across multiple pipelines."
            ),
            Self::PatternCountOverflow { count } => write!(
                f,
                "regex_compile: pattern count {count} exceeds u32 capacity. Fix: shard the pattern set before GPU regex compilation."
            ),
            Self::MatchLengthOverflow {
                pattern_index,
                len,
            } => write!(
                f,
                "regex_compile: pattern {pattern_index} match length {len} exceeds u32 capacity. Fix: bound or shard the regex before GPU compilation."
            ),
            Self::TableWordCountOverflow { table } => write!(
                f,
                "regex_compile: {table} table word count overflows host usize. Fix: shard the regex pattern set before table construction."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "regex_compile: could not reserve {requested} {field} slot(s): {message}. Fix: shard the regex pattern set before GPU compilation."
            ),
        }
    }
}

impl std::error::Error for RegexCompileError {}

/// Output of [`compile_regex_set`]  -  same triple shape as the literal
/// `nfa::compile` returns plus the GPU side-tables `nfa::nfa_scan`
/// expects, so consumers can plug this into `RulePipeline` without
/// changing the dispatch path.
#[derive(Debug, Clone)]
pub struct CompiledRegexSet {
    /// State graph + accept-state metadata.
    pub plan: NfaPlan,
    /// Lane-major byte→bitset transition table:
    /// `[num_states × 256 × LANES_PER_SUBGROUP]` u32s.
    pub transition_table: Vec<u32>,
    /// Lane-major epsilon (free) transition table:
    /// `[num_states × LANES_PER_SUBGROUP]` u32s.
    pub epsilon_table: Vec<u32>,
}

const STATE_CAP: usize = LANES * 32;

/// Compile a list of regex strings into a single multimatch NFA.
///
/// # Errors
/// See [`RegexCompileError`].
pub fn compile_regex_set(patterns: &[&str]) -> Result<CompiledRegexSet, RegexCompileError> {
    let mut builder = NfaBuilder::new();
    let _pattern_count =
        u32::try_from(patterns.len()).map_err(|_| RegexCompileError::PatternCountOverflow {
            count: patterns.len(),
        })?;
    let mut accept_states = Vec::new();
    reserve_vec(&mut accept_states, patterns.len(), "accept state")?;
    let mut accept_state_ids = Vec::new();
    reserve_vec(&mut accept_state_ids, patterns.len(), "accept state id")?;
    let mut accept_start_anchored = Vec::new();
    reserve_vec(
        &mut accept_start_anchored,
        patterns.len(),
        "accept start-anchor flag",
    )?;
    let mut accept_end_anchored = Vec::new();
    reserve_vec(
        &mut accept_end_anchored,
        patterns.len(),
        "accept end-anchor flag",
    )?;
    let entry = builder.fresh_state()?; // shared entry state 0

    // Use the byte-oriented parser configuration: `unicode(false)` +
    // `utf8(false)` makes `\d` / `\w` / `\s` ASCII-only, which is what
    // this primitive's byte-state automaton can represent.
    // `regex_syntax::parse(pat)` defaults to Unicode classes that
    // explode into hundreds of byte ranges and trip our `> 0x7F` guard.
    for (pid, pat) in patterns.iter().enumerate() {
        // Two-phase parse: byte-mode first (keeps `\d`/`\w`/`\s` ASCII
        // so they don't explode into hundreds of Unicode codepoint
        // ranges), then unicode-mode as a fallback when the source
        // contains a non-ASCII codepoint inside a character class
        // (e.g. homoglyph-expanded `[hнһｈ]`). The unicode-mode HIR
        // gets the same `build_class` lowering - non-ASCII members
        // expand into UTF-8 byte-sequence alternations.
        let hir = match regex_syntax::ParserBuilder::new()
            .unicode(false)
            .utf8(false)
            .build()
            .parse(pat)
        {
            Ok(h) => h,
            Err(byte_mode_err) => regex_syntax::ParserBuilder::new()
                .unicode(true)
                .utf8(false)
                .build()
                .parse(pat)
                .map_err(|_unicode_err| RegexCompileError::Parse {
                    pattern_index: pid,
                    // Surface the byte-mode diagnostic since that's the
                    // narrow grammar the kernel actually supports; the
                    // unicode-mode retry exists only to widen the
                    // character-class path.
                    message: format!("{byte_mode_err}"),
                })?,
        };
        let (frag, anchors) = build_pattern_hir(&mut builder, &hir, pid)?;
        // Connect the shared entry to this pattern's start via epsilon.
        builder.add_epsilon(entry, frag.start);
        let pid_u32 = u32::try_from(pid).map_err(|_| RegexCompileError::PatternCountOverflow {
            count: patterns.len(),
        })?;
        let match_len_u32 =
            u32::try_from(frag.match_len).map_err(|_| RegexCompileError::MatchLengthOverflow {
                pattern_index: pid,
                len: frag.match_len,
            })?;
        accept_states.push((pid_u32, match_len_u32));
        accept_state_ids.push(frag.end);
        accept_start_anchored.push(anchors.start);
        accept_end_anchored.push(anchors.end);
    }

    if builder.state_count() > STATE_CAP {
        return Err(RegexCompileError::TooManyStates {
            states: builder.state_count(),
            cap: STATE_CAP,
        });
    }

    let plan = NfaPlan {
        num_states: u32::try_from(builder.state_count()).map_err(|_| {
            RegexCompileError::TooManyStates {
                states: builder.state_count(),
                cap: STATE_CAP,
            }
        })?,
        input_len: 0,
        accept_states,
        accept_state_ids,
        accept_start_anchored,
        accept_end_anchored,
    };
    let (transition_table, epsilon_table) = builder.emit_lane_major_tables()?;
    Ok(CompiledRegexSet {
        plan,
        transition_table,
        epsilon_table,
    })
}

/// Build a [`crate::scan::RulePipeline`] directly from regex
/// sources. Convenience for consumers who don't need the
/// `CompiledRegexSet` intermediate. `input_len` matches the contract
/// of `mega_scan::build` (haystack byte count the dispatch will scan).
///
/// # Errors
/// Forwards [`RegexCompileError`].
pub fn build_rule_pipeline_from_regex(
    patterns: &[&str],
    input_buf: &str,
    hit_buf: &str,
    input_len: u32,
) -> Result<crate::scan::RulePipeline, RegexCompileError> {
    let compiled = compile_regex_set(patterns)?;
    let has_epsilon = compiled.epsilon_table.iter().any(|word| *word != 0);
    let program = crate::scan::nfa::nfa_scan_with_plan(
        &compiled.plan,
        has_epsilon,
        input_buf,
        hit_buf,
        input_len,
    )
    .map_err(|_| RegexCompileError::TooManyStates {
        states: compiled.plan.num_states as usize,
        cap: STATE_CAP,
    })?;
    Ok(crate::scan::RulePipeline {
        program,
        transition_table: compiled.transition_table,
        epsilon_table: compiled.epsilon_table,
        plan: compiled.plan.for_input_len(input_len),
    })
}

// ---- Thompson NFA construction over byte transitions ----

#[derive(Debug)]
struct NfaBuilder {
    state_count: usize,
    /// Flat byte transitions. Emission consumes the stream directly,
    /// so construction does not need one allocation per NFA state.
    transitions: Vec<ByteTransition>,
    /// Flat epsilon (free) transitions.
    epsilons: Vec<(u32, u32)>,
}

#[derive(Debug, Clone)]
struct ByteTransition {
    src: u32,
    set: ByteSet,
    dst: u32,
}

#[derive(Debug, Clone)]
struct ByteSet {
    bits: [u64; 4], // 256 bits → 4 u64s
}

impl ByteSet {
    fn new() -> Self {
        Self { bits: [0; 4] }
    }
    fn insert(&mut self, b: u8) {
        self.bits[(b / 64) as usize] |= 1u64 << (b % 64);
    }
    fn from_byte(b: u8) -> Self {
        let mut s = Self::new();
        s.insert(b);
        s
    }
    fn from_range(lo: u8, hi: u8) -> Self {
        let mut s = Self::new();
        for b in lo..=hi {
            s.insert(b);
        }
        s
    }
    fn for_each_set_byte(&self, mut f: impl FnMut(u8)) {
        for (word_idx, &word) in self.bits.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                f((word_idx * 64 + bit) as u8);
                bits &= bits - 1;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Fragment {
    start: u32,
    end: u32,
    /// Sum of byte-steps along the longest path. Used as the
    /// `pattern_len` reported in `NfaPlan::accept_states`.
    match_len: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct PatternAnchors {
    start: bool,
    end: bool,
}

impl NfaBuilder {
    fn new() -> Self {
        Self {
            state_count: 0,
            transitions: Vec::new(),
            epsilons: Vec::new(),
        }
    }

    fn state_count(&self) -> usize {
        self.state_count
    }

    fn fresh_state(&mut self) -> Result<u32, RegexCompileError> {
        if self.state_count >= STATE_CAP {
            return Err(RegexCompileError::TooManyStates {
                states: self.state_count.saturating_add(1),
                cap: STATE_CAP,
            });
        }
        let state =
            u32::try_from(self.state_count).map_err(|_| RegexCompileError::TooManyStates {
                states: self.state_count,
                cap: STATE_CAP,
            })?;
        self.state_count =
            self.state_count
                .checked_add(1)
                .ok_or(RegexCompileError::TooManyStates {
                    states: usize::MAX,
                    cap: STATE_CAP,
                })?;
        Ok(state)
    }

    fn add_byte_transition(&mut self, src: u32, set: ByteSet, dst: u32) {
        self.transitions.push(ByteTransition { src, set, dst });
    }

    fn add_epsilon(&mut self, src: u32, dst: u32) {
        self.epsilons.push((src, dst));
    }

    /// Lane-major emission, matching the contract of
    /// `nfa::build_transition_table` + `build_epsilon_table`.
    fn emit_lane_major_tables(&self) -> Result<(Vec<u32>, Vec<u32>), RegexCompileError> {
        let n = self.state_count();
        let mut transitions = zeroed_u32_table(
            table_word_count(n, 256, "transition")?,
            "transition table word",
        )?;
        let mut epsilons =
            zeroed_u32_table(table_word_count(n, 1, "epsilon")?, "epsilon table word")?;

        for edge in &self.transitions {
            let src = edge.src as usize;
            let dst_lane = (edge.dst / 32) as usize;
            let dst_bit = 1u32 << (edge.dst % 32);
            edge.set.for_each_set_byte(|byte| {
                let idx = src * 256 * LANES + (byte as usize) * LANES + dst_lane;
                transitions[idx] |= dst_bit;
            });
        }
        for &(src, dst) in &self.epsilons {
            let dst_lane = (dst / 32) as usize;
            let dst_bit = 1u32 << (dst % 32);
            let idx = src as usize * LANES + dst_lane;
            epsilons[idx] |= dst_bit;
        }
        Ok((transitions, epsilons))
    }
}

fn table_word_count(
    states: usize,
    byte_columns: usize,
    table: &'static str,
) -> Result<usize, RegexCompileError> {
    states
        .checked_mul(byte_columns)
        .and_then(|words| words.checked_mul(LANES))
        .ok_or(RegexCompileError::TableWordCountOverflow { table })
}

fn zeroed_u32_table(words: usize, field: &'static str) -> Result<Vec<u32>, RegexCompileError> {
    let mut table = Vec::new();
    reserve_vec(&mut table, words, field)?;
    table.resize(words, 0);
    Ok(table)
}

fn reserve_vec<T>(
    vec: &mut Vec<T>,
    requested: usize,
    field: &'static str,
) -> Result<(), RegexCompileError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(vec, requested).map_err(|source| {
        RegexCompileError::StorageReserveFailed {
            field,
            requested,
            message: source.to_string(),
        }
    })
}

fn empty_fragment(b: &mut NfaBuilder) -> Result<Fragment, RegexCompileError> {
    let s = b.fresh_state()?;
    Ok(Fragment {
        start: s,
        end: s,
        match_len: 0,
    })
}

fn build_pattern_hir(
    b: &mut NfaBuilder,
    hir: &Hir,
    pid: usize,
) -> Result<(Fragment, PatternAnchors), RegexCompileError> {
    match hir.kind() {
        HirKind::Look(Look::Start) => Ok((
            empty_fragment(b)?,
            PatternAnchors {
                start: true,
                end: false,
            },
        )),
        HirKind::Look(Look::End) => Ok((
            empty_fragment(b)?,
            PatternAnchors {
                start: false,
                end: true,
            },
        )),
        HirKind::Concat(parts) => {
            let mut first = 0usize;
            let mut last = parts.len();
            let mut anchors = PatternAnchors::default();

            if first < last && is_text_start_look(&parts[first]) {
                anchors.start = true;
                first += 1;
            }
            if first < last && is_text_end_look(&parts[last - 1]) {
                anchors.end = true;
                last -= 1;
            }

            Ok((build_hir_slice(b, &parts[first..last], pid)?, anchors))
        }
        _ => Ok((build_hir(b, hir, pid)?, PatternAnchors::default())),
    }
}

fn is_text_start_look(hir: &Hir) -> bool {
    matches!(hir.kind(), HirKind::Look(Look::Start))
}

fn is_text_end_look(hir: &Hir) -> bool {
    matches!(hir.kind(), HirKind::Look(Look::End))
}

fn build_hir_slice(
    b: &mut NfaBuilder,
    parts: &[Hir],
    pid: usize,
) -> Result<Fragment, RegexCompileError> {
    let Some(first_part) = parts.first() else {
        return empty_fragment(b);
    };
    let mut acc = build_hir(b, first_part, pid)?;
    for child in &parts[1..] {
        let next = build_hir(b, child, pid)?;
        b.add_epsilon(acc.end, next.start);
        acc = Fragment {
            start: acc.start,
            end: next.end,
            match_len: acc.match_len + next.match_len,
        };
    }
    Ok(acc)
}

fn build_hir(b: &mut NfaBuilder, hir: &Hir, pid: usize) -> Result<Fragment, RegexCompileError> {
    match hir.kind() {
        HirKind::Empty => empty_fragment(b),
        HirKind::Literal(lit) => {
            // Each literal byte gets its own state.
            let start = b.fresh_state()?;
            let mut prev = start;
            for &byte in lit.0.iter() {
                let next = b.fresh_state()?;
                b.add_byte_transition(prev, ByteSet::from_byte(byte), next);
                prev = next;
            }
            Ok(Fragment {
                start,
                end: prev,
                match_len: lit.0.len(),
            })
        }
        HirKind::Class(cls) => build_class(b, cls, pid),
        HirKind::Repetition(rep) => build_repetition(b, rep, pid),
        HirKind::Concat(parts) => build_hir_slice(b, parts, pid),
        HirKind::Alternation(alts) => {
            // Diamond: shared fork → each branch → shared join.
            let fork = b.fresh_state()?;
            let join = b.fresh_state()?;
            let mut max_len = 0usize;
            for child in alts {
                let frag = build_hir(b, child, pid)?;
                b.add_epsilon(fork, frag.start);
                b.add_epsilon(frag.end, join);
                if frag.match_len > max_len {
                    max_len = frag.match_len;
                }
            }
            Ok(Fragment {
                start: fork,
                end: join,
                match_len: max_len,
            })
        }
        HirKind::Look(_) => Err(RegexCompileError::Unsupported {
            pattern_index: pid,
            feature: "non-edge lookaround assertion",
        }),
        HirKind::Capture(c) => {
            // We don't expose capture groups (NFA scan is multimatch,
            // not capture). Strip and recurse.
            build_hir(b, &c.sub, pid)
        }
    }
}

fn build_repetition(
    b: &mut NfaBuilder,
    rep: &Repetition,
    pid: usize,
) -> Result<Fragment, RegexCompileError> {
    let min = rep.min;
    let max = rep.max;

    // Keep pathological repetitions from materializing a giant transient NFA.
    // The final state cap is the source of truth, so oversized repetitions
    // report TooManyStates instead of pretending the syntax is unsupported.
    if let Some(m) = max {
        if m as usize > STATE_CAP {
            return Err(RegexCompileError::TooManyStates {
                states: m as usize,
                cap: STATE_CAP,
            });
        }
    }
    if min as usize > STATE_CAP {
        return Err(RegexCompileError::TooManyStates {
            states: min as usize,
            cap: STATE_CAP,
        });
    }

    // Build by unrolling: emit `min` copies, then either
    //   - a Kleene loop if max is None (`*` / `+`), OR
    //   - `max - min` optional copies if max is bounded.
    let start = b.fresh_state()?;
    let mut tail = start;
    let mut total_len = 0usize;

    for _ in 0..min {
        let frag = build_hir(b, &rep.sub, pid)?;
        b.add_epsilon(tail, frag.start);
        tail = frag.end;
        total_len += frag.match_len;
    }

    match max {
        None => {
            // Open-ended: insert a Kleene wrapper. tail → frag.start →
            // frag.end → tail (loop back) ; tail → join (skip).
            let join = b.fresh_state()?;
            let frag = build_hir(b, &rep.sub, pid)?;
            b.add_epsilon(tail, frag.start);
            b.add_epsilon(frag.end, frag.start); // loop
            b.add_epsilon(frag.end, join);
            b.add_epsilon(tail, join); // zero matches
            tail = join;
        }
        Some(m) => {
            for _ in min..m {
                let frag = build_hir(b, &rep.sub, pid)?;
                let join = b.fresh_state()?;
                b.add_epsilon(tail, frag.start);
                b.add_epsilon(frag.end, join);
                b.add_epsilon(tail, join); // skip this optional copy
                tail = join;
            }
        }
    }
    Ok(Fragment {
        start,
        end: tail,
        match_len: total_len,
    })
}

/// Lower a regex character class into an NFA fragment, taking the
/// single-byte fast path when the class fits in 0..=127 and the
/// UTF-8-alternation expansion path otherwise.
///
/// The single-byte path is identical to the original implementation:
/// one ByteSet, one transition, `match_len = 1`. The expansion path
/// emits one byte-chain fragment per codepoint (or per pre-existing
/// multi-byte range like `\u{0100}-\u{01FF}` enumerated codepoint-by-
/// codepoint) and ε-merges them via a shared end state.
///
/// `match_len` for the expansion case is the MAX byte length across
/// arms - anchored extraction uses `match_len` only to position
/// the post-process window, not to extract the credential text, and
/// over-sizing the window is harmless (the real regex re-extracts the
/// exact match inside it).
///
/// To keep state-budget worst case bounded, expansion is capped at
/// `MAX_CLASS_EXPANSION_CODEPOINTS = 256` enumerated codepoints (a
/// `[\u{0100}-\u{017F}]` Latin-Extended block sits at 128, which is
/// well within budget; a class spanning a full CJK block would refuse).
fn build_class(b: &mut NfaBuilder, cls: &Class, pid: usize) -> Result<Fragment, RegexCompileError> {
    if let Some(set) = try_class_as_ascii_byte_set(cls) {
        let start = b.fresh_state()?;
        let end = b.fresh_state()?;
        b.add_byte_transition(start, set, end);
        return Ok(Fragment {
            start,
            end,
            match_len: 1,
        });
    }
    let sequences = class_to_utf8_sequences(cls, pid)?;
    if sequences.is_empty() {
        return Err(RegexCompileError::Unsupported {
            pattern_index: pid,
            feature: "empty character class after Unicode expansion",
        });
    }
    let start = b.fresh_state()?;
    let end = b.fresh_state()?;
    let mut max_len = 1usize;
    for seq in &sequences {
        if seq.is_empty() {
            continue;
        }
        // Build a sequential chain start ε→ s0 -b0-> s1 -b1-> ... -bN-> end
        // for this UTF-8 byte sequence.
        let arm_start = b.fresh_state()?;
        b.add_epsilon(start, arm_start);
        let mut prev = arm_start;
        for &byte in seq {
            let next = b.fresh_state()?;
            b.add_byte_transition(prev, ByteSet::from_byte(byte), next);
            prev = next;
        }
        b.add_epsilon(prev, end);
        if seq.len() > max_len {
            max_len = seq.len();
        }
    }
    Ok(Fragment {
        start,
        end,
        match_len: max_len,
    })
}

/// Returns `Some(ByteSet)` when every member of the class fits in
/// 0..=127 (i.e. the original single-byte ASCII fast path). Otherwise
/// returns None so the caller takes the UTF-8 expansion path.
fn try_class_as_ascii_byte_set(cls: &Class) -> Option<ByteSet> {
    let mut out = ByteSet::new();
    match cls {
        Class::Bytes(byte_class) => {
            // Byte classes are already at the byte level - every member
            // is a u8, no codepoint expansion involved. The legacy fast
            // path always applies.
            for r in byte_class.iter() {
                let merged = ByteSet::from_range(r.start(), r.end());
                for w in 0..4 {
                    out.bits[w] |= merged.bits[w];
                }
            }
            Some(out)
        }
        Class::Unicode(uni) => {
            // ASCII-only fast path. The moment any range escapes
            // 0..=0x7F, fall through to UTF-8 expansion.
            for r in uni.iter() {
                if (r.end() as u32) > 0x7F {
                    return None;
                }
                let merged = ByteSet::from_range(r.start() as u8, r.end() as u8);
                for w in 0..4 {
                    out.bits[w] |= merged.bits[w];
                }
            }
            Some(out)
        }
    }
}

/// Cap on enumerated codepoints during UTF-8 expansion. A class like
/// `[\u{0100}-\u{017F}]` (Latin Extended-A) expands to 128 sequences,
/// well within the cap. A class spanning a full CJK block (~20 000
/// codepoints) would blow past it - the byte-state automaton can't
/// represent that cleanly, so the consumer should keep that pattern on
/// the CPU regex path.
const MAX_CLASS_EXPANSION_CODEPOINTS: usize = 256;

/// Enumerate every codepoint in `cls`, encode each into UTF-8, and
/// return the resulting `Vec<Vec<u8>>` so the caller can build an
/// alternation of byte-chain fragments. ASCII members come back as
/// 1-byte sequences; non-ASCII as 2-4 byte sequences.
fn class_to_utf8_sequences(cls: &Class, pid: usize) -> Result<Vec<Vec<u8>>, RegexCompileError> {
    let mut sequences: Vec<Vec<u8>> = Vec::new();
    let mut budget = MAX_CLASS_EXPANSION_CODEPOINTS;
    match cls {
        Class::Bytes(byte_class) => {
            for r in byte_class.iter() {
                for byte in r.start()..=r.end() {
                    if budget == 0 {
                        return Err(RegexCompileError::Unsupported {
                            pattern_index: pid,
                            feature: "byte character class exceeded expansion cap",
                        });
                    }
                    sequences.push(vec![byte]);
                    budget -= 1;
                }
            }
        }
        Class::Unicode(uni) => {
            for r in uni.iter() {
                let lo = r.start() as u32;
                let hi = r.end() as u32;
                for cp in lo..=hi {
                    if budget == 0 {
                        return Err(RegexCompileError::Unsupported {
                            pattern_index: pid,
                            feature: "unicode character class exceeded expansion cap",
                        });
                    }
                    // Use a small buffer + `char::encode_utf8` to avoid
                    // pulling in a heavyweight UTF-8 dependency. Invalid
                    // codepoints (surrogates) are silently skipped -
                    // regex-syntax shouldn't emit them in a parsed HIR
                    // for character classes, but the `char::from_u32`
                    // guard catches the corner case if it ever does.
                    if let Some(c) = char::from_u32(cp) {
                        let mut buf = [0u8; 4];
                        let encoded = c.encode_utf8(&mut buf);
                        sequences.push(encoded.as_bytes().to_vec());
                        budget -= 1;
                    }
                }
            }
        }
    }
    Ok(sequences)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn states_of(s: &str) -> u32 {
        compile_regex_set(&[s]).unwrap().plan.num_states
    }

    #[test]
    fn literal_compiles() {
        let r = compile_regex_set(&["abc"]).unwrap();
        // 1 entry + 1 literal-start + 3 letter states = 5
        assert_eq!(r.plan.num_states, 5);
        assert_eq!(r.plan.accept_states.len(), 1);
    }

    #[test]
    fn alternation_compiles() {
        let r = compile_regex_set(&["a|b"]).unwrap();
        // entry + fork + join + 2*(start + 1 byte) = 1+1+1+2+2 = 7
        // (exact count depends on builder; just sanity-check it's >0).
        assert!(r.plan.num_states > 0);
        assert_eq!(r.plan.accept_states.len(), 1);
    }

    #[test]
    fn class_compiles() {
        let r = compile_regex_set(&["[a-z]"]).unwrap();
        assert!(r.plan.num_states > 0);
        // Sanity: 26 lowercase bytes hit the same destination state.
        // We don't introspect the table here  -  just ensure it builds.
    }

    #[test]
    fn text_anchors_compile_to_accept_flags() {
        let r = compile_regex_set(&["^foo$"]).unwrap();
        assert_eq!(r.plan.accept_start_anchored, vec![true]);
        assert_eq!(r.plan.accept_end_anchored, vec![true]);
    }

    #[test]
    fn bounded_repetition_above_old_cap_compiles_under_state_cap() {
        let r = compile_regex_set(&["a{0,128}"]).unwrap();
        assert!(r.plan.num_states > 64);
        assert!(r.plan.num_states <= STATE_CAP as u32);
    }

    #[test]
    fn regex_compile_preserves_accept_metadata_through_checked_paths() {
        let r = compile_regex_set(&["a", "bc", "^de$"]).unwrap();

        assert_eq!(r.plan.accept_states, vec![(0, 1), (1, 2), (2, 2)]);
        assert_eq!(r.plan.accept_state_ids.len(), 3);
        assert_eq!(r.plan.accept_start_anchored, vec![false, false, true]);
        assert_eq!(r.plan.accept_end_anchored, vec![false, false, true]);
        assert_eq!(
            r.transition_table.len(),
            r.plan.num_states as usize * 256 * LANES
        );
        assert_eq!(r.epsilon_table.len(), r.plan.num_states as usize * LANES);
    }

    #[test]
    fn regex_compile_uses_checked_abi_and_table_allocation_paths() {
        let production = include_str!("regex_compile.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: regex_compile.rs must contain production section");

        assert!(
            production.contains("u32::try_from(pid)")
                && production.contains("u32::try_from(frag.match_len)")
                && production.contains("u32::try_from(builder.state_count())")
                && production.contains("u32::try_from(self.state_count)")
                && production.contains("checked_add(1)")
                && production.contains("try_reserve_vec_to_capacity")
                && !production.contains("pid as u32")
                && !production.contains("frag.match_len as u32")
                && !production.contains("builder.state_count() as u32")
                && !production.contains("self.state_count as u32")
                && !production.contains("vec![0u32;")
                && !production.contains("Vec::with_capacity(patterns.len())"),
            "Fix: regex compilation must not truncate ids/counts or allocate NFA tables with infallible zero-vector construction."
        );
    }

    #[test]
    fn regex_pipeline_uses_compiled_plan_instead_of_literal_source_plan() {
        let compiled = compile_regex_set(&["a|bc"]).unwrap();
        let pipeline = build_rule_pipeline_from_regex(&["a|bc"], "input", "hits", 64).unwrap();

        assert_eq!(pipeline.plan.num_states, compiled.plan.num_states);
        assert_eq!(
            pipeline.plan.accept_state_ids,
            compiled.plan.accept_state_ids
        );
        assert_eq!(
            pipeline.epsilon_table.iter().any(|word| *word != 0),
            compiled.epsilon_table.iter().any(|word| *word != 0)
        );
        assert_ne!(
            pipeline.plan.num_states,
            crate::scan::nfa::compile(&["a|bc"]).num_states,
            "regex pipeline must not rebuild the scan program from literal regex source bytes"
        );
    }

    #[test]
    fn states_count_grows_with_concat() {
        let one = states_of("a");
        let two = states_of("ab");
        let three = states_of("abc");
        assert!(two > one);
        assert!(three > two);
    }

    #[test]
    fn state_cap_enforced() {
        // Build a regex that would exceed the per-pipeline state cap.
        // A literal of LANES*32+1 = 1025 chars exceeds the cap.
        let huge: String = (0..(STATE_CAP + 4)).map(|_| 'a').collect();
        let err = compile_regex_set(&[&huge]).unwrap_err();
        assert!(matches!(err, RegexCompileError::TooManyStates { .. }));
    }

    #[test]
    fn unsupported_regex_diagnostic_does_not_route_to_cpu_backend() {
        let err = compile_regex_set(&[r"\bsecret\b"]).unwrap_err();
        let message = err.to_string().to_ascii_lowercase();
        assert!(
            !message.contains("cpu"),
            "unsupported GPU-NFA regex diagnostics must not recommend host-side routing: {message}"
        );
        assert!(
            message.contains("gpu"),
            "unsupported GPU-NFA regex diagnostics must name the GPU-compatible rewrite contract: {message}"
        );
    }

    /// Contract: non-ASCII codepoints inside a character class no longer
    /// abort compile. They expand into a UTF-8 byte-sequence alternation
    /// the byte-NFA can represent. Mirrors the homoglyph-expanded
    /// detector patterns consumers feed in (e.g. openai `[hнһｈ]f_...`)
    /// that used to fall on the floor with "unicode character classes
    /// outside ASCII".
    #[test]
    fn unicode_class_outside_ascii_compiles_via_utf8_expansion() {
        // `н` (U+043D) and `һ` (U+04BB) are 2-byte UTF-8; `ｈ` (U+FF48)
        // is 3-byte UTF-8; `h` (U+0068) is 1-byte. All four must be
        // representable.
        let pat = "[hнһｈ]f_[a-zA-Z0-9]{4}";
        let result = compile_regex_set(&[pat]);
        let compiled = match result {
            Ok(c) => c,
            Err(e) => {
                panic!("unicode-extended character class must compile via UTF-8 expansion; got {e}")
            }
        };
        // 4 alternation arms (one per codepoint) × varying byte length
        // + chain states + literal `f_` chain + bounded repetition
        // states - the exact count is implementation-dependent, but
        // every successfully-compiled regex must produce >=2 accept-
        // state-ids worth of state graph.
        assert!(
            compiled.plan.num_states > 4,
            "expanded NFA must have non-trivial state count"
        );
        // accept_state_ids carries one entry per accept (one pattern,
        // so one accept) regardless of arm count; the load-bearing
        // assertion is that compile didn't error.
        assert_eq!(compiled.plan.accept_states.len(), 1);
    }

    /// Contract: classes containing ONLY ASCII still take the fast
    /// single-byte-transition path. Without this guarantee, every AC
    /// detector regex would pay the multi-state expansion cost.
    #[test]
    fn ascii_only_class_keeps_single_byte_transition_path() {
        // Single state for entry + 2 for `[ab]` (start + end) = 3.
        // Anything larger means we accidentally took the expansion arm.
        let r = compile_regex_set(&["[ab]"]).unwrap();
        assert_eq!(
            r.plan.num_states, 3,
            "[ab] must stay on the single-transition fast path (entry + 2 class states); got {} states",
            r.plan.num_states
        );
    }

    /// Contract: massive Unicode ranges that would blow past the
    /// expansion cap return a structured error instead of consuming
    /// unbounded memory.
    #[test]
    fn unicode_class_above_expansion_cap_errors_cleanly() {
        // 257 codepoints - one above MAX_CLASS_EXPANSION_CODEPOINTS = 256.
        let pat = "[\u{0100}-\u{0200}]";
        let err = compile_regex_set(&[pat]).unwrap_err();
        match err {
            RegexCompileError::Unsupported { feature, .. } => {
                assert!(
                    feature.contains("expansion cap"),
                    "over-cap expansion must name the cap in its diagnostic: {feature}"
                );
            }
            other => panic!("expected Unsupported expansion-cap error, got {other:?}"),
        }
    }
}
