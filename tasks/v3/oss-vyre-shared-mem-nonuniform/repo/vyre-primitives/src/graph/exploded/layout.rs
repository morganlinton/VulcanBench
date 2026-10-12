/// Checked dispatch layout for an exploded IFDS CSR build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IfdsCsrLayout {
    /// Whether the declared IFDS domain is empty and should not dispatch.
    pub empty: bool,
    /// Number of procedures in the exploded domain.
    pub num_procs: u32,
    /// Number of blocks per procedure.
    pub blocks_per_proc: u32,
    /// Number of facts per procedure.
    pub facts_per_proc: u32,
    /// Number of intra-procedural control-flow edges.
    pub intra_count: u32,
    /// Number of inter-procedural call/return edges.
    pub inter_count: u32,
    /// Number of GEN rules.
    pub gen_count: u32,
    /// Number of KILL rules.
    pub kill_count: u32,
    /// Number of u32 words required by each intra edge field buffer.
    pub intra_storage_words: usize,
    /// Number of u32 words required by each inter edge field buffer.
    pub inter_storage_words: usize,
    /// Number of u32 words required by each GEN rule field buffer.
    pub gen_storage_words: usize,
    /// Number of u32 words required by each KILL rule field buffer.
    pub kill_storage_words: usize,
    /// Dense nodes per procedure.
    pub slots_per_proc: u32,
    /// Total dense node count.
    pub total_nodes: u32,
    /// Number of `u32` words in `row_ptr`.
    pub row_words: usize,
    /// Number of `u32` words in the dispatch row cursor scratch buffer.
    pub row_cursor_words: usize,
    /// Number of `u32` words in the dense kill bitmap scratch buffer.
    pub killed_words: usize,
    /// Maximum emitted column count for the declared edge/rule counts.
    pub max_col_count: u32,
    /// Number of `u32` words allocated for `col_idx`.
    pub col_buffer_words: usize,
}

/// Primitive-owned cache identity for exploded IFDS CSR construction Programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IfdsCsrProgramCacheKey {
    /// Number of procedures in the exploded domain.
    pub num_procs: u32,
    /// Number of blocks per procedure.
    pub blocks_per_proc: u32,
    /// Number of facts per procedure.
    pub facts_per_proc: u32,
    /// Number of intra-procedural control-flow edges.
    pub intra_count: u32,
    /// Number of inter-procedural call/return edges.
    pub inter_count: u32,
    /// Number of GEN rules.
    pub gen_count: u32,
    /// Number of KILL rules.
    pub kill_count: u32,
    /// Maximum emitted column count baked into the generated Program.
    pub max_col_count: u32,
}

/// Stable identity for IFDS rule tuples supplied to the CSR builder.
///
/// This is intentionally distinct from [`IfdsCsrProgramCacheKey`]: the generated
/// program depends on dimensions and rule counts, while staged dispatch input
/// reuse also depends on the actual tuple contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IfdsCsrRuleInputFingerprint {
    /// Fingerprint of `(proc, src_block, dst_block)` intra edges.
    pub intra: u128,
    /// Fingerprint of `(src_proc, src_block, dst_proc, dst_block)` inter edges.
    pub inter: u128,
    /// Fingerprint of `(proc, block, fact)` GEN rules.
    pub gen: u128,
    /// Fingerprint of `(proc, block, fact)` KILL rules.
    pub kill: u128,
}

/// Primitive-owned identity for reusable exploded IFDS static inputs.
///
/// The generated Program depends on [`IfdsCsrProgramCacheKey`]. Staged rule
/// inputs also depend on tuple contents, so dispatch wrappers use this key to
/// refresh uploads without owning IFDS fingerprint composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IfdsCsrStaticInputKey {
    /// Program-shape key selected by the primitive dispatch plan.
    pub program_key: IfdsCsrProgramCacheKey,
    /// Stable content fingerprint of all staged IFDS rule tuples.
    pub rule_fingerprint: IfdsCsrRuleInputFingerprint,
}

impl IfdsCsrRuleInputFingerprint {
    /// Build a stable rule-content fingerprint without allocating columns.
    #[must_use]
    pub fn from_rules(
        intra_edges: &[(u32, u32, u32)],
        inter_edges: &[(u32, u32, u32, u32)],
        flow_gen: &[(u32, u32, u32)],
        flow_kill: &[(u32, u32, u32)],
    ) -> Self {
        Self {
            intra: fingerprint_rule_triples(intra_edges),
            inter: fingerprint_rule_quads(inter_edges),
            gen: fingerprint_rule_triples(flow_gen),
            kill: fingerprint_rule_triples(flow_kill),
        }
    }
}

fn mix_rule_word(hash: &mut u128, value: u32) {
    *hash ^= u128::from(value)
        .wrapping_add(0x9E37_79B9_7F4A_7C15_6A09_E667_F3BC_C909)
        .wrapping_add(*hash << 7)
        .wrapping_add(*hash >> 3);
    *hash = hash
        .rotate_left(31)
        .wrapping_mul(0xD6E8_FD9D_DA37_3C91_BB67_AE85_84CA_A73B);
}

fn fingerprint_rule_triples(rules: &[(u32, u32, u32)]) -> u128 {
    let mut hash = 0x243F_6A88_85A3_08D3_1319_8A2E_0370_7344_u128 ^ rules.len() as u128;
    for &(a, b, c) in rules {
        mix_rule_word(&mut hash, a);
        mix_rule_word(&mut hash, b);
        mix_rule_word(&mut hash, c);
    }
    hash
}

fn fingerprint_rule_quads(rules: &[(u32, u32, u32, u32)]) -> u128 {
    let mut hash = 0xA409_3822_299F_31D0_082E_FA98_EC4E_6C89_u128 ^ rules.len() as u128;
    for &(a, b, c, d) in rules {
        mix_rule_word(&mut hash, a);
        mix_rule_word(&mut hash, b);
        mix_rule_word(&mut hash, c);
        mix_rule_word(&mut hash, d);
    }
    hash
}

impl IfdsCsrProgramCacheKey {
    /// Build a Program cache key from a validated IFDS layout.
    #[must_use]
    pub const fn from_layout(layout: &IfdsCsrLayout) -> Self {
        Self {
            num_procs: layout.num_procs,
            blocks_per_proc: layout.blocks_per_proc,
            facts_per_proc: layout.facts_per_proc,
            intra_count: layout.intra_count,
            inter_count: layout.inter_count,
            gen_count: layout.gen_count,
            kill_count: layout.kill_count,
            max_col_count: layout.max_col_count,
        }
    }
}
