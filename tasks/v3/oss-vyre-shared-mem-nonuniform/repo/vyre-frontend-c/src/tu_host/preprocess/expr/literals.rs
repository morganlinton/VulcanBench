pub(super) fn parse_preproc_integer_literal(raw: &str) -> i128 {
    let literal = raw.trim_end_matches(|ch: char| matches!(ch, 'u' | 'U' | 'l' | 'L'));
    if literal.is_empty() {
        return 0;
    }
    let (digits, radix) = if let Some(hex) = literal
        .strip_prefix("0x")
        .or_else(|| literal.strip_prefix("0X"))
    {
        (hex, 16)
    } else if let Some(binary) = literal
        .strip_prefix("0b")
        .or_else(|| literal.strip_prefix("0B"))
    {
        (binary, 2)
    } else if literal.len() > 1 && literal.starts_with('0') {
        (&literal[1..], 8)
    } else {
        (literal, 10)
    };
    let digits = if digits.is_empty() { "0" } else { digits };
    let _ = raw;
    i128::from_str_radix(digits, radix).unwrap_or(0)
}

pub(super) fn parse_preproc_char_literal(src: &str, start: usize) -> (i128, usize) {
    let bytes = src.as_bytes();
    let mut i = start + 1;
    let mut value = 0i128;
    let mut units = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if units == 0 {
                panic!("empty character literal ''");
            }
            return (value, i + 1);
        }
        let unit = if bytes[i] == b'\\' {
            let (escaped, next) = parse_preproc_escape(src, i);
            i = next;
            escaped
        } else {
            let unit = bytes[i] as u32;
            i += 1;
            unit
        };
        value = value
            .checked_shl(8)
            .and_then(|shifted| shifted.checked_add(i128::from(unit & 0xff)))
            .unwrap_or(i128::MAX);
        units += 1;
    }
    let _ = src;
    (value, bytes.len())
}

pub(super) fn parse_preproc_escape(src: &str, slash: usize) -> (u32, usize) {
    let bytes = src.as_bytes();
    let Some(&escaped) = bytes.get(slash + 1) else {
        return (0, bytes.len());
    };
    match escaped {
        b'\'' => (u32::from(b'\''), slash + 2),
        b'"' => (u32::from(b'"'), slash + 2),
        b'?' => (u32::from(b'?'), slash + 2),
        b'\\' => (u32::from(b'\\'), slash + 2),
        b'a' => (7, slash + 2),
        b'b' => (8, slash + 2),
        b'f' => (12, slash + 2),
        b'n' => (10, slash + 2),
        b'r' => (13, slash + 2),
        b't' => (9, slash + 2),
        b'v' => (11, slash + 2),
        b'x' => parse_hex_escape(src, slash + 2),
        b'0'..=b'7' => parse_octal_escape(src, slash + 1),
        other => (u32::from(other), slash + 2),
    }
}

pub(super) fn parse_hex_escape(src: &str, mut i: usize) -> (u32, usize) {
    let bytes = src.as_bytes();
    let start = i;
    let mut value = 0u32;
    while let Some(&byte) = bytes.get(i) {
        let Some(digit) = (byte as char).to_digit(16) else {
            break;
        };
        value = value.saturating_mul(16).saturating_add(digit);
        i += 1;
    }
    if i == start {
        return (0, start);
    }
    let _ = src;
    (value, i)
}

pub(super) fn parse_octal_escape(src: &str, mut i: usize) -> (u32, usize) {
    let bytes = src.as_bytes();
    let mut value = 0u32;
    let mut digits = 0usize;
    while digits < 3 {
        let Some(&byte) = bytes.get(i) else {
            break;
        };
        if !(b'0'..=b'7').contains(&byte) {
            break;
        }
        value = value * 8 + u32::from(byte - b'0');
        i += 1;
        digits += 1;
    }
    (value, i)
}
