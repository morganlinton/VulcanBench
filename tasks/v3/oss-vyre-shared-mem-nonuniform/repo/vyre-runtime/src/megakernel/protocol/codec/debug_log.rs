use super::{debug, read_required_word, read_word_from_optional_words, validate_word_aligned};
use super::{DebugRecord, ProtocolError};

/// Decode PRINTF records out of the debug-log buffer.
#[must_use]
pub fn read_debug_log(debug_bytes: &[u8]) -> Vec<DebugRecord> {
    try_read_debug_log(debug_bytes).unwrap_or_default()
}

/// Decode PRINTF records into caller-owned storage.
///
/// Clears `out`, then reuses its allocation.
pub fn read_debug_log_into(debug_bytes: &[u8], out: &mut Vec<DebugRecord>) {
    // Clearing to empty on a decode failure silently drops the PRINTF records
    // the kernel emitted — the operator sees "no debug output" instead of the
    // decode error (Law 10). Fail loud; callers use try_read_debug_log_into.
    if let Err(error) = try_read_debug_log_into(debug_bytes, out) {
        panic!("vyre-runtime debug-log decode failed: {error}");
    }
}

/// Strictly decode PRINTF records out of the debug-log buffer.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the buffer is not word-aligned, too short for
/// the cursor word, or the cursor points at a partial record.
pub fn try_read_debug_log(debug_bytes: &[u8]) -> Result<Vec<DebugRecord>, ProtocolError> {
    let mut records = Vec::new();
    try_read_debug_log_into(debug_bytes, &mut records)?;
    Ok(records)
}

/// Strictly decode PRINTF records into caller-owned storage.
///
/// Clears `out`, then reuses its allocation.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the buffer is not word-aligned, too short for
/// the cursor word, or the cursor points at a partial record.
pub fn try_read_debug_log_into(
    debug_bytes: &[u8],
    out: &mut Vec<DebugRecord>,
) -> Result<(), ProtocolError> {
    validate_word_aligned("debug_log", debug_bytes)?;
    let cursor = read_required_word(
        "debug_log",
        debug_bytes,
        debug_word_index(debug::CURSOR_WORD, "cursor word")?,
    )?;
    let record_words = debug_word_index(debug::RECORD_WORDS, "record word count")?;
    let records_start = debug_word_index(debug::RECORDS_BASE, "records base word")?;
    let total_word_capacity = debug_bytes.len() / 4;
    if total_word_capacity < records_start {
        return Err(ProtocolError::MissingWord {
            buffer: "debug_log",
            word_idx: records_start,
            byte_len: debug_bytes.len(),
            fix: "build debug-log bytes with encode_empty_debug_log",
        });
    }
    let capacity_words = total_word_capacity - records_start;
    let cursor = usize::try_from(cursor).map_err(|_| ProtocolError::ByteLengthOverflow {
        buffer: "debug_log",
        fix: "debug-log cursor does not fit host usize; keep protocol buffers within host addressable range",
    })?;
    if cursor > capacity_words {
        return Err(ProtocolError::MissingWord {
            buffer: "debug_log",
            word_idx: records_start + cursor,
            byte_len: debug_bytes.len(),
            fix: "debug-log cursor must stay within the encoded record capacity",
        });
    }
    let available = cursor;
    if available % record_words != 0 {
        return Err(ProtocolError::MissingWord {
            buffer: "debug_log",
            word_idx: records_start + available,
            byte_len: debug_bytes.len(),
            fix: "debug-log cursor must advance in whole PRINTF records",
        });
    }
    let record_count = available / record_words;
    out.clear();
    try_reserve_record_capacity(out, record_count)?;
    let words = bytemuck::try_cast_slice::<u8, u32>(debug_bytes).ok();
    for i in 0..record_count {
        let w = records_start + i * record_words;
        out.push(DebugRecord {
            fmt_id: read_word_from_optional_words(words, debug_bytes, w).ok_or(
                ProtocolError::MissingWord {
                buffer: "debug_log",
                word_idx: w,
                byte_len: debug_bytes.len(),
                fix: "decode only debug-log buffers produced by the matching megakernel protocol encoder",
            })?,
            args: [
                read_word_from_optional_words(words, debug_bytes, w + 1).ok_or(
                    ProtocolError::MissingWord {
                    buffer: "debug_log",
                    word_idx: w + 1,
                    byte_len: debug_bytes.len(),
                    fix: "decode only debug-log buffers produced by the matching megakernel protocol encoder",
                })?,
                read_word_from_optional_words(words, debug_bytes, w + 2).ok_or(
                    ProtocolError::MissingWord {
                    buffer: "debug_log",
                    word_idx: w + 2,
                    byte_len: debug_bytes.len(),
                    fix: "decode only debug-log buffers produced by the matching megakernel protocol encoder",
                })?,
                read_word_from_optional_words(words, debug_bytes, w + 3).ok_or(
                    ProtocolError::MissingWord {
                    buffer: "debug_log",
                    word_idx: w + 3,
                    byte_len: debug_bytes.len(),
                    fix: "decode only debug-log buffers produced by the matching megakernel protocol encoder",
                })?,
            ],
        });
    }
    Ok(())
}

fn debug_word_index(word: u32, label: &'static str) -> Result<usize, ProtocolError> {
    usize::try_from(word).map_err(|_| ProtocolError::ByteLengthOverflow {
        buffer: "debug_log",
        fix: match label {
            "cursor word" => "debug-log cursor word cannot fit host usize",
            "record word count" => "debug-log record word count cannot fit host usize",
            "records base word" => "debug-log records base word cannot fit host usize",
            _ => "debug-log word index cannot fit host usize",
        },
    })
}

fn try_reserve_record_capacity(
    out: &mut Vec<DebugRecord>,
    target_capacity: usize,
) -> Result<(), ProtocolError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(out, target_capacity).map_err(|_| {
        ProtocolError::ByteLengthOverflow {
            buffer: "debug_log",
            fix: "host debug-log decode could not reserve output records; reduce debug-log capacity or decode into a reused scratch vector",
        }
    })
}
