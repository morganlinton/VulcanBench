use crate::api::case::BenchError;
use vyre_foundation::ir::Program;
use vyre_foundation::match_result::Match;
use vyre_libs::scan::dispatch_io::try_unpack_match_triples;

use super::metrics::ScanAcStats;
use super::{MATCH_TRIPLE_WORDS, PATTERNS};

pub(super) fn pattern_lengths() -> Result<Vec<u32>, BenchError> {
    PATTERNS
        .iter()
        .map(|pattern| {
            u32::try_from(pattern.len()).map_err(|_| {
                BenchError::EnvironmentInvalid(
                    "irregular AC pattern length exceeded u32. Fix: split oversized literals."
                        .to_string(),
                )
            })
        })
        .collect()
}

pub(crate) fn build_irregular_haystack(len: usize) -> (Vec<u8>, u32) {
    let mut haystack = vec![0_u8; len];
    for (index, byte) in haystack.iter_mut().enumerate() {
        let mixed = mix32(index as u32);
        *byte = 33 + (mixed % 90) as u8;
    }

    let mut planted = 0_u32;
    for (pattern_index, pattern) in PATTERNS.iter().enumerate() {
        let stride = 8_191 + pattern_index * 271;
        let phase = 17 + pattern_index * 113;
        let mut offset = phase;
        while offset + pattern.len() <= haystack.len() {
            if (offset & 31) != 0 {
                haystack[offset..offset + pattern.len()].copy_from_slice(pattern);
                planted += 1;
            }
            offset += stride;
        }
    }
    (haystack, planted)
}

pub(super) fn decode_scan_outputs(
    outputs: &[Vec<u8>],
    context: &str,
) -> Result<Vec<Match>, BenchError> {
    let count_bytes = outputs.first().ok_or_else(|| {
        BenchError::CorrectnessViolation(format!("{context} did not produce match_count"))
    })?;
    if count_bytes.len() < 4 {
        return Err(BenchError::CorrectnessViolation(format!(
            "{context} match_count buffer was {} bytes, expected at least 4",
            count_bytes.len()
        )));
    }
    let count = u32::from_le_bytes([
        count_bytes[0],
        count_bytes[1],
        count_bytes[2],
        count_bytes[3],
    ]);
    let triples = outputs.get(1).ok_or_else(|| {
        BenchError::CorrectnessViolation(format!("{context} did not produce match triples"))
    })?;
    let required_triple_bytes = match_triples_readback_bytes(count)?;
    if triples.len() < required_triple_bytes {
        return Err(BenchError::CorrectnessViolation(format!(
            "{context} match triples failed to decode: count={count} requires {required_triple_bytes} bytes but compact readback returned {}",
            triples.len()
        )));
    }
    try_unpack_match_triples(triples, count).map_err(|error| {
        BenchError::CorrectnessViolation(format!(
            "{context} match triples failed to decode: {error}"
        ))
    })
}

pub(crate) fn encode_match_triples(matches: &[Match]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(matches.len() * 12);
    for hit in matches {
        encoded.extend_from_slice(&hit.pattern_id.to_le_bytes());
        encoded.extend_from_slice(&hit.start.to_le_bytes());
        encoded.extend_from_slice(&hit.end.to_le_bytes());
    }
    encoded
}

pub(super) fn match_triples_output_bytes(max_matches: u32) -> Result<usize, BenchError> {
    usize::try_from(max_matches)
        .ok()
        .and_then(|matches| matches.checked_mul(MATCH_TRIPLE_WORDS))
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "irregular AC scan max_matches={max_matches} overflows resident output byte sizing. Fix: split the scan output into smaller shards."
            ))
        })
}

pub(super) fn match_triples_readback_bytes(match_count: u32) -> Result<usize, BenchError> {
    usize::try_from(match_count)
        .ok()
        .and_then(|matches| matches.checked_mul(MATCH_TRIPLE_WORDS))
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "irregular AC scan match_count={match_count} overflows compact match readback byte sizing. Fix: split the scan output into smaller shards."
            ))
        })
}

pub(super) fn selected_scan_output_bytes(stats: ScanAcStats) -> u64 {
    4 + u64::from(stats.expected_matches) * MATCH_TRIPLE_WORDS as u64 * 4
}

pub(super) fn with_matches_readback_range(
    program: Program,
    match_count: u32,
) -> Result<Program, BenchError> {
    let byte_len = match_triples_readback_bytes(match_count)?;
    let mut found_matches_output = false;
    let buffers = program
        .buffers()
        .iter()
        .cloned()
        .map(|buffer| {
            if buffer.name() == "matches" && buffer.is_output() {
                found_matches_output = true;
                buffer.with_output_byte_range(0..byte_len)
            } else {
                buffer
            }
        })
        .collect::<Vec<_>>();
    if !found_matches_output {
        return Err(BenchError::ExecutionFailed(
            "irregular AC scan program did not expose the matches output buffer. Fix: preserve the bounded-ranges scan buffer layout before compact readback planning."
                .to_string(),
        ));
    }
    Ok(program.with_rewritten_buffers(buffers))
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
