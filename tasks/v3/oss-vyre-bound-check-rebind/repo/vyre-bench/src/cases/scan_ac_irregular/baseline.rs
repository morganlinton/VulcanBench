use crate::api::case::BenchError;
use vyre_foundation::match_result::Match;
use vyre_libs::scan::classic_ac::{classic_ac_bounded_ranges_scan, ClassicAcAutomaton};

pub(super) fn cpu_bounded_range_matches(
    ac: &ClassicAcAutomaton,
    pattern_lengths: &[u32],
    haystack: &[u8],
) -> Vec<Match> {
    classic_ac_bounded_ranges_scan(ac, pattern_lengths, haystack)
        .into_iter()
        .map(|(pattern_id, start, end)| Match::new(pattern_id, start, end))
        .collect()
}

pub(super) fn cpu_aho_overlapping_matches(
    patterns: &[&[u8]],
    haystack: &[u8],
) -> Result<Vec<Match>, BenchError> {
    let ac = aho_corasick::AhoCorasick::new(patterns).map_err(|error| {
        BenchError::EnvironmentInvalid(format!(
            "aho-corasick CPU baseline could not compile irregular literal set: {error}. Fix: remove empty or unsupported patterns from the benchmark fixture."
        ))
    })?;
    let mut matches = Vec::new();
    for hit in ac.find_overlapping_iter(haystack) {
        matches.push(Match::new(
            hit.pattern().as_u32(),
            u32::try_from(hit.start()).map_err(|source| {
                BenchError::EnvironmentInvalid(format!(
                    "aho-corasick CPU baseline match start exceeded u32: {source}. Fix: split the scan before benchmarking."
                ))
            })?,
            u32::try_from(hit.end()).map_err(|source| {
                BenchError::EnvironmentInvalid(format!(
                    "aho-corasick CPU baseline match end exceeded u32: {source}. Fix: split the scan before benchmarking."
                ))
            })?,
        ));
    }
    matches.sort_unstable();
    Ok(matches)
}
