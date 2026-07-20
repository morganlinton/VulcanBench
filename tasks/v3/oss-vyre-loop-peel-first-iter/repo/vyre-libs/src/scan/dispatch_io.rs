//! Shared GPU dispatch primitives for matching engines.
//!
//! Every high-level matcher in `vyre-libs::matching` (`GpuLiteralSet`,
//! `RulePipeline`, future ones) needs the same four operations to talk
//! to a `VyreBackend`:
//!
//!   1. Pack a haystack `&[u8]` into `u32` words for the read-only
//!      input storage buffer.
//!   2. Encode an arbitrary `&[u32]` slice as little-endian bytes for
//!      a storage buffer.
//!   3. Validate the haystack's length fits in `u32` (the wire-format
//!      bound that vyre's IR enforces) and return a typed
//!      `BackendError` with an actionable `Fix:` message otherwise.
//!   4. Compute the per-axis grid geometry that maps haystack bytes
//!      onto the program's `workgroup_size[0]` lane fan-out.
//!
//! Each of those was duplicated 2x as I added the second matcher
//! (`RulePipeline::scan`). Centralising them here makes the *next*
//! matcher (parser combinators, taint-flow scan, custom regex
//! compositions in downstream crates) free to compose  -  write the unique
//! plumbing, reuse the shared four.
//!
//! The output-layout step is intentionally **not** centralised:
//! `GpuLiteralSet` uses a two-buffer layout (`match_count` + `matches`),
//! while `RulePipeline` uses a single hit buffer with embedded counter.
//! Once the caller has isolated the counter and match-triple byte range,
//! decoding is shared so every engine rejects malformed readbacks the same
//! way.

use std::borrow::Cow;

use vyre::{BackendError, DispatchConfig};

const U32_COUNTER_BYTES: usize = 4;
const MATCH_TRIPLE_BYTES: usize = 12;

/// Reusable host-side staging for scan dispatches.
///
/// Engines that repeatedly scan many haystacks can keep one scratch value per
/// worker thread and pass it through `*_with_scratch` APIs. This removes the
/// fixed haystack-packing allocation from every dispatch while preserving the
/// same borrowed-input backend contract.
#[derive(Debug, Default)]
pub struct ScanDispatchScratch {
    /// Packed little-endian `u32` haystack bytes.
    pub haystack_bytes: Vec<u8>,
    /// Optional zeroed hit-buffer staging used by single-buffer hit layouts.
    pub hit_bytes: Vec<u8>,
}

/// Pack a haystack of bytes into `u32` little-endian words ready for an
/// input storage buffer. Each 4 input bytes become one little-endian
/// `u32`; a tail less than 4 bytes is zero-padded into the high lanes.
///
/// This is the layout every vyre matcher's `BufferDecl::storage(..,
/// DataType::U32, ReadOnly)` haystack input expects.
///
/// # Panics
///
/// Aborts when padded length arithmetic or allocation fails. Returning an empty
/// packed buffer would make the GPU scan an EMPTY haystack — silently finding
/// nothing and reporting the real input as clean (Law 10). Fail closed instead;
/// callers that must recover use [`try_pack_haystack_u32`].
#[must_use]
pub fn pack_haystack_u32(haystack: &[u8]) -> Vec<u8> {
    match try_pack_haystack_u32(haystack) {
        Ok(packed) => packed,
        Err(error) => {
            panic!(
                "vyre-libs scan dispatch pack_haystack_u32 failed: {error} — \
                 returning an empty packed buffer would make the GPU scan an empty haystack and silently report the input as clean; \
                 use try_pack_haystack_u32 and split the haystack before dispatch."
            )
        }
    }
}

/// Fallible owned variant of [`pack_haystack_u32`].
///
/// # Errors
///
/// Returns [`BackendError`] when padded length arithmetic or allocation fails.
pub fn try_pack_haystack_u32(haystack: &[u8]) -> Result<Vec<u8>, BackendError> {
    let mut packed = Vec::new();
    pack_haystack_u32_into(haystack, &mut packed)?;
    Ok(packed)
}

/// Pack a haystack into caller-owned scratch.
///
/// Clears `packed`, reserves the exact padded byte capacity, copies
/// `haystack`, and appends zero padding up to the next `u32` word boundary.
///
/// # Errors
///
/// Returns [`BackendError`] when padded length arithmetic or allocation fails.
pub fn pack_haystack_u32_into(haystack: &[u8], packed: &mut Vec<u8>) -> Result<(), BackendError> {
    let padded_len = haystack_padded_u32_byte_len(haystack.len())?;
    packed.clear();
    vyre_foundation::allocation::try_reserve_vec_to_capacity(packed, padded_len).map_err(
        |source| {
            BackendError::new(format!(
                "scan dispatch could not reserve {padded_len} packed haystack byte(s): {source}. Fix: split the haystack before dispatch."
            ))
        },
    )?;
    packed.extend_from_slice(haystack);
    packed.resize(padded_len, 0);
    Ok(())
}

/// Byte length of `byte_len` haystack bytes packed and zero-padded to the next
/// `u32` word boundary — the exact size a resident haystack buffer must be
/// allocated at so [`pack_haystack_u32_into`] output uploads in place.
pub fn haystack_padded_u32_byte_len(byte_len: usize) -> Result<usize, BackendError> {
    byte_len
        .checked_add(3)
        .map(|len| (len / 4) * 4)
        .ok_or_else(|| {
            BackendError::new(
                "scan dispatch haystack padding overflows host usize. Fix: split the haystack before dispatch.",
            )
        })
}

#[cfg(test)]
mod scratch_reuse_tests {
    use super::{
        haystack_padded_u32_byte_len, pack_haystack_u32, pack_haystack_u32_into,
        try_pack_haystack_u32, ScanDispatchScratch,
    };

    #[test]
    fn pack_haystack_into_reuses_capacity_and_matches_owned_helper() {
        let mut scratch = ScanDispatchScratch::default();
        pack_haystack_u32_into(b"abcdef", &mut scratch.haystack_bytes)
            .expect("Fix: packed haystack scratch should reserve");
        let retained = scratch.haystack_bytes.capacity();
        assert_eq!(scratch.haystack_bytes, pack_haystack_u32(b"abcdef"));

        pack_haystack_u32_into(b"xy", &mut scratch.haystack_bytes)
            .expect("Fix: smaller packed haystack should reuse scratch");

        assert_eq!(scratch.haystack_bytes, vec![b'x', b'y', 0, 0]);
        assert!(scratch.haystack_bytes.capacity() >= retained);
    }

    #[test]
    fn try_pack_haystack_owned_matches_compat_helper() {
        let packed = try_pack_haystack_u32(b"abcde")
            .expect("Fix: small owned haystack packing must reserve");

        assert_eq!(packed, pack_haystack_u32(b"abcde"));
        assert_eq!(packed, vec![b'a', b'b', b'c', b'd', b'e', 0, 0, 0]);
    }

    #[test]
    fn haystack_padding_overflow_reports_split_fix() {
        let error = haystack_padded_u32_byte_len(usize::MAX)
            .expect_err("Fix: usize::MAX padding must overflow instead of wrapping");
        let message = format!("{error}");

        assert!(message.contains("padding overflows host usize"));
        assert!(message.contains("Fix: split the haystack"));
    }
}

/// Pack a `&[u32]` into a little-endian `Vec<u8>` suitable for upload
/// to a storage buffer of type `DataType::U32`.
#[must_use]
pub fn pack_u32_slice(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

/// Borrow a `u32` slice as little-endian bytes on little-endian hosts,
/// falling back to an owned conversion on big-endian targets.
#[must_use]
pub fn u32_words_as_le_bytes(words: &[u32]) -> Cow<'_, [u8]> {
    if cfg!(target_endian = "little") {
        Cow::Borrowed(bytemuck::cast_slice(words))
    } else {
        Cow::Owned(pack_u32_slice(words))
    }
}

/// Validate that `haystack.len()` fits in a `u32` and return it. Vyre's
/// IR uses `u32` for buffer indices, and most matching kernels rely on
/// it indirectly via 4 GiB-bounded loop counters; the check belongs at
/// the dispatch boundary so the user-facing error message points at the
/// real fix (split the input).
///
/// # Errors
/// Returns a `BackendError` carrying the message
/// `"<context> haystack length exceeds u32 capacity. Fix: split the
/// scan into chunks smaller than 4 GiB."` so callers can include their
/// engine name in the surfaced diagnostic.
pub fn haystack_len_u32(haystack: &[u8], context: &str) -> Result<u32, BackendError> {
    u32::try_from(haystack.len()).map_err(|_| {
        BackendError::new(format!(
            "{context} haystack length exceeds u32 capacity. \
             Fix: split the scan into chunks smaller than 4 GiB."
        ))
    })
}

/// Default scan-guard ceiling. Picked at 1 GiB on the assumption that
/// a single GPU dispatch over more than 1 GiB of haystack is almost
/// always a caller bug  -  fragmenting at this granularity keeps device
/// allocations bounded and lets failed segments retry independently.
/// Callers that genuinely need the full u32 range pass `u32::MAX` to
/// [`scan_guard`].
pub const DEFAULT_MAX_SCAN_BYTES: u32 = 1 << 30;

/// Pre-dispatch length check: enforce both the hard `u32` cap (the IR
/// limit) **and** a configurable `max_bytes` ceiling (the
/// caller-policy limit) in one call. Returns the validated length so
/// callers don't need a separate `u32::try_from` site.
///
/// This is the single source of truth for "how big a haystack will
/// vyre accept on this dispatch?"  -  every matcher in `vyre-libs` is
/// expected to call it before assembling input buffers, so the
/// surface message on overflow is uniform across engines.
///
/// # Errors
/// Returns a [`BackendError`] when:
/// - `haystack.len()` exceeds `u32::MAX` (carries the
///   `haystack_len_u32` overflow message).
/// - `haystack.len()` exceeds `max_bytes` (carries a
///   `Fix: split the scan…` message that names the limit).
pub fn scan_guard(haystack: &[u8], context: &str, max_bytes: u32) -> Result<u32, BackendError> {
    let len = haystack_len_u32(haystack, context)?;
    if len > max_bytes {
        return Err(BackendError::new(format!(
            "{context} haystack length {len} bytes exceeds scan-guard ceiling {max_bytes} bytes. \
             Fix: split the scan into chunks <= {max_bytes} bytes, or pass a larger \
             max_bytes if the larger dispatch is intentional."
        )));
    }
    Ok(len)
}

/// Compute the standard "one workgroup per `workgroup_size[0]` haystack
/// bytes" grid geometry. Every byte-scan matcher in `vyre-libs::matching`
/// uses the same X-axis lane fan-out, so callers should not duplicate
/// this divceil-clamp arithmetic at every dispatch site.
#[must_use]
pub fn byte_scan_dispatch_config(haystack_len: u32, workgroup_x: u32) -> DispatchConfig {
    let mut config = DispatchConfig::default();
    let workgroups = haystack_len.div_ceil(workgroup_x.max(1)).max(1);
    config.grid_override = Some([workgroups, 1, 1]);
    config
}

/// Compute grid geometry for matchers that assign one workgroup to
/// each candidate start offset. Subgroup-local lanes cooperate inside
/// that workgroup to advance the automaton state, so X-grid density is
/// the input byte count rather than `haystack_len / workgroup_size`.
#[must_use]
pub fn candidate_start_dispatch_config(haystack_len: u32) -> DispatchConfig {
    let mut config = DispatchConfig::default();
    config.grid_override = Some([haystack_len.max(1), 1, 1]);
    config
}

/// Decode a little-endian scan counter from the first four bytes of a backend
/// readback buffer.
///
/// # Errors
///
/// Returns [`BackendError`] when the readback is shorter than one `u32`.
pub fn try_read_u32_prefix(bytes: &[u8], field: &'static str) -> Result<u32, BackendError> {
    if bytes.len() < U32_COUNTER_BYTES {
        return Err(BackendError::new(format!(
            "scan dispatch {field} was {} byte(s) but a u32 counter requires {U32_COUNTER_BYTES} bytes. Fix: preserve the counter output byte range before decoding scan results.",
            bytes.len()
        )));
    }

    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Borrow one backend output buffer by declaration index.
///
/// # Errors
///
/// Returns [`BackendError`] when the backend omitted a declared output slot.
pub fn try_output_bytes<'a>(
    outputs: &'a [Vec<u8>],
    index: usize,
    field: &'static str,
) -> Result<&'a [u8], BackendError> {
    outputs.get(index).map(Vec::as_slice).ok_or_else(|| {
        BackendError::new(format!(
            "scan dispatch missing {field} at output index {index}; backend returned {} output buffer(s). Fix: preserve Program output declaration order and return every declared output buffer.",
            outputs.len()
        ))
    })
}

/// Decode a packed match-triple buffer (`pid, start, end` × N) into
/// [`vyre_foundation::match_result::Match`] values. The triple layout is
/// shared between `GpuLiteralSet` and `RulePipeline`; only the *position*
/// of the buffer in the dispatch outputs differs.
///
/// Decodes at most `count` triples and never reads past a complete 12-byte
/// record, so the returned length is
/// `min(count, triples_bytes.len() / 12)`. Extra bytes after the last full
/// triple are ignored. Using a `usize` lane index keeps `i * 12` inside
/// buffer-derived bounds and avoids `(i as usize) * 12` wrapping on 32-bit
/// targets when `count` is large but the buffer is short.
#[must_use]
pub fn unpack_match_triples(
    triples_bytes: &[u8],
    count: u32,
) -> Vec<vyre_foundation::match_result::Match> {
    match try_unpack_match_triples(triples_bytes, count) {
        Ok(results) => results,
        Err(error) => {
            // Returning an empty match set would silently drop every match the
            // GPU actually found — a recall-loss silent fallback (Law 10). Fail
            // closed; callers that must recover use try_unpack_match_triples.
            panic!(
                "vyre-libs scan dispatch unpack_match_triples failed: {error} — \
                 returning an empty match set would silently drop matches the GPU found; \
                 use try_unpack_match_triples and reduce the match count or reserve more storage."
            )
        }
    }
}

/// Fallible owned variant of [`unpack_match_triples`].
///
/// # Errors
///
/// Returns [`BackendError`] when decoded match storage cannot be reserved.
pub fn try_unpack_match_triples(
    triples_bytes: &[u8],
    count: u32,
) -> Result<Vec<vyre_foundation::match_result::Match>, BackendError> {
    let mut results = Vec::new();
    try_unpack_match_triples_into(triples_bytes, count, &mut results)?;
    Ok(results)
}

/// Caller-owned variant of [`unpack_match_triples`].
///
/// Reuses `results` across dispatches and therefore removes one hot
/// allocation from benchmark loops and long-running daemons. The decode
/// contract is identical to [`unpack_match_triples`]: at most `count`
/// complete triples are read, truncated tail bytes are ignored, and the
/// final output is sorted by [`vyre_foundation::match_result::Match`]'s
/// ordering.
pub fn unpack_match_triples_into(
    triples_bytes: &[u8],
    count: u32,
    results: &mut Vec<vyre_foundation::match_result::Match>,
) {
    if let Err(error) = try_unpack_match_triples_into(triples_bytes, count, results) {
        // Clearing `results` would silently drop every match the GPU found — a
        // recall-loss silent fallback (Law 10). Fail closed; callers that must
        // recover use try_unpack_match_triples_into.
        panic!(
            "vyre-libs scan dispatch unpack_match_triples_into failed: {error} — \
             clearing the result buffer would silently drop matches the GPU found; \
             use try_unpack_match_triples_into and reduce the match count or reserve more storage."
        )
    }
}

/// Fallible caller-owned variant of [`unpack_match_triples_into`].
///
/// # Errors
///
/// Returns [`BackendError`] when decoded match storage cannot be reserved.
pub fn try_unpack_match_triples_into(
    triples_bytes: &[u8],
    count: u32,
    results: &mut Vec<vyre_foundation::match_result::Match>,
) -> Result<(), BackendError> {
    let n = decoded_match_triple_count(triples_bytes, count);
    vyre_foundation::allocation::try_reserve_vec_to_capacity(results, n).map_err(|source| {
        BackendError::new(format!(
            "scan dispatch could not reserve {n} decoded match record(s): {source}. Fix: lower max_matches or split the scan before dispatch."
        ))
    })?;
    results.clear();
    for i in 0..n {
        let off = i * 12;
        let pid = u32::from_le_bytes([
            triples_bytes[off],
            triples_bytes[off + 1],
            triples_bytes[off + 2],
            triples_bytes[off + 3],
        ]);
        let start = u32::from_le_bytes([
            triples_bytes[off + 4],
            triples_bytes[off + 5],
            triples_bytes[off + 6],
            triples_bytes[off + 7],
        ]);
        let end = u32::from_le_bytes([
            triples_bytes[off + 8],
            triples_bytes[off + 9],
            triples_bytes[off + 10],
            triples_bytes[off + 11],
        ]);
        results.push(vyre_foundation::match_result::Match::new(pid, start, end));
    }
    results.sort_unstable();
    Ok(())
}

/// Strict caller-owned variant for bounded scan readbacks.
///
/// Unlike [`try_unpack_match_triples_into`], this helper rejects a backend
/// buffer that cannot hold exactly the `count` complete triples requested by
/// the caller; short buffers are backend/readback corruption, not a successful
/// partial decode.
///
/// `count` MUST already be proven `<= max_matches` (the fixed output-buffer
/// capacity). Do NOT pass `count.min(max_matches)` here to clamp an
/// over-capacity kernel counter: that hands back a truncated set the caller
/// cannot distinguish from a complete one — a SILENT dropped-match false
/// negative (Law 10). Route every fixed-capacity GPU match readback through
/// [`try_unpack_match_triples_capped_into`], which fails closed on overflow and
/// only then calls this.
///
/// # Errors
///
/// Returns [`BackendError`] when `count` cannot be represented on this host,
/// when the required byte length overflows `usize`, when the readback is too
/// short for `count`, or when decoded match storage cannot be reserved.
pub fn try_unpack_match_triples_exact_prefix_into(
    triples_bytes: &[u8],
    count: u32,
    results: &mut Vec<vyre_foundation::match_result::Match>,
) -> Result<(), BackendError> {
    results.clear();
    let required = required_match_triple_bytes(count)?;
    if triples_bytes.len() < required {
        return Err(BackendError::new(format!(
            "scan dispatch match triples readback was {} byte(s) but count={count} requires {required} byte(s). Fix: preserve the output byte range for the requested match cap before decoding scan results.",
            triples_bytes.len()
        )));
    }
    try_unpack_match_triples_into(triples_bytes, count, results)
}

/// Capacity-guarded match-triple decode: the ONE primitive every fixed-capacity
/// GPU match readback must use. Fails closed when the kernel's reported `count`
/// exceeds the output-buffer cap.
///
/// The GPU match counter is an atomic incremented for EVERY match the kernel
/// finds — including matches at slots past `cap` it could not write (the emit
/// is guarded by `slot < cap`, the counter is not). So `count > cap` is the
/// exact, host-detectable signal that `count - cap` matches were dropped.
/// Decoding the `min(count, cap)` prefix instead would return a truncated set
/// indistinguishable from a complete scan: a SILENT false negative — the worst
/// failure mode for a scanner (Law 10). This surfaces the overflow as an error
/// naming the shortfall and the fix; on success `count` is proven `<= cap` and
/// decoded exactly via [`try_unpack_match_triples_exact_prefix_into`].
///
/// `context` labels the readback (e.g. `"RulePipeline hit buffer"`) so the
/// error points the operator at the dispatch that overflowed.
///
/// # Errors
///
/// Returns [`BackendError`] when `count > cap` (overflow ⇒ dropped matches), or
/// for any error [`try_unpack_match_triples_exact_prefix_into`] raises.
pub fn try_unpack_match_triples_capped_into(
    triples_bytes: &[u8],
    count: u32,
    cap: u32,
    context: &str,
    results: &mut Vec<vyre_foundation::match_result::Match>,
) -> Result<(), BackendError> {
    if count > cap {
        results.clear();
        return Err(BackendError::new(format!(
            "{context}: GPU match count {count} exceeds the output-buffer cap {cap}; decoding would silently drop {} match(es). Fix: raise the match cap (max_matches) or split the scan before dispatch.",
            count - cap
        )));
    }
    try_unpack_match_triples_exact_prefix_into(triples_bytes, count, results)
}

#[inline]
fn decoded_match_triple_count(triples_bytes: &[u8], count: u32) -> usize {
    let max_complete = triples_bytes.len() / MATCH_TRIPLE_BYTES;
    let requested = match usize::try_from(count) {
        Ok(requested) => requested,
        Err(_) => usize::MAX,
    };
    requested.min(max_complete)
}

fn required_match_triple_bytes(count: u32) -> Result<usize, BackendError> {
    let n = usize::try_from(count).map_err(|source| {
        BackendError::new(format!(
            "scan dispatch match count does not fit host usize: {source}. Fix: lower max_matches or split the scan before dispatch."
        ))
    })?;
    n.checked_mul(MATCH_TRIPLE_BYTES).ok_or_else(|| {
        BackendError::new(
            "scan dispatch match triple byte count overflowed host usize. Fix: lower max_matches or split the scan before dispatch.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_io_wrappers_fail_loud_not_silent_fallback() {
        // Law 10 regression guard: the infallible pack/unpack wrappers must
        // never swallow an error into an empty buffer (GPU scans nothing / GPU
        // matches silently dropped). The old arms logged the failure and then
        // returned/cleared to empty instead of failing loud. Scan the WHOLE
        // file (this file has two test modules with production code between
        // them, so a "split on first cfg-test" slice would miss the unpack
        // region). The swallow marker is assembled with concat! and is NOT
        // written contiguously anywhere else in this file (including comments),
        // so this assertion never matches its own source text.
        let src = include_str!("dispatch_io.rs");
        let swallow_marker = concat!("eprintln", "!(\"vyre-libs scan dispatch ");
        assert!(
            !src.contains(swallow_marker),
            "Fix: a dispatch wrapper reintroduced an eprintln!-then-return-empty silent fallback (Law 10) — fail loud via panic!() so callers use the try_ variants."
        );
    }

    #[test]
    fn capped_decode_fails_closed_when_count_exceeds_cap() {
        // Law 10 regression: the kernel's atomic match counter overcounts past
        // the fixed output cap (the emit is `slot < cap`-guarded, the counter is
        // not), so `count > cap` means matches were dropped. The capped decode
        // MUST surface that, never hand back the silently-truncated prefix.
        // Forge 4 complete triples but a counter claiming 9 hits.
        let mut triples = Vec::new();
        for i in 0..4u32 {
            triples.extend_from_slice(&i.to_le_bytes()); // pattern_id
            triples.extend_from_slice(&i.to_le_bytes()); // start
            triples.extend_from_slice(&(i + 2).to_le_bytes()); // end
        }
        let mut results = vec![vyre_foundation::match_result::Match::new(7, 7, 7)];
        let err = try_unpack_match_triples_capped_into(&triples, 9, 4, "unit cap test", &mut results)
            .expect_err("count 9 over cap 4 must fail closed, not truncate");
        let msg = err.to_string();
        assert!(
            msg.contains("unit cap test")
                && msg.contains("exceeds the output-buffer cap 4")
                && msg.contains("drop 5 match(es)"),
            "error must name the context, cap, and dropped count: {msg}"
        );
        assert!(
            results.is_empty(),
            "a failed capped decode must expose no partial matches, got {results:?}"
        );
    }

    #[test]
    fn capped_decode_passes_and_decodes_exactly_within_cap() {
        // Within the cap the decode is exact: count == 3 of a 4-slot buffer
        // yields exactly those 3 triples (assert the real values, not is_empty).
        let mut triples = Vec::new();
        for i in 0..4u32 {
            triples.extend_from_slice(&(i + 10).to_le_bytes()); // pattern_id
            triples.extend_from_slice(&i.to_le_bytes()); // start
            triples.extend_from_slice(&(i + 1).to_le_bytes()); // end
        }
        let mut results = Vec::new();
        try_unpack_match_triples_capped_into(&triples, 3, 4, "unit within-cap", &mut results)
            .expect("count 3 within cap 4 must decode");
        assert_eq!(
            results,
            vec![
                vyre_foundation::match_result::Match::new(10, 0, 1),
                vyre_foundation::match_result::Match::new(11, 1, 2),
                vyre_foundation::match_result::Match::new(12, 2, 3),
            ],
            "within-cap decode must yield exactly the first `count` triples"
        );
        // The exact-cap boundary (count == cap) is allowed, not an overflow.
        let mut at_cap = Vec::new();
        try_unpack_match_triples_capped_into(&triples, 4, 4, "unit at-cap", &mut at_cap)
            .expect("count == cap is not an overflow");
        assert_eq!(at_cap.len(), 4, "count == cap must decode all four");
    }

    #[test]
    fn pack_haystack_aligned() {
        let bytes = b"abcdefgh";
        let packed = pack_haystack_u32(bytes);
        // Two LE u32 words: "abcd" → 0x64636261, "efgh" → 0x68676665.
        assert_eq!(packed, vec![0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68]);
    }

    #[test]
    fn pack_haystack_unaligned_zero_pads() {
        let bytes = b"abc";
        let packed = pack_haystack_u32(bytes);
        // Single u32: "abc\0" → 0x00636261. Tail high lane is 0.
        assert_eq!(packed, vec![0x61, 0x62, 0x63, 0x00]);
    }

    #[test]
    fn pack_haystack_empty() {
        assert!(pack_haystack_u32(&[]).is_empty());
    }

    #[test]
    fn pack_u32_slice_layout() {
        let words: [u32; 2] = [0x01020304, 0xAABBCCDD];
        assert_eq!(
            pack_u32_slice(&words),
            vec![0x04, 0x03, 0x02, 0x01, 0xDD, 0xCC, 0xBB, 0xAA]
        );
    }

    #[test]
    fn u32_words_as_le_bytes_matches_pack_layout() {
        let words: [u32; 2] = [0x01020304, 0xAABBCCDD];
        let bytes = u32_words_as_le_bytes(&words);
        assert_eq!(
            bytes.as_ref(),
            [0x04, 0x03, 0x02, 0x01, 0xDD, 0xCC, 0xBB, 0xAA]
        );
        if cfg!(target_endian = "little") {
            assert!(matches!(bytes, std::borrow::Cow::Borrowed(_)));
        }
    }

    #[test]
    fn haystack_len_under_4gib_ok() {
        let buf = vec![0u8; 1024];
        assert_eq!(haystack_len_u32(&buf, "test").unwrap(), 1024);
    }

    #[test]
    fn scan_guard_under_ceiling_ok() {
        let buf = vec![0u8; 1024];
        assert_eq!(
            scan_guard(&buf, "test", DEFAULT_MAX_SCAN_BYTES).unwrap(),
            1024
        );
    }

    #[test]
    fn scan_guard_over_ceiling_errors() {
        let buf = vec![0u8; 1024];
        let err = scan_guard(&buf, "test", 512).expect_err("over ceiling must err");
        let msg = format!("{err}");
        assert!(
            msg.contains("scan-guard ceiling"),
            "scan_guard error must name the ceiling, got: {msg}"
        );
        assert!(
            msg.contains("512"),
            "must echo the ceiling number, got: {msg}"
        );
    }

    #[test]
    fn scan_guard_zero_ceiling_rejects_nonempty() {
        let buf = vec![0u8; 1];
        let err = scan_guard(&buf, "ctx", 0).expect_err("nonempty haystack with zero ceiling");
        let msg = err.to_string();
        assert!(
            msg.contains("scan-guard ceiling") && msg.contains('0'),
            "zero-ceiling rejection must name the ceiling: {msg}"
        );
    }

    #[test]
    fn scan_guard_zero_ceiling_accepts_empty() {
        let buf: Vec<u8> = vec![];
        assert_eq!(scan_guard(&buf, "ctx", 0).unwrap(), 0);
    }

    #[test]
    fn scan_guard_at_max_u32_ceiling_accepts_real_inputs() {
        let buf = vec![0u8; 1 << 16];
        assert_eq!(scan_guard(&buf, "ctx", u32::MAX).unwrap(), 1 << 16);
    }

    #[test]
    fn dispatch_config_clamps_at_one() {
        // Haystack shorter than a single workgroup must still yield ≥1
        // workgroup so the kernel actually runs.
        let cfg = byte_scan_dispatch_config(0, 64);
        assert_eq!(cfg.grid_override, Some([1, 1, 1]));
    }

    #[test]
    fn dispatch_config_divceils() {
        let cfg = byte_scan_dispatch_config(129, 64);
        assert_eq!(cfg.grid_override, Some([3, 1, 1]));
    }

    #[test]
    fn unpack_match_triples_sorts() {
        let bytes = [
            // (pid=2, start=10, end=20)
            2, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, // (pid=1, start=5, end=8)
            1, 0, 0, 0, 5, 0, 0, 0, 8, 0, 0, 0,
        ];
        let matches = unpack_match_triples(&bytes, 2);
        assert_eq!(matches.len(), 2);
        // sort_unstable orders by (start, end, pid) via Match's Ord impl.
        assert!(matches[0].start <= matches[1].start);
    }

    #[test]
    fn unpack_match_triples_into_reuses_caller_buffer() {
        let bytes = [
            2, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, 1, 0, 0, 0, 5, 0, 0, 0, 8, 0, 0, 0,
        ];
        let mut matches = Vec::with_capacity(8);
        let ptr = matches.as_ptr();

        unpack_match_triples_into(&bytes, 2, &mut matches);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches.as_ptr(), ptr);
        assert!(matches[0].start <= matches[1].start);
    }

    #[test]
    fn try_unpack_match_triples_into_keeps_fallible_hot_path_reusable() {
        let bytes = [
            9, 0, 0, 0, 40, 0, 0, 0, 44, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 8, 0, 0, 0,
        ];
        let mut matches = Vec::with_capacity(4);
        let ptr = matches.as_ptr();

        try_unpack_match_triples_into(&bytes, 2, &mut matches)
            .expect("Fix: small decoded match buffer must reserve");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches.as_ptr(), ptr);
        assert_eq!(matches[0].pattern_id, 3);
        assert_eq!(matches[1].pattern_id, 9);
    }

    #[test]
    fn try_unpack_match_triples_owned_matches_compat_helper() {
        let bytes = [
            5, 0, 0, 0, 11, 0, 0, 0, 13, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0,
        ];

        assert_eq!(
            try_unpack_match_triples(&bytes, 2)
                .expect("Fix: small decoded match buffer must reserve"),
            unpack_match_triples(&bytes, 2)
        );
    }

    #[test]
    fn read_u32_prefix_decodes_counter_and_rejects_short_readback() {
        assert_eq!(
            try_read_u32_prefix(&[0x34, 0x12, 0, 0, 0xAA], "test counter")
                .expect("Fix: four-byte counter prefix must decode"),
            0x1234
        );

        let err = try_read_u32_prefix(&[1, 2, 3], "test counter")
            .expect_err("short scan counter readback must fail closed");
        let msg = err.to_string();
        assert!(
            msg.contains("test counter")
                && msg.contains("3 byte(s)")
                && msg.contains("requires 4 bytes"),
            "short counter error must name the field and required length: {msg}"
        );
    }

    #[test]
    fn output_bytes_rejects_missing_declared_output_slot() {
        let outputs = vec![vec![1, 2, 3, 4]];
        assert_eq!(
            try_output_bytes(&outputs, 0, "first").expect("Fix: present output slot must borrow"),
            &[1, 2, 3, 4]
        );

        let err = try_output_bytes(&outputs, 1, "matches")
            .expect_err("missing backend output slot must fail closed");
        let msg = err.to_string();
        assert!(
            msg.contains("matches")
                && msg.contains("output index 1")
                && msg.contains("returned 1 output buffer"),
            "missing output error must identify the omitted slot: {msg}"
        );
    }

    #[test]
    fn exact_prefix_match_decode_sorts_and_reuses_caller_buffer() {
        let bytes = [
            9, 0, 0, 0, 40, 0, 0, 0, 44, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 8, 0, 0, 0, 0xAA, 0xBB,
        ];
        let mut matches = Vec::with_capacity(4);
        let ptr = matches.as_ptr();

        try_unpack_match_triples_exact_prefix_into(&bytes, 2, &mut matches)
            .expect("Fix: exact two-triple prefix must decode");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches.as_ptr(), ptr);
        assert_eq!(matches[0].pattern_id, 3);
        assert_eq!(matches[1].pattern_id, 9);
    }

    #[test]
    fn exact_prefix_match_decode_rejects_short_payload_and_clears_results() {
        let bytes = [
            7u8, 0, 0, 0, // pid
            1, 0, 0, 0, // start
            3, 0, 0, 0, // end
        ];
        let mut matches = vec![vyre_foundation::match_result::Match::new(99, 1, 2)];

        let err = try_unpack_match_triples_exact_prefix_into(&bytes, 2, &mut matches)
            .expect_err("short match triple readback must fail closed");

        let msg = err.to_string();
        assert!(
            matches.is_empty(),
            "malformed readback must clear stale matches"
        );
        assert!(
            msg.contains("readback was 12 byte(s)")
                && msg.contains("count=2")
                && msg.contains("requires 24 byte(s)"),
            "short match readback error must identify observed and required bytes: {msg}"
        );
    }

    #[test]
    fn exact_prefix_match_decode_huge_count_short_payload_fails_closed() {
        let bytes = [
            7u8, 0, 0, 0, // pid
            1, 0, 0, 0, // start
            3, 0, 0, 0, // end
        ];
        let mut matches = vec![vyre_foundation::match_result::Match::new(99, 1, 2)];

        let err = try_unpack_match_triples_exact_prefix_into(&bytes, u32::MAX, &mut matches)
            .expect_err("huge count with short readback must fail closed");

        let msg = err.to_string();
        assert!(
            matches.is_empty(),
            "malformed readback must clear stale matches"
        );
        assert!(
            msg.contains("requires") || msg.contains("overflowed") || msg.contains("does not fit"),
            "huge-count error must report required size or host capacity: {msg}"
        );
    }

    /// Adversarial / regression: a bogus or truncated readback may pair a
    /// huge `count` (e.g. `u32::MAX`) with a short buffer. The decoder must
    /// only walk full 12-byte triples so we never form `off` from a wrapped
    /// `u32_index * 12` on 32-bit `usize` before comparing to `len`, and we
    /// return exactly the complete records present (not a silent under-filled
    /// long vec).
    #[test]
    fn unpack_match_triples_huge_count_short_buffer_stays_in_bounds() {
        let bytes = [
            7u8, 0, 0, 0, // pid
            1, 0, 0, 0, // start
            3, 0, 0, 0, // end
        ];
        let matches = unpack_match_triples(&bytes, u32::MAX);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_id, 7);
        assert_eq!(matches[0].start, 1);
        assert_eq!(matches[0].end, 3);
    }
}
