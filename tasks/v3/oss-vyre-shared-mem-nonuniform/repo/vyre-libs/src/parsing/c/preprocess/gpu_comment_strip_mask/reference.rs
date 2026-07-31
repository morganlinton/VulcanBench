// ---- reference oracle contract ----

/// Reference oracle: returns the same per-byte mask the GPU kernel emits.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_gpu_comment_strip_mask(source: &[u8]) -> Vec<u32> {
    let mut out = vec![0u32; source.len()];
    let mut in_line = false;
    let mut in_block = false;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut i = 0usize;
    while i < source.len() {
        let b = source[i];
        let b1 = source.get(i + 1).copied().unwrap_or(0);
        if !in_line && !in_block && !in_string && !in_char {
            if b == b'/' && spliced_next_byte(source, i) == Some(b'/') {
                in_line = true;
                out[i] = 2;
                i += 1;
                continue;
            }
            if b == b'/' && spliced_next_byte(source, i) == Some(b'*') {
                in_block = true;
                out[i] = 2;
                i += 1;
                continue;
            }
        }
        if !in_line && !in_block {
            out[i] = 0;
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
            } else if in_char {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'\'' {
                    in_char = false;
                }
            } else if b == b'"' {
                in_string = true;
                escaped = false;
            } else if b == b'\'' {
                in_char = true;
                escaped = false;
            }
            i += 1;
        } else if in_line {
            if b == b'\r' && b1 == b'\n' {
                out[i] = 0;
                out[i + 1] = 0;
                in_line = false;
                i += 2;
            } else if b == b'\n' {
                out[i] = 0;
                in_line = false;
                i += 1;
            } else if b == b'\\' && b1 == b'\n' {
                out[i] = 1;
                out[i + 1] = 1;
                i += 2;
            } else if b == b'\\' && b1 == b'\r' && source.get(i + 2).copied() == Some(b'\n') {
                out[i] = 1;
                out[i + 1] = 1;
                out[i + 2] = 1;
                i += 3;
            } else {
                out[i] = 1;
                i += 1;
            }
        } else if b == b'*' && spliced_next_byte(source, i) == Some(b'/') {
            out[i] = 1;
            if let Some(close_idx) = spliced_next_index(source, i) {
                for slot in out.iter_mut().take(close_idx + 1).skip(i + 1) {
                    *slot = 1;
                }
                i = close_idx + 1;
            } else {
                i += 1;
            }
            in_block = false;
        } else {
            out[i] = 1;
            i += 1;
        }
    }
    out
}

fn spliced_next_byte(source: &[u8], i: usize) -> Option<u8> {
    spliced_next_index(source, i).and_then(|idx| source.get(idx).copied())
}

fn spliced_next_index(source: &[u8], i: usize) -> Option<usize> {
    let next = i.checked_add(1)?;
    match (source.get(next).copied(), source.get(next + 1).copied()) {
        (Some(b'\\'), Some(b'\n')) => Some(next + 2),
        (Some(b'\\'), Some(b'\r')) if source.get(next + 2).copied() == Some(b'\n') => {
            Some(next + 3)
        }
        (Some(_), _) => Some(next),
        (None, _) => None,
    }
}
