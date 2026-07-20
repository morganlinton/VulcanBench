//! Multi-corpus batching for translation-unit frontend work.

use std::collections::BTreeMap;

/// Stable batching key for frontend translation-unit reuse.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TranslationUnitBatchKey {
    /// Include-graph cache key.
    pub include_graph_hash: u64,
    /// Preprocessor environment key.
    pub preprocessor_env_hash: u64,
    /// Semantic graph shape key.
    pub semantic_shape_hash: u64,
}

/// One translation unit candidate for corpus batching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranslationUnitBatchItem {
    /// Stable translation unit id.
    pub translation_unit_id: u32,
    /// Reuse key.
    pub key: TranslationUnitBatchKey,
    /// Estimated source bytes.
    pub source_bytes: u64,
}

/// One batch of translation units sharing frontend residency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationUnitBatch {
    /// Shared key for all translation units in this batch.
    pub key: TranslationUnitBatchKey,
    /// Translation unit ids in stable order.
    pub translation_unit_ids: Vec<u32>,
    /// Total source bytes covered by the batch.
    pub source_bytes: u64,
}

/// Corpus batch plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiCorpusBatchPlan {
    /// Batches sorted by largest source byte weight first.
    pub batches: Vec<TranslationUnitBatch>,
    /// Number of distinct include/preprocessor/semantic uploads required.
    pub resident_upload_groups: usize,
    /// Number of uploads avoided compared with one upload group per TU.
    pub avoided_upload_groups: usize,
}

/// Multi-corpus batching errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiCorpusBatchError {
    /// Duplicate translation unit id.
    DuplicateTranslationUnit { id: u32 },
    /// Source byte accumulation overflowed.
    SourceBytesOverflow,
}

impl std::fmt::Display for MultiCorpusBatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateTranslationUnit { id } => write!(
                f,
                "multi-corpus batch received duplicate translation unit id {id}. Fix: assign stable unique ids before batching."
            ),
            Self::SourceBytesOverflow => f.write_str(
                "multi-corpus batch source byte accumulation overflowed. Fix: shard the corpus before planning batches.",
            ),
        }
    }
}

impl std::error::Error for MultiCorpusBatchError {}

/// Plan frontend batches that share include cache and semantic graph residency.
pub fn plan_multi_corpus_batches(
    items: &[TranslationUnitBatchItem],
) -> Result<MultiCorpusBatchPlan, MultiCorpusBatchError> {
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut groups: BTreeMap<TranslationUnitBatchKey, TranslationUnitBatch> = BTreeMap::new();
    for item in items {
        if !seen_ids.insert(item.translation_unit_id) {
            return Err(MultiCorpusBatchError::DuplicateTranslationUnit {
                id: item.translation_unit_id,
            });
        }
        let batch = groups
            .entry(item.key)
            .or_insert_with(|| TranslationUnitBatch {
                key: item.key,
                translation_unit_ids: Vec::new(),
                source_bytes: 0,
            });
        batch.translation_unit_ids.push(item.translation_unit_id);
        batch.source_bytes = batch
            .source_bytes
            .checked_add(item.source_bytes)
            .ok_or(MultiCorpusBatchError::SourceBytesOverflow)?;
    }

    let mut batches: Vec<TranslationUnitBatch> = groups.into_values().collect();
    for batch in &mut batches {
        batch.translation_unit_ids.sort_unstable();
    }
    batches.sort_by(|left, right| {
        right
            .source_bytes
            .cmp(&left.source_bytes)
            .then_with(|| left.key.cmp(&right.key))
    });
    let resident_upload_groups = batches.len();
    let avoided_upload_groups = items.len().saturating_sub(resident_upload_groups);

    Ok(MultiCorpusBatchPlan {
        batches,
        resident_upload_groups,
        avoided_upload_groups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_corpus_batching_groups_shared_frontend_residency() {
        let shared = key(1, 2, 3);
        let other = key(1, 2, 4);
        let plan = plan_multi_corpus_batches(&[
            item(10, shared, 100),
            item(12, other, 50),
            item(11, shared, 300),
        ])
        .expect("Fix: valid corpus should batch");

        assert_eq!(plan.resident_upload_groups, 2);
        assert_eq!(plan.avoided_upload_groups, 1);
        assert_eq!(plan.batches[0].key, shared);
        assert_eq!(plan.batches[0].translation_unit_ids, vec![10, 11]);
        assert_eq!(plan.batches[0].source_bytes, 400);
    }

    #[test]
    fn multi_corpus_batching_rejects_duplicate_translation_units() {
        let err =
            plan_multi_corpus_batches(&[item(7, key(1, 1, 1), 10), item(7, key(1, 1, 1), 20)])
                .expect_err("duplicate TU ids should fail");

        assert_eq!(
            err,
            MultiCorpusBatchError::DuplicateTranslationUnit { id: 7 }
        );
    }

    #[test]
    fn multi_corpus_batching_rejects_source_byte_overflow() {
        let err =
            plan_multi_corpus_batches(&[item(1, key(1, 1, 1), u64::MAX), item(2, key(1, 1, 1), 1)])
                .expect_err("source byte overflow should fail");

        assert_eq!(err, MultiCorpusBatchError::SourceBytesOverflow);
    }

    fn key(
        include_graph_hash: u64,
        preprocessor_env_hash: u64,
        semantic_shape_hash: u64,
    ) -> TranslationUnitBatchKey {
        TranslationUnitBatchKey {
            include_graph_hash,
            preprocessor_env_hash,
            semantic_shape_hash,
        }
    }

    fn item(
        translation_unit_id: u32,
        key: TranslationUnitBatchKey,
        source_bytes: u64,
    ) -> TranslationUnitBatchItem {
        TranslationUnitBatchItem {
            translation_unit_id,
            key,
            source_bytes,
        }
    }
}
