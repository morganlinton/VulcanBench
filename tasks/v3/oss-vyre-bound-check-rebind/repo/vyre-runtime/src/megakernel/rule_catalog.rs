//! DFA rule catalog packing for batched megakernel dispatch.

use super::staging_reserve::{
    reserve_hash_map_capacity as reserve_catalog_map, reserve_vec_capacity as reserve_catalog_vec,
};
use crate::PipelineError;
use rustc_hash::FxHashMap;

/// Dense byte alphabet used by the DFA transition table as the INPUT
/// (`BatchRuleProgram`) representation: every rule still arrives as a dense
/// `state * 256 + byte` table. The on-device packed table is byte-class
/// compressed (see [`pack_rule_catalog_into`]); this constant is the source
/// alphabet width the compressor folds DOWN from.
pub const ALPHABET_SIZE: u32 = 256;
const ALPHABET_SIZE_USIZE: usize = 256;

/// Number of `u32` words per rule metadata entry. The kernel reads these in
/// order: `transition_base`, `accept_base`, `state_count`, `class_map_base`,
/// `num_classes`. Bump in lockstep with [`RuleMeta`] and the dispatcher's
/// `dfa_byte_scanner` if the per-rule metadata grows.
pub const RULE_META_WORDS: usize = 5;

/// One compiled DFA-backed rule program consumed by the batch dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRuleProgram {
    /// Stable rule-table index.
    pub rule_idx: u32,
    /// Dense DFA transition table (`state * 256 + byte -> next_state`).
    pub transitions: Vec<u32>,
    /// Dense DFA accept table (`state -> non-zero match marker`).
    pub accept: Vec<u32>,
    /// DFA state count.
    pub state_count: u32,
}

impl BatchRuleProgram {
    /// Build one DFA-backed rule program.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when the DFA buffers do not match
    /// `state_count`.
    pub fn new(
        rule_idx: u32,
        transitions: Vec<u32>,
        accept: Vec<u32>,
        state_count: u32,
    ) -> Result<Self, PipelineError> {
        validate_rule_shape(rule_idx, &transitions, &accept, state_count)?;
        Ok(Self {
            rule_idx,
            transitions,
            accept,
            state_count,
        })
    }
}

/// Packed metadata for one byte-class-compressed DFA rule entry.
///
/// The on-device transition table is `transitions[transition_base + state *
/// num_classes + class_maps[class_map_base + byte]]`: each rule carries a
/// 256-entry byte→class map (into the shared `class_maps` buffer) and a
/// compressed `state_count * num_classes` transition block, instead of a dense
/// `state_count * 256` block. The compression is LOSSLESS — bytes share a class
/// only when their transition column is identical across every state — so GPU
/// firings are byte-for-byte identical to the dense table.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RuleMeta {
    /// Word offset into the flattened (compressed) transition table.
    pub transition_base: u32,
    /// Word offset into the flattened accept table.
    pub accept_base: u32,
    /// DFA state count for this rule.
    pub state_count: u32,
    /// Word offset into the shared 256-entry-per-rule byte→class map table.
    pub class_map_base: u32,
    /// Number of distinct byte classes for this rule (the compressed row width).
    pub num_classes: u32,
}

/// One rule rejected from a megakernel batch while other rules still ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRuleRejection {
    /// Caller-supplied rule index when present.
    pub rule_idx: Option<u32>,
    /// Human-readable rejection reason.
    pub reason: String,
}

/// Packed rule catalog uploaded to device storage buffers.
pub struct PackedRuleCatalog {
    /// Dense per-rule metadata table.
    pub rule_meta: Vec<RuleMeta>,
    /// Deduplicated flattened byte-class-COMPRESSED DFA transition storage.
    /// Indexed `transitions[rule.transition_base + state * rule.num_classes +
    /// class]` where `class = class_maps[rule.class_map_base + byte]`.
    pub transitions: Vec<u32>,
    /// Deduplicated flattened DFA accept storage.
    pub accept: Vec<u32>,
    /// Deduplicated flattened 256-entry-per-rule byte→class maps. Indexed
    /// `class_maps[rule.class_map_base + byte]`.
    pub class_maps: Vec<u32>,
    /// Rules rejected during validation or dense-slot assignment.
    pub rejected_rules: Vec<BatchRuleRejection>,
}

/// Caller-owned storage for packing rule catalogs without rebuilding host
/// allocations on every refresh.
#[derive(Default)]
pub struct RuleCatalogPackingScratch {
    /// Dense per-rule metadata table.
    pub rule_meta: Vec<RuleMeta>,
    /// Deduplicated flattened byte-class-COMPRESSED DFA transition storage.
    pub transitions: Vec<u32>,
    /// Deduplicated flattened DFA accept storage.
    pub accept: Vec<u32>,
    /// Deduplicated flattened 256-entry-per-rule byte→class maps.
    pub class_maps: Vec<u32>,
    /// Rules rejected during validation or dense-slot assignment.
    pub rejected_rules: Vec<BatchRuleRejection>,
    /// fingerprint -> (transition_base, accept_base, state_count,
    /// class_map_base, num_classes) for storage dedup across identical DFAs.
    unique_storage: FxHashMap<[u8; 32], UniqueStorageLayout>,
    occupied: Vec<bool>,
    addressed: Vec<bool>,
    /// Reusable 256-entry byte→class scratch built per unique DFA.
    class_scratch: Vec<u32>,
}

/// Resident-buffer layout for one deduplicated unique DFA storage block.
#[derive(Clone, Copy)]
struct UniqueStorageLayout {
    transition_base: u32,
    accept_base: u32,
    state_count: u32,
    class_map_base: u32,
    num_classes: u32,
}

/// Fingerprints for the valid dense catalog entries.
#[must_use]
pub fn accepted_rule_fingerprints(
    rules: &[BatchRuleProgram],
) -> (Vec<[u8; 32]>, Vec<BatchRuleRejection>) {
    let mut fingerprints = Vec::new();
    let mut occupied = Vec::new();
    let mut addressed = Vec::new();
    let rejections =
        accepted_rule_fingerprints_into(rules, &mut fingerprints, &mut occupied, &mut addressed);
    (fingerprints, rejections)
}

/// Fill caller-owned storage with fingerprints for valid dense catalog entries.
///
/// The output fingerprint order matches dense rule-table order, not input
/// order. `fingerprints`, `occupied`, and `addressed` are cleared and reused so
/// dispatchers can check resident catalog identity without allocating on every
/// cache-hit dispatch.
pub fn accepted_rule_fingerprints_into(
    rules: &[BatchRuleProgram],
    fingerprints: &mut Vec<[u8; 32]>,
    occupied: &mut Vec<bool>,
    addressed: &mut Vec<bool>,
) -> Vec<BatchRuleRejection> {
    let mut rejections = Vec::new();
    accepted_rule_fingerprints_and_rejections_into(
        rules,
        fingerprints,
        occupied,
        addressed,
        &mut rejections,
    );
    rejections
}

/// Fill caller-owned storage with fingerprints and rejection details for valid
/// dense catalog entries.
///
/// This is the allocation-stable form used by hot dispatchers. All scratch
/// vectors are cleared and reused; valid unchanged catalogs perform no host
/// allocations while checking resident rule-buffer identity.
pub fn accepted_rule_fingerprints_and_rejections_into(
    rules: &[BatchRuleProgram],
    fingerprints: &mut Vec<[u8; 32]>,
    occupied: &mut Vec<bool>,
    addressed: &mut Vec<bool>,
    rejections: &mut Vec<BatchRuleRejection>,
) {
    fingerprints.clear();
    fingerprints.resize(rules.len(), [0; 32]);
    occupied.clear();
    occupied.resize(rules.len(), false);
    addressed.clear();
    addressed.resize(rules.len(), false);
    rejections.clear();

    for rule in rules {
        mark_addressed(addressed, rule.rule_idx);
        match validate_rule_shape(
            rule.rule_idx,
            &rule.transitions,
            &rule.accept,
            rule.state_count,
        ) {
            Ok(()) => match claim_dense_index(occupied, rule.rule_idx, rules.len()) {
                Ok(index) => fingerprints[index] = rule_fingerprint(rule),
                Err(rejection) => rejections.push(rejection),
            },
            Err(error) => rejections.push(BatchRuleRejection {
                rule_idx: Some(rule.rule_idx),
                reason: error.to_string(),
            }),
        }
    }

    extend_missing_rejections(occupied, addressed, rejections);
    let mut write = 0;
    for read in 0..occupied.len() {
        if occupied[read] {
            fingerprints[write] = fingerprints[read];
            write += 1;
        }
    }
    fingerprints.truncate(write);
}

/// Pack valid DFA rules into compact shared device tables.
///
/// Rules with identical `(transitions, accept, state_count)` share backing
/// transition and accept storage while retaining distinct dense metadata slots.
pub fn pack_rule_catalog(rules: &[BatchRuleProgram]) -> Result<PackedRuleCatalog, PipelineError> {
    let mut scratch = RuleCatalogPackingScratch::default();
    pack_rule_catalog_into(rules, &mut scratch)?;
    Ok(PackedRuleCatalog {
        rule_meta: scratch.rule_meta,
        transitions: scratch.transitions,
        accept: scratch.accept,
        class_maps: scratch.class_maps,
        rejected_rules: scratch.rejected_rules,
    })
}

/// Pack valid DFA rules into caller-owned storage.
///
/// Existing vector and hash-map allocations in `scratch` are reused across
/// calls. This is the hot-path form for resident megakernel dispatchers that
/// refresh device rule buffers repeatedly.
pub fn pack_rule_catalog_into(
    rules: &[BatchRuleProgram],
    scratch: &mut RuleCatalogPackingScratch,
) -> Result<(), PipelineError> {
    scratch.unique_storage.clear();
    reserve_catalog_map(
        &mut scratch.unique_storage,
        rules.len(),
        "unique DFA storage",
    )?;
    // Inert rule slot 0: a 1-state DFA that self-loops on every byte and never
    // accepts. Compressed it is num_classes=1, a 1-word transition row `[0]`,
    // and a 256-entry all-zero byte→class map. Rejected / missing rules point
    // their metadata here so the kernel reads a well-formed (no-match) DFA
    // instead of out-of-bounds storage.
    scratch.transitions.clear();
    reserve_catalog_vec(&mut scratch.transitions, 1, "inert transition row")?;
    scratch.transitions.push(0);
    scratch.accept.clear();
    reserve_catalog_vec(&mut scratch.accept, 1, "inert accept row")?;
    scratch.accept.push(0);
    scratch.class_maps.clear();
    reserve_catalog_vec(
        &mut scratch.class_maps,
        ALPHABET_SIZE_USIZE,
        "inert byte-class map",
    )?;
    scratch.class_maps.resize(ALPHABET_SIZE_USIZE, 0);
    scratch.rule_meta.clear();
    reserve_catalog_vec(&mut scratch.rule_meta, rules.len(), "rule metadata")?;
    scratch.rule_meta.resize(
        rules.len(),
        RuleMeta {
            transition_base: 0,
            accept_base: 0,
            state_count: 1,
            class_map_base: 0,
            num_classes: 1,
        },
    );
    scratch.rejected_rules.clear();
    reserve_catalog_vec(
        &mut scratch.rejected_rules,
        rules.len(),
        "rule rejection rows",
    )?;
    scratch.occupied.clear();
    reserve_catalog_vec(&mut scratch.occupied, rules.len(), "dense occupancy bitmap")?;
    scratch.occupied.resize(rules.len(), false);
    scratch.addressed.clear();
    reserve_catalog_vec(
        &mut scratch.addressed,
        rules.len(),
        "dense addressed bitmap",
    )?;
    scratch.addressed.resize(rules.len(), false);

    for rule in rules {
        mark_addressed(&mut scratch.addressed, rule.rule_idx);
        if let Err(error) = validate_rule_shape(
            rule.rule_idx,
            &rule.transitions,
            &rule.accept,
            rule.state_count,
        ) {
            scratch.rejected_rules.push(BatchRuleRejection {
                rule_idx: Some(rule.rule_idx),
                reason: error.to_string(),
            });
            continue;
        }

        let meta_index = match claim_dense_index(
            &mut scratch.occupied,
            rule.rule_idx,
            scratch.rule_meta.len(),
        ) {
            Ok(index) => index,
            Err(rejection) => {
                scratch.rejected_rules.push(rejection);
                continue;
            }
        };

        let storage_fingerprint = dfa_storage_fingerprint(rule);
        let layout = if let Some(layout) = scratch.unique_storage.get(&storage_fingerprint) {
            *layout
        } else {
            // Build the LOSSLESS byte→class map for this DFA into reusable
            // scratch, then emit the compressed `state * num_classes + class`
            // transition block. `num_classes <= 256`, with equality only when
            // every byte transitions differently in some state — the common
            // secret-detector DFAs collapse to a handful of classes.
            let num_classes = build_byte_class_map_for_table(
                &rule.transitions,
                rule.state_count as usize,
                &mut scratch.class_scratch,
            );

            let class_map_base =
                u32::try_from(scratch.class_maps.len()).map_err(|_| PipelineError::QueueFull {
                    queue: "submission",
                    fix: "flattened byte-class map table exceeds u32::MAX words; split the rule catalog into smaller groups",
                })?;
            let class_map_target = scratch
                .class_maps
                .len()
                .checked_add(ALPHABET_SIZE_USIZE)
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "flattened byte-class map length overflows usize; split the rule catalog into smaller groups",
                })?;
            reserve_catalog_vec(
                &mut scratch.class_maps,
                class_map_target,
                "flattened byte-class map storage",
            )?;
            scratch.class_maps.extend_from_slice(&scratch.class_scratch);

            let transition_base =
                u32::try_from(scratch.transitions.len()).map_err(|_| PipelineError::QueueFull {
                    queue: "submission",
                    fix: "flattened transition table exceeds u32::MAX words; split the rule catalog into smaller groups",
                })?;
            let accept_base = u32::try_from(scratch.accept.len()).map_err(|_| PipelineError::QueueFull {
                queue: "submission",
                fix: "flattened accept table exceeds u32::MAX words; split the rule catalog into smaller groups",
            })?;
            // Compressed block size = state_count * num_classes. Both are
            // bounded (state_count is validated, num_classes <= 256), so the
            // product cannot exceed the dense state_count * 256 size that
            // already validated.
            let compressed_words = (rule.state_count as usize)
                .checked_mul(num_classes as usize)
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "compressed transition block size overflows usize; split the rule catalog into smaller groups",
                })?;
            let transition_target = scratch
                .transitions
                .len()
                .checked_add(compressed_words)
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "flattened transition table length overflows usize; split the rule catalog into smaller groups",
                })?;
            reserve_catalog_vec(
                &mut scratch.transitions,
                transition_target,
                "flattened transition storage",
            )?;
            let accept_target = scratch
                .accept
                .len()
                .checked_add(rule.accept.len())
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "flattened accept table length overflows usize; split the rule catalog into smaller groups",
                })?;
            reserve_catalog_vec(
                &mut scratch.accept,
                accept_target,
                "flattened accept storage",
            )?;
            // Emit the compressed `state * num_classes + class` transition block
            // via the shared primitive (lossless: every byte in a class shares
            // its dense column).
            compress_dense_transitions_into(
                &rule.transitions,
                rule.state_count as usize,
                &scratch.class_scratch,
                num_classes,
                &mut scratch.transitions,
            );
            scratch.accept.extend_from_slice(&rule.accept);

            let layout = UniqueStorageLayout {
                transition_base,
                accept_base,
                state_count: rule.state_count,
                class_map_base,
                num_classes,
            };
            scratch
                .unique_storage
                .insert(storage_fingerprint, layout);
            layout
        };
        scratch.rule_meta[meta_index] = RuleMeta {
            transition_base: layout.transition_base,
            accept_base: layout.accept_base,
            state_count: layout.state_count,
            class_map_base: layout.class_map_base,
            num_classes: layout.num_classes,
        };
    }

    extend_missing_rejections(
        &scratch.occupied,
        &scratch.addressed,
        &mut scratch.rejected_rules,
    );
    Ok(())
}

/// Build the LOSSLESS byte→class map for one dense DFA into `out` (resized to
/// 256) and return the class count.
///
/// Two bytes share a class iff their transition COLUMN is identical across every
/// state: `transitions[s*256 + a] == transitions[s*256 + b]` for all `s`. Class
/// ids are assigned in order of first byte appearance, so the map is `0` for
/// byte 0's class and deterministic. The returned width `num_classes` is `<=
/// 256`; for the secret-detector DFAs (long fixed prefixes + a few char
/// classes) it collapses to a handful, shrinking the per-state transition row
/// from 256 words to `num_classes` words — a lossless ~16x reduction on the
/// ~987 MB catalog without changing a single firing.
///
/// `out` is cleared/reused so the hot resident-refresh path allocates nothing.
/// Build the LOSSLESS byte→class map for a dense `state_count * 256` DFA
/// transition table into `out` (resized to 256) and return the class count.
///
/// Two bytes share a class iff their transition COLUMN is identical across every
/// state: `transitions[s*256 + a] == transitions[s*256 + b]` for all `s`. Class
/// ids are assigned in order of first byte appearance, so the map is
/// deterministic. The returned `num_classes` is `<= 256`.
///
/// This is the shared compression primitive: the per-rule catalog packer and
/// the combined-AC megakernel both call it so a single definition owns the
/// "identical column ⇒ same class" contract. `out` is cleared/reused so hot
/// paths allocate nothing.
#[must_use]
pub fn build_byte_class_map_for_table(
    transitions: &[u32],
    state_count: usize,
    out: &mut Vec<u32>,
) -> u32 {
    out.clear();
    out.resize(ALPHABET_SIZE_USIZE, 0);
    // Column signature per byte = its next-state across every state. Group by
    // signature. `FxHashMap` keyed on the column bytes; the first byte to
    // produce a signature owns a fresh class id.
    let mut signature_to_class: FxHashMap<Vec<u32>, u32> = FxHashMap::default();
    let mut next_class: u32 = 0;
    let mut signature = Vec::with_capacity(state_count);
    for byte in 0..ALPHABET_SIZE_USIZE {
        signature.clear();
        for state in 0..state_count {
            signature.push(transitions[state * ALPHABET_SIZE_USIZE + byte]);
        }
        let class = match signature_to_class.get(&signature) {
            Some(&class) => class,
            None => {
                let class = next_class;
                next_class += 1;
                signature_to_class.insert(signature.clone(), class);
                class
            }
        };
        out[byte] = class;
    }
    next_class
}

/// Append the compressed `state_count * num_classes` transition block for a
/// dense `state_count * 256` table to `out`, given the byte→class `class_map`
/// from [`build_byte_class_map_for_table`].
///
/// For class `c` it copies the dense column of ANY byte mapping to `c` (the
/// first; every byte in a class has an identical column by construction, so the
/// value is well-defined and LOSSLESS). Shared by the per-rule packer and the
/// combined-AC megakernel.
pub fn compress_dense_transitions_into(
    dense: &[u32],
    state_count: usize,
    class_map: &[u32],
    num_classes: u32,
    out: &mut Vec<u32>,
) {
    let num_classes = num_classes as usize;
    let mut class_representative = vec![0usize; num_classes];
    let mut seen = vec![false; num_classes];
    for (byte, &class) in class_map.iter().enumerate() {
        let class = class as usize;
        if !seen[class] {
            seen[class] = true;
            class_representative[class] = byte;
        }
    }
    for state in 0..state_count {
        let dense_row = state * ALPHABET_SIZE_USIZE;
        for &rep_byte in &class_representative {
            out.push(dense[dense_row + rep_byte]);
        }
    }
}

/// Pack a byte-class-compressed `state_count * num_classes` transition table
/// (from [`compress_dense_transitions_into`]) into u16 targets stored two per
/// u32 word: the LOW half holds the even flat index, the HIGH half the odd
/// index. Halves the device transition footprint and bytes-per-transaction —
/// the lever the keyhog-scale L1 working-set analysis identified as the one that
/// directly narrows each transition read (`docs/GPU_OOM_SEGMENTATION.md`; row
/// deduplication was measured and refuted there).
///
/// FAIL CLOSED (Law 10): every transition target is a state index, so it must
/// fit `u16`. If ANY target exceeds `u16::MAX` this REFUSES the pack — a silent
/// `& 0xFFFF` truncation would repoint a next-state at the wrong state, an
/// invisible recall loss. Callers gate on `state_count <= 65536`; this is the
/// enforcing check, not an assumption. `out` is cleared/reused so the hot
/// resident-refresh path allocates nothing.
///
/// # Errors
///
/// Returns [`PipelineError::Backend`] naming the offending index/target when a
/// transition target does not fit `u16`.
pub fn try_pack_u16_transitions_into(
    compressed: &[u32],
    out: &mut Vec<u32>,
) -> Result<(), PipelineError> {
    for (idx, &target) in compressed.iter().enumerate() {
        if target > u32::from(u16::MAX) {
            return Err(PipelineError::Backend(format!(
                "transition target {target} at index {idx} exceeds u16::MAX ({}); u16 packing \
                 would silently truncate it and corrupt the automaton. Fix: keep this catalog on \
                 the u32 transition path (state_count must be <= 65536 for u16 packing).",
                u16::MAX
            )));
        }
    }
    out.clear();
    out.reserve(compressed.len().div_ceil(2));
    let mut chunks = compressed.chunks_exact(2);
    for pair in chunks.by_ref() {
        // Both halves validated <= 0xFFFF above, so no masking is needed.
        out.push(pair[0] | (pair[1] << 16));
    }
    if let [last] = chunks.remainder() {
        out.push(*last);
    }
    Ok(())
}

/// Unpack the `flat_index`-th u16 transition target from a table packed by
/// [`try_pack_u16_transitions_into`]. The exact CPU mirror of the kernel's
/// unpack (`word = packed[idx/2]; (word >> ((idx & 1) * 16)) & 0xFFFF`), so the
/// round-trip can be proven lossless without a GPU.
#[must_use]
pub fn unpack_u16_transition(packed: &[u32], flat_index: usize) -> u32 {
    let word = packed[flat_index / 2];
    (word >> ((flat_index & 1) * 16)) & 0xFFFF
}

fn validate_rule_shape(
    rule_idx: u32,
    transitions: &[u32],
    accept: &[u32],
    state_count: u32,
) -> Result<(), PipelineError> {
    let expected_transitions = usize::try_from(state_count)
        .ok()
        .and_then(|count| count.checked_mul(ALPHABET_SIZE_USIZE))
        .ok_or_else(|| {
            PipelineError::Backend("rule transition table size overflowed usize".to_string())
        })?;
    if transitions.len() != expected_transitions {
        return Err(PipelineError::Backend(format!(
            "rule {rule_idx} transition table has {} words, expected {expected_transitions}. Fix: compile a dense state_count * 256 DFA table before batch dispatch.",
            transitions.len()
        )));
    }
    let state_count_usize = usize::try_from(state_count).map_err(|source| {
        PipelineError::Backend(format!(
            "rule {rule_idx} state_count {state_count} cannot fit usize: {source}. Fix: shard the DFA state space before batch dispatch."
        ))
    })?;
    if accept.len() != state_count_usize {
        return Err(PipelineError::Backend(format!(
            "rule {rule_idx} accept table has {} words, expected {state_count}. Fix: emit one accept entry per DFA state before batch dispatch.",
            accept.len()
        )));
    }
    Ok(())
}

fn rule_fingerprint(rule: &BatchRuleProgram) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&rule.rule_idx.to_le_bytes());
    hasher.update(bytemuck::cast_slice(&rule.transitions));
    hasher.update(bytemuck::cast_slice(&rule.accept));
    hasher.update(&rule.state_count.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn dfa_storage_fingerprint(rule: &BatchRuleProgram) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytemuck::cast_slice(&rule.transitions));
    hasher.update(bytemuck::cast_slice(&rule.accept));
    hasher.update(&rule.state_count.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn mark_addressed(addressed: &mut [bool], rule_idx: u32) {
    if let Some(index) = usize::try_from(rule_idx)
        .ok()
        .filter(|index| *index < addressed.len())
    {
        addressed[index] = true;
    }
}

fn claim_dense_index(
    occupied: &mut [bool],
    rule_idx: u32,
    slot_count: usize,
) -> Result<usize, BatchRuleRejection> {
    let Some(meta_index) = usize::try_from(rule_idx).ok() else {
        return Err(BatchRuleRejection {
            rule_idx: Some(rule_idx),
            reason: "rule_idx exceeds usize. Fix: rebuild the batch with a smaller rule catalog"
                .to_string(),
        });
    };
    if meta_index >= slot_count {
        return Err(BatchRuleRejection {
            rule_idx: Some(rule_idx),
            reason: format!(
                "rule_idx {rule_idx} falls outside 0..{slot_count}. Fix: keep the rule catalog dense so the batch work queue can address every rule"
            ),
        });
    }
    if occupied[meta_index] {
        return Err(BatchRuleRejection {
            rule_idx: Some(rule_idx),
            reason: format!(
                "duplicate rule_idx {rule_idx}. Fix: keep exactly one rule per dense rule-table slot"
            ),
        });
    }
    occupied[meta_index] = true;
    Ok(meta_index)
}

fn extend_missing_rejections(
    occupied: &[bool],
    addressed: &[bool],
    out: &mut Vec<BatchRuleRejection>,
) {
    for (rule_idx, (occupied, addressed)) in occupied
        .iter()
        .copied()
        .zip(addressed.iter().copied())
        .enumerate()
    {
        if !occupied && !addressed {
            let Ok(rule_idx_u32) = u32::try_from(rule_idx) else {
                continue;
            };
            out.push(BatchRuleRejection {
                rule_idx: Some(rule_idx_u32),
                reason: format!(
                    "rule_idx {rule_idx} has no valid catalog entry. Fix: provide a well-formed DFA for every dense rule slot before batch dispatch"
                ),
            });
        }
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    /// Resolve the next state the COMPRESSED packed catalog yields for
    /// `(rule, state, byte)` — mirrors the GPU kernel's index math exactly so
    /// the parity tests can prove byte-for-byte equivalence to the dense table.
    fn packed_next_state(packed: &PackedRuleCatalog, meta_index: usize, state: u32, byte: u8) -> u32 {
        let meta = packed.rule_meta[meta_index];
        let class = packed.class_maps[meta.class_map_base as usize + byte as usize];
        let idx = meta.transition_base as usize
            + state as usize * meta.num_classes as usize
            + class as usize;
        packed.transitions[idx]
    }

    #[test]
    fn duplicate_dfas_share_catalog_storage() {
        let first = BatchRuleProgram::new(0, vec![0; 256], vec![0], 1).unwrap();
        let second = BatchRuleProgram::new(1, vec![0; 256], vec![0], 1).unwrap();
        let packed = pack_rule_catalog(&[first, second]).unwrap();
        // Identical DFAs share compressed transition, accept AND class-map storage.
        assert_eq!(
            packed.rule_meta[0].transition_base,
            packed.rule_meta[1].transition_base
        );
        assert_eq!(
            packed.rule_meta[0].accept_base,
            packed.rule_meta[1].accept_base
        );
        assert_eq!(
            packed.rule_meta[0].class_map_base,
            packed.rule_meta[1].class_map_base
        );
        assert_eq!(packed.rule_meta[0].num_classes, packed.rule_meta[1].num_classes);
        // An all-zero 1-state DFA collapses to a SINGLE byte class (every byte
        // self-loops to state 0), so its compressed row is exactly one word, not
        // 256. transition_base points just past the 1-word inert row.
        assert_eq!(packed.rule_meta[0].num_classes, 1);
        assert_eq!(packed.rule_meta[0].transition_base, 1);
        assert_eq!(
            packed.transitions.len(),
            packed.rule_meta[0].transition_base as usize + 1
        );
        assert_eq!(
            packed.accept.len(),
            packed.rule_meta[0].accept_base as usize + 1
        );
        assert!(packed.rejected_rules.is_empty());
    }

    #[test]
    fn u16_pack_round_trips_losslessly_including_odd_tail() {
        // Odd element count exercises the lone-remainder path; 65535 is the max
        // legal u16 target.
        let compressed: Vec<u32> = vec![0, 1, 2, 65_535, 100, 0, 42];
        let mut packed = Vec::new();
        try_pack_u16_transitions_into(&compressed, &mut packed).expect("all targets fit u16");
        // Two u16 per u32 word, rounding up for the odd tail.
        assert_eq!(packed.len(), compressed.len().div_ceil(2));
        // Every flat index unpacks to EXACTLY the original target — proving the
        // pack→kernel-unpack round-trip changes no transition (Law 6).
        for (idx, &original) in compressed.iter().enumerate() {
            assert_eq!(
                unpack_u16_transition(&packed, idx),
                original,
                "u16 round-trip diverged at flat index {idx}",
            );
        }
    }

    #[test]
    fn u16_pack_fails_closed_on_target_exceeding_u16() {
        // 70000 > u16::MAX: packing MUST refuse, never silently `& 0xFFFF` it to a
        // wrong next-state (Law 10 — that truncation is an invisible recall loss).
        let compressed: Vec<u32> = vec![0, 1, 70_000, 3];
        let mut out = Vec::new();
        let err = try_pack_u16_transitions_into(&compressed, &mut out)
            .expect_err("a target above u16::MAX must be refused");
        let msg = err.to_string();
        assert!(
            msg.contains("70000") && msg.contains("index 2") && msg.contains("u16"),
            "error must name the offending target/index and the u16 cause: {msg}",
        );
    }

    /// Regression for P2 decoration test: the structural shared-storage checks
    /// above were not sufficient — a refactor could share the WRONG compressed
    /// block and still pass the field-equality assertion. This test packs TWO
    /// identical copies of the non-trivial 3-class DFA from
    /// `byte_class_compression_is_lossless` and then calls `packed_next_state`
    /// on BOTH meta indices for every (state, byte) pair, asserting both return
    /// the same value AND that value matches the dense source table.
    #[test]
    fn duplicate_dfas_shared_storage_both_rules_fire_correctly() {
        // 3-state, 3-class DFA — same fixture as byte_class_compression_is_lossless.
        let states = 3usize;
        let mut dense = vec![0u32; states * 256];
        dense[0 * 256 + 0x41] = 1; // state 0: 'A' -> 1
        dense[1 * 256 + 0x41] = 2; // state 1: 'A' -> 2
        dense[1 * 256 + 0x42] = 2; // state 1: 'B' -> 2
        dense[2 * 256 + 0x41] = 2; // state 2: 'A' -> 2
        let accept = vec![0u32, 0, 1];

        let rule0 = BatchRuleProgram::new(0, dense.clone(), accept.clone(), states as u32).unwrap();
        let rule1 = BatchRuleProgram::new(1, dense.clone(), accept.clone(), states as u32).unwrap();
        let packed = pack_rule_catalog(&[rule0, rule1]).unwrap();

        assert!(packed.rejected_rules.is_empty());
        // Both rules must share storage.
        assert_eq!(packed.rule_meta[0].transition_base, packed.rule_meta[1].transition_base,
            "Fix: duplicate DFAs must share transition storage");
        assert_eq!(packed.rule_meta[0].accept_base, packed.rule_meta[1].accept_base,
            "Fix: duplicate DFAs must share accept storage");

        // Critical: verify BOTH meta indices yield the correct DFA output for
        // every (state, byte) — structural field sharing is necessary but not
        // sufficient; the shared block must actually encode the right DFA.
        for state in 0..states as u32 {
            for byte in 0u16..256 {
                let byte = byte as u8;
                let expected = dense[state as usize * 256 + byte as usize];
                let got0 = packed_next_state(&packed, 0, state, byte);
                let got1 = packed_next_state(&packed, 1, state, byte);
                assert_eq!(
                    got0, expected,
                    "Fix: rule0 compressed transition mismatch at state {state} byte {byte:#x}: expected {expected} got {got0}"
                );
                assert_eq!(
                    got1, expected,
                    "Fix: rule1 compressed transition mismatch at state {state} byte {byte:#x}: expected {expected} got {got1}"
                );
            }
        }
    }

    #[test]
    fn duplicate_dfas_do_not_reserve_raw_duplicate_storage() {
        let rules = (0..32)
            .map(|rule_idx| BatchRuleProgram::new(rule_idx, vec![0; 256], vec![0], 1).unwrap())
            .collect::<Vec<_>>();

        let packed = pack_rule_catalog(&rules).unwrap();

        // 1-word inert row + 1-word shared compressed row for all 32 duplicates.
        assert_eq!(packed.transitions.len(), 2);
        assert!(
            packed.transitions.capacity() < ALPHABET_SIZE as usize * rules.len(),
            "Fix: duplicate DFA catalogs must not reserve memory as if every rule had unique transition storage."
        );
        assert_eq!(packed.accept.len(), 2);
        assert!(
            packed.accept.capacity() < rules.len(),
            "Fix: duplicate DFA catalogs must not reserve accept storage for every duplicate rule."
        );
        // One inert + one shared class map, not 32.
        assert_eq!(packed.class_maps.len(), ALPHABET_SIZE as usize * 2);
    }

    /// The compressed catalog yields byte-for-byte identical next-states to the
    /// dense `state * 256 + byte` table for EVERY (state, byte) of a non-trivial
    /// multi-class DFA — the lossless parity contract the GPU kernel depends on.
    #[test]
    fn byte_class_compression_is_lossless() {
        // 3-state DFA. byte 0x41 ('A') advances 0->1->2->2; byte 0x42 ('B')
        // advances 1->2 only; all other bytes reset to 0. This forces THREE
        // distinct byte classes (A, B, everything-else) so num_classes < 256
        // and the compression is exercised, not a degenerate single class.
        let states = 3usize;
        let mut dense = vec![0u32; states * 256];
        // state 0: 'A' -> 1, else -> 0
        dense[0 * 256 + 0x41] = 1;
        // state 1: 'A' -> 2, 'B' -> 2, else -> 0
        dense[1 * 256 + 0x41] = 2;
        dense[1 * 256 + 0x42] = 2;
        // state 2: 'A' -> 2, else -> 0
        dense[2 * 256 + 0x41] = 2;
        let accept = vec![0u32, 0, 1];
        let rule = BatchRuleProgram::new(0, dense.clone(), accept, states as u32).unwrap();
        let packed = pack_rule_catalog(&[rule]).unwrap();

        assert_eq!(packed.rejected_rules.len(), 0);
        // 'A', 'B', and the rest are three behaviourally-distinct columns.
        assert_eq!(packed.rule_meta[0].num_classes, 3);
        assert!(
            packed.transitions.len() < 1 + states * 256,
            "compressed transitions must be smaller than the dense table"
        );

        for state in 0..states as u32 {
            for byte in 0u16..256 {
                let byte = byte as u8;
                let expected = dense[state as usize * 256 + byte as usize];
                let got = packed_next_state(&packed, 0, state, byte);
                assert_eq!(
                    got, expected,
                    "compressed transition mismatch at state {state} byte {byte:#x}: dense={expected} packed={got}"
                );
            }
        }
    }

    /// A DFA whose every byte transitions differently in some state must NOT be
    /// over-compressed: it keeps all 256 classes and still round-trips losslessly.
    #[test]
    fn full_alphabet_dfa_keeps_all_classes_and_is_lossless() {
        // 2-state DFA where state 0 sends byte b -> (b as state is impossible
        // with 2 states), so instead: state 0 sends EVERY byte to a distinct
        // value by using state 1 vs 0 based on parity — that only yields 2
        // classes. To force 256 classes we need 256 distinct columns, which
        // needs >=256 states. Use a 256-state identity: state s, byte b -> b.
        let states = 256usize;
        let mut dense = vec![0u32; states * 256];
        for s in 0..states {
            for b in 0..256 {
                dense[s * 256 + b] = b as u32; // column for byte b is constant = b across all states
            }
        }
        // Every byte's column is the constant vector [b; 256], all distinct, so
        // 256 classes.
        let accept = vec![0u32; states];
        let rule = BatchRuleProgram::new(0, dense.clone(), accept, states as u32).unwrap();
        let packed = pack_rule_catalog(&[rule]).unwrap();
        assert_eq!(packed.rule_meta[0].num_classes, 256);
        for state in 0..states as u32 {
            for byte in 0u16..256 {
                let byte = byte as u8;
                let expected = dense[state as usize * 256 + byte as usize];
                assert_eq!(packed_next_state(&packed, 0, state, byte), expected);
            }
        }
    }

    #[test]
    fn accepted_rule_fingerprints_into_reuses_caller_storage() {
        let rules = (0..8)
            .map(|rule_idx| BatchRuleProgram::new(rule_idx, vec![0; 256], vec![0], 1).unwrap())
            .collect::<Vec<_>>();
        let mut fingerprints = Vec::with_capacity(16);
        let mut occupied = Vec::with_capacity(16);
        let mut addressed = Vec::with_capacity(16);
        let fingerprint_ptr = fingerprints.as_ptr();
        let occupied_ptr = occupied.as_ptr();
        let addressed_ptr = addressed.as_ptr();

        let rejections = accepted_rule_fingerprints_into(
            &rules,
            &mut fingerprints,
            &mut occupied,
            &mut addressed,
        );

        assert!(rejections.is_empty());
        assert_eq!(fingerprints.len(), rules.len());
        assert_eq!(fingerprints.as_ptr(), fingerprint_ptr);
        assert_eq!(occupied.as_ptr(), occupied_ptr);
        assert_eq!(addressed.as_ptr(), addressed_ptr);
    }

    #[test]
    fn invalid_rules_are_isolated_to_inert_catalog_entries() {
        let valid = BatchRuleProgram::new(0, vec![0; 256], vec![1], 1).unwrap();
        let invalid = BatchRuleProgram {
            rule_idx: 1,
            transitions: vec![0; 8],
            accept: vec![0],
            state_count: 1,
        };

        let packed = pack_rule_catalog(&[valid, invalid]).unwrap();
        assert_eq!(packed.rejected_rules.len(), 1);
        assert_eq!(packed.rejected_rules[0].rule_idx, Some(1));
        // Valid rule (slot 0) points at a REAL compressed block past the inert
        // row; the inert/rejected slot 1 points back at the inert row 0.
        assert_eq!(packed.rule_meta[0].state_count, 1);
        assert!(packed.rule_meta[0].transition_base >= 1);
        assert_eq!(packed.rule_meta[1].transition_base, 0);
        assert_eq!(packed.rule_meta[1].accept_base, 0);
        assert_eq!(packed.rule_meta[1].state_count, 1);
        assert_eq!(packed.rule_meta[1].class_map_base, 0);
        assert_eq!(packed.rule_meta[1].num_classes, 1);
        // Inert row 0: a single self-loop word and an all-zero 256-entry class
        // map — the rejected slot reads a well-formed no-match DFA.
        assert_eq!(packed.transitions[0], 0);
        assert_eq!(packed.accept[0], 0);
        assert_eq!(
            &packed.class_maps[..ALPHABET_SIZE as usize],
            &vec![0; ALPHABET_SIZE as usize]
        );
        // Regression for P2 decoration test: a single-byte spot check is not
        // sufficient — a corrupt inert row could have non-zero entries at other
        // bytes or at the accept table while still passing b'X'. This loop
        // proves the inert slot self-loops to state 0 on EVERY byte value and
        // that the accept entry for the inert slot is zero (can never match).
        for byte in 0u16..256 {
            let byte = byte as u8;
            assert_eq!(
                packed_next_state(&packed, 1, 0, byte),
                0,
                "Fix: inert slot must self-loop to state 0 for every byte, failed at byte {byte:#x}"
            );
        }
        // Accept entry for the inert slot at state 0 must be zero (no match).
        assert_eq!(
            packed.accept[packed.rule_meta[1].accept_base as usize],
            0,
            "Fix: inert slot accept entry at state 0 must be 0 — the inert DFA must never produce a match"
        );
    }
}
