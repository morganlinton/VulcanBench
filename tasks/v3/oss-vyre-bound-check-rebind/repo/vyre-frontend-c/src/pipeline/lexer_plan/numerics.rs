use super::*;
pub(super) fn sparse_numeric_literal_end(source: &[u8], start: usize) -> usize {
    let mut cursor = start;
    let mut exponent_allows_sign = false;
    while let Some(byte) = source.get(cursor).copied() {
        match byte {
            b'\'' | b'.' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9' => {
                exponent_allows_sign = matches!(byte, b'e' | b'E' | b'p' | b'P');
                cursor += 1;
            }
            b'+' | b'-' if exponent_allows_sign => {
                exponent_allows_sign = false;
                cursor += 1;
            }
            _ => break,
        }
    }
    cursor
}

pub(super) fn sparse_numeric_literal_supported(source: &[u8], start: usize) -> bool {
    let Some(first) = source.get(start).copied() else {
        return false;
    };
    if first == b'.' {
        return sparse_decimal_numeric_literal_supported(source, start, true);
    }
    let hex = matches!(source.get(start).copied(), Some(b'0'))
        && matches!(source.get(start + 1).copied(), Some(b'x') | Some(b'X'));
    let binary = matches!(source.get(start).copied(), Some(b'0'))
        && matches!(source.get(start + 1).copied(), Some(b'b') | Some(b'B'));
    let supported = if hex {
        sparse_hex_numeric_literal_supported(source, start)
    } else if binary {
        sparse_binary_numeric_literal_supported(source, start)
    } else {
        sparse_decimal_numeric_literal_supported(source, start, false)
    };
    supported && sparse_numeric_literal_end(source, start) - start <= CUDA_SPARSE_LEX_MAX_TOKEN_SCAN
}

pub(super) fn sparse_decimal_numeric_literal_supported(
    source: &[u8],
    start: usize,
    starts_with_dot: bool,
) -> bool {
    let mut cursor = start;
    let mut is_float = starts_with_dot;
    if starts_with_dot {
        cursor += 1;
        if !matches!(source.get(cursor).copied(), Some(b'0'..=b'9')) {
            return false;
        }
    }
    let digits_before_dot = consume_separated_digits(source, &mut cursor, digit10);
    if !starts_with_dot && digits_before_dot == 0 {
        return false;
    }
    if !starts_with_dot && matches!(source.get(cursor).copied(), Some(b'.')) {
        is_float = true;
        cursor += 1;
        consume_separated_digits(source, &mut cursor, digit10);
    }
    if matches!(source.get(cursor).copied(), Some(b'e') | Some(b'E')) {
        is_float = true;
        cursor += 1;
        if matches!(source.get(cursor).copied(), Some(b'+') | Some(b'-')) {
            cursor += 1;
        }
        if consume_separated_digits(source, &mut cursor, digit10) == 0 {
            return false;
        }
    }
    consume_numeric_suffix(source, &mut cursor, is_float)
        && cursor == sparse_numeric_literal_end(source, start)
}

pub(super) fn sparse_hex_numeric_literal_supported(source: &[u8], start: usize) -> bool {
    let mut cursor = start + 2;
    let digits_before_dot = consume_separated_digits(source, &mut cursor, digit16);
    let mut is_float = false;
    let mut digits_after_dot = 0;
    if matches!(source.get(cursor).copied(), Some(b'.')) {
        is_float = true;
        cursor += 1;
        digits_after_dot = consume_separated_digits(source, &mut cursor, digit16);
    }
    if digits_before_dot + digits_after_dot == 0 {
        return false;
    }
    if matches!(source.get(cursor).copied(), Some(b'p') | Some(b'P')) {
        is_float = true;
        cursor += 1;
        if matches!(source.get(cursor).copied(), Some(b'+') | Some(b'-')) {
            cursor += 1;
        }
        if consume_separated_digits(source, &mut cursor, digit10) == 0 {
            return false;
        }
    } else if is_float {
        return false;
    }
    consume_numeric_suffix(source, &mut cursor, is_float)
        && cursor == sparse_numeric_literal_end(source, start)
}

pub(super) fn sparse_binary_numeric_literal_supported(source: &[u8], start: usize) -> bool {
    let mut cursor = start + 2;
    if consume_separated_digits(source, &mut cursor, digit2) == 0 {
        return false;
    }
    consume_numeric_suffix(source, &mut cursor, false)
        && cursor == sparse_numeric_literal_end(source, start)
}

pub(super) fn consume_separated_digits(
    source: &[u8],
    cursor: &mut usize,
    digit: fn(u8) -> bool,
) -> usize {
    let mut count = 0usize;
    let mut previous_was_separator = false;
    while let Some(byte) = source.get(*cursor).copied() {
        if digit(byte) {
            previous_was_separator = false;
            count += 1;
            *cursor += 1;
            continue;
        }
        if byte == b'\'' {
            if previous_was_separator
                || !matches!(source.get(*cursor + 1).copied(), Some(next) if digit(next))
            {
                break;
            }
            previous_was_separator = true;
            *cursor += 1;
            continue;
        }
        break;
    }
    count
}

pub(super) fn consume_numeric_suffix(source: &[u8], cursor: &mut usize, is_float: bool) -> bool {
    let suffix_start = *cursor;
    while let Some(byte) = source.get(*cursor).copied() {
        let allowed = if is_float {
            matches!(byte, b'f' | b'F' | b'l' | b'L')
        } else {
            matches!(byte, b'u' | b'U' | b'l' | b'L')
        };
        if !allowed {
            break;
        }
        *cursor += 1;
    }
    if is_float {
        *cursor - suffix_start <= 1
    } else {
        valid_integer_suffix(&source[suffix_start..*cursor])
    }
}

pub(super) fn valid_integer_suffix(suffix: &[u8]) -> bool {
    matches!(
        suffix,
        [] | [b'u']
            | [b'U']
            | [b'l']
            | [b'L']
            | [b'l', b'l']
            | [b'L', b'L']
            | [b'u', b'l']
            | [b'u', b'L']
            | [b'U', b'l']
            | [b'U', b'L']
            | [b'l', b'u']
            | [b'l', b'U']
            | [b'L', b'u']
            | [b'L', b'U']
            | [b'u', b'l', b'l']
            | [b'u', b'L', b'L']
            | [b'U', b'l', b'l']
            | [b'U', b'L', b'L']
            | [b'l', b'l', b'u']
            | [b'l', b'l', b'U']
            | [b'L', b'L', b'u']
            | [b'L', b'L', b'U']
    )
}

pub(super) fn digit2(byte: u8) -> bool {
    matches!(byte, b'0' | b'1')
}

pub(super) fn digit10(byte: u8) -> bool {
    byte.is_ascii_digit()
}

pub(super) fn digit16(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}
