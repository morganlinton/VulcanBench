//! Incremental invalidation planning for source, macro, semantic, and fact work.

/// Half-open source byte span.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceSpan {
    /// Start byte offset.
    pub start: u32,
    /// End byte offset, exclusive.
    pub end: u32,
}

/// Dependency region affected by source edits.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvalidationRegionKind {
    /// Token classification or lexer output.
    Token,
    /// Macro expansion or include-driven preprocessor state.
    Macro,
    /// Semantic scope, declaration, or type-dependent state.
    SemanticScope,
    /// Dataflow fact derived from parser or semantic graph nodes.
    Fact,
}

/// One dependency region in the invalidation graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidationRegion {
    /// Region kind.
    pub kind: InvalidationRegionKind,
    /// Source span covered by the region.
    pub span: SourceSpan,
    /// Stable region identifier.
    pub id: u32,
}

/// One recomputation wave.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidationWave {
    /// Region kind recomputed by this wave.
    pub kind: InvalidationRegionKind,
    /// Sorted affected region ids.
    pub region_ids: Vec<u32>,
}

/// Incremental recomputation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncrementalInvalidationPlan {
    /// Merged changed source spans.
    pub changed_spans: Vec<SourceSpan>,
    /// Recompute waves ordered by compiler/dataflow dependency.
    pub waves: Vec<InvalidationWave>,
}

/// Invalidation planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncrementalInvalidationError {
    /// A source span had end before start or zero length.
    InvalidSpan { span: SourceSpan },
}

impl std::fmt::Display for IncrementalInvalidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSpan { span } => write!(
                f,
                "invalid source span {}..{}. Fix: incremental invalidation requires non-empty half-open spans.",
                span.start, span.end
            ),
        }
    }
}

impl std::error::Error for IncrementalInvalidationError {}

/// Plan incremental recomputation from edited spans and dependency regions.
pub fn plan_incremental_invalidation(
    changed_spans: &[SourceSpan],
    dependency_regions: &[InvalidationRegion],
) -> Result<IncrementalInvalidationPlan, IncrementalInvalidationError> {
    let changed_spans = normalize_spans(changed_spans)?;
    let mut token = Vec::new();
    let mut macros = Vec::new();
    let mut semantic = Vec::new();
    let mut facts = Vec::new();

    for region in dependency_regions {
        if changed_spans
            .iter()
            .any(|changed| spans_overlap(*changed, region.span))
        {
            match region.kind {
                InvalidationRegionKind::Token => token.push(region.id),
                InvalidationRegionKind::Macro => macros.push(region.id),
                InvalidationRegionKind::SemanticScope => semantic.push(region.id),
                InvalidationRegionKind::Fact => facts.push(region.id),
            }
        }
    }

    let mut waves = Vec::new();
    push_wave(&mut waves, InvalidationRegionKind::Token, token);
    push_wave(&mut waves, InvalidationRegionKind::Macro, macros);
    push_wave(&mut waves, InvalidationRegionKind::SemanticScope, semantic);
    push_wave(&mut waves, InvalidationRegionKind::Fact, facts);

    Ok(IncrementalInvalidationPlan {
        changed_spans,
        waves,
    })
}

fn normalize_spans(spans: &[SourceSpan]) -> Result<Vec<SourceSpan>, IncrementalInvalidationError> {
    let mut spans = spans.to_vec();
    for span in &spans {
        if span.start >= span.end {
            return Err(IncrementalInvalidationError::InvalidSpan { span: *span });
        }
    }
    spans.sort_unstable();
    let mut merged: Vec<SourceSpan> = Vec::with_capacity(spans.len());
    for span in spans {
        if let Some(last) = merged.last_mut() {
            if span.start <= last.end {
                last.end = last.end.max(span.end);
                continue;
            }
        }
        merged.push(span);
    }
    Ok(merged)
}

fn spans_overlap(left: SourceSpan, right: SourceSpan) -> bool {
    left.start < right.end && right.start < left.end
}

fn push_wave(
    waves: &mut Vec<InvalidationWave>,
    kind: InvalidationRegionKind,
    mut region_ids: Vec<u32>,
) {
    if region_ids.is_empty() {
        return;
    }
    region_ids.sort_unstable();
    region_ids.dedup();
    waves.push(InvalidationWave { kind, region_ids });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidation_plans_only_overlapping_dependency_waves() {
        let plan = plan_incremental_invalidation(
            &[SourceSpan { start: 10, end: 20 }],
            &[
                region(InvalidationRegionKind::Token, 0, 0, 9),
                region(InvalidationRegionKind::Token, 1, 10, 12),
                region(InvalidationRegionKind::Macro, 2, 11, 30),
                region(InvalidationRegionKind::SemanticScope, 3, 18, 40),
                region(InvalidationRegionKind::Fact, 4, 40, 50),
            ],
        )
        .expect("Fix: valid invalidation plan should build");

        assert_eq!(
            plan.waves,
            vec![
                wave(InvalidationRegionKind::Token, &[1]),
                wave(InvalidationRegionKind::Macro, &[2]),
                wave(InvalidationRegionKind::SemanticScope, &[3]),
            ]
        );
    }

    #[test]
    fn invalidation_merges_changed_spans_and_deduplicates_regions() {
        let plan = plan_incremental_invalidation(
            &[
                SourceSpan { start: 20, end: 30 },
                SourceSpan { start: 10, end: 20 },
                SourceSpan { start: 12, end: 14 },
            ],
            &[
                region(InvalidationRegionKind::Fact, 7, 11, 13),
                region(InvalidationRegionKind::Fact, 7, 25, 27),
            ],
        )
        .expect("Fix: merged spans should plan");

        assert_eq!(plan.changed_spans, vec![SourceSpan { start: 10, end: 30 }]);
        assert_eq!(plan.waves, vec![wave(InvalidationRegionKind::Fact, &[7])]);
    }

    #[test]
    fn invalidation_rejects_empty_or_reversed_spans() {
        assert_eq!(
            plan_incremental_invalidation(&[SourceSpan { start: 3, end: 3 }], &[])
                .expect_err("empty span must fail"),
            IncrementalInvalidationError::InvalidSpan {
                span: SourceSpan { start: 3, end: 3 },
            }
        );
        assert_eq!(
            plan_incremental_invalidation(&[SourceSpan { start: 4, end: 3 }], &[])
                .expect_err("reversed span must fail"),
            IncrementalInvalidationError::InvalidSpan {
                span: SourceSpan { start: 4, end: 3 },
            }
        );
    }

    fn region(kind: InvalidationRegionKind, id: u32, start: u32, end: u32) -> InvalidationRegion {
        InvalidationRegion {
            kind,
            span: SourceSpan { start, end },
            id,
        }
    }

    fn wave(kind: InvalidationRegionKind, ids: &[u32]) -> InvalidationWave {
        InvalidationWave {
            kind,
            region_ids: ids.to_vec(),
        }
    }
}
