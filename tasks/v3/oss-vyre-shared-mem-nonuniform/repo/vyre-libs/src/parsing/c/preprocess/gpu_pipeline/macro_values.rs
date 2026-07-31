//! Object-like macro integer folding for live conditional evaluation.
//!
//! This file owns the host-side packing semantics for the integer-value
//! buffer consumed by `gpu_if_expression`. Macro table layout, replacement
//! token packing, and pipeline orchestration are intentionally elsewhere.

use crate::parsing::c::preprocess::gpu_pipeline::MacroDef;
use rustc_hash::FxHashMap as HashMap;

fn reserve_macro_value_vec<T>(
    target: &mut Vec<T>,
    additional: usize,
    label: &str,
) -> Result<(), String> {
    target.try_reserve_exact(additional).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {additional} {label}: {error:?}. Fix: shard macro integer folding before GPU conditional evaluation."
        )
    })
}

pub(super) fn macro_integer_values(macros: &[MacroDef]) -> Result<Vec<u32>, String> {
    let mut values = Vec::new();
    reserve_macro_value_vec(&mut values, macros.len(), "macro integer values")?;
    values.resize(macros.len(), 0u32);
    if macros.is_empty() {
        return Ok(values);
    }
    let mut macro_indexes: HashMap<&[u8], usize> = HashMap::default();
    macro_indexes.try_reserve(macros.len()).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {} macro integer index entries: {error:?}. Fix: shard macro integer folding before GPU conditional evaluation.",
            macros.len()
        )
    })?;
    for (idx, mac) in macros.iter().enumerate() {
        macro_indexes.insert(mac.name.as_slice(), idx);
    }
    let mut dependents = Vec::new();
    reserve_macro_value_vec(&mut dependents, macros.len(), "macro dependency buckets")?;
    dependents.resize_with(macros.len(), Vec::new);
    let mut unresolved_counts = Vec::new();
    reserve_macro_value_vec(
        &mut unresolved_counts,
        macros.len(),
        "macro unresolved dependency counters",
    )?;
    unresolved_counts.resize(macros.len(), 0usize);
    let mut seen_dependency_marks = Vec::new();
    reserve_macro_value_vec(
        &mut seen_dependency_marks,
        macros.len(),
        "macro dependency dedupe marks",
    )?;
    seen_dependency_marks.resize(macros.len(), usize::MAX);
    for (idx, mac) in macros.iter().enumerate() {
        if mac.is_function_like {
            continue;
        }
        collect_macro_body_identifiers(&mac.body, |ident| {
            let Some(dep_idx) = macro_indexes.get(ident).copied() else {
                return Ok(());
            };
            if dep_idx == idx {
                unresolved_counts[idx] = unresolved_counts[idx].saturating_add(1);
            } else if seen_dependency_marks[dep_idx] != idx {
                seen_dependency_marks[dep_idx] = idx;
                unresolved_counts[idx] = unresolved_counts[idx].saturating_add(1);
                dependents[dep_idx].try_reserve(1).map_err(|error| {
                    format!(
                        "vyre-libs::gpu_pipeline: could not reserve macro dependency edge: {error:?}. Fix: shard macro integer folding before GPU conditional evaluation."
                    )
                })?;
                dependents[dep_idx].push(idx);
            }
            Ok(())
        })?;
    }
    let mut ready = Vec::new();
    reserve_macro_value_vec(&mut ready, macros.len(), "ready macro queue entries")?;
    ready.extend(
        unresolved_counts
            .iter()
            .enumerate()
            .filter_map(|(idx, count)| (*count == 0).then_some(idx)),
    );
    let mut resolved = Vec::new();
    reserve_macro_value_vec(&mut resolved, macros.len(), "resolved macro flags")?;
    resolved.resize(macros.len(), false);
    while let Some(idx) = ready.pop() {
        if resolved[idx] {
            continue;
        }
        let next = object_like_macro_value(&macros[idx], &macro_indexes, &values);
        values[idx] = next;
        resolved[idx] = true;
        for dependent in std::mem::take(&mut dependents[idx]) {
            unresolved_counts[dependent] = unresolved_counts[dependent].saturating_sub(1);
            if unresolved_counts[dependent] == 0 {
                ready.push(dependent);
            }
        }
    }
    Ok(values)
}

pub(super) fn macro_integer_values_with_builtin_prefix(
    macros: &[MacroDef],
) -> Result<Vec<u32>, String> {
    let user_values = macro_integer_values(macros)?;
    let builtin_slots =
        crate::parsing::c::parse::gnu_builtins::GPU_BUILTIN_HASH_TABLE_SIZE as usize;
    let mut values = Vec::new();
    reserve_macro_value_vec(
        &mut values,
        builtin_slots.checked_add(user_values.len()).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: macro integer builtin prefix length overflows usize. Fix: shard macro integer folding before GPU conditional evaluation.".to_string()
        })?,
        "macro integer builtin-prefixed values",
    )?;
    values.resize(builtin_slots, 0);
    values.extend_from_slice(&user_values);
    Ok(values)
}

fn collect_macro_body_identifiers(
    mut body: &[u8],
    mut visit: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    while let Some((&byte, rest)) = body.split_first() {
        if byte.is_ascii_digit() || (byte == b'.' && rest.first().is_some_and(u8::is_ascii_digit)) {
            let end = scan_numeric_literal(body);
            body = &body[end..];
        } else if body.starts_with(b"/*") {
            let end = scan_block_comment(body);
            body = &body[end..];
        } else if body.starts_with(b"//") {
            break;
        } else if byte == b'\'' || byte == b'"' {
            let end = scan_quoted_literal(body, byte);
            body = &body[end..];
        } else if is_ident_start(byte) {
            let end = scan_while(body, 1, is_ident_continue);
            visit(&body[..end])?;
            body = &body[end..];
        } else {
            body = rest;
        }
    }
    Ok(())
}

fn scan_numeric_literal(body: &[u8]) -> usize {
    let mut index = 0usize;
    while let Some(byte) = body.get(index).copied() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'\'' | b'.' | b'+' | b'-') {
            index += 1;
        } else {
            break;
        }
    }
    index.max(1)
}

fn scan_quoted_literal(body: &[u8], quote: u8) -> usize {
    let mut index = 1usize;
    while let Some(byte) = body.get(index).copied() {
        index += 1;
        if byte == b'\\' {
            index += usize::from(index < body.len());
            continue;
        }
        if byte == quote {
            break;
        }
    }
    index
}

fn scan_block_comment(body: &[u8]) -> usize {
    let mut index = 2usize;
    while index + 1 < body.len() {
        if body[index] == b'*' && body[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    body.len()
}

fn object_like_macro_value(
    mac: &MacroDef,
    macro_indexes: &HashMap<&[u8], usize>,
    values: &[u32],
) -> u32 {
    if mac.is_function_like {
        return 0;
    }
    parse_object_like_integer_macro_with_idents(&mac.body, macro_indexes, values)
        .unwrap_or_else(|| u32::from(mac.body.is_empty()))
}

fn parse_object_like_integer_macro_with_idents(
    body: &[u8],
    macro_indexes: &HashMap<&[u8], usize>,
    values: &[u32],
) -> Option<u32> {
    let mut parser = MacroIntegerParser {
        body,
        index: 0,
        macro_indexes,
        values,
        depth: 0,
    };
    let value = parser.parse_conditional_expression()?;
    parser.skip_ws();
    (parser.index == body.len()).then_some(value)
}

struct MacroIntegerParser<'a> {
    body: &'a [u8],
    index: usize,
    macro_indexes: &'a HashMap<&'a [u8], usize>,
    values: &'a [u32],
    /// Recursion depth of the integer-macro conditional evaluator, bounded by
    /// [`crate::parsing::c::preprocess::expr_parser::MAX_PP_EXPR_DEPTH`] so a
    /// deeply-nested object-like macro body (e.g. `#define A ((((...))))`)
    /// fails closed (`None`) instead of overflowing the native stack and
    /// aborting the process during expansion.
    depth: usize,
}

impl MacroIntegerParser<'_> {
    fn parse_conditional_expression(&mut self) -> Option<u32> {
        // Guard the ternary recursion (and, via parens in
        // `parse_primary_expression`, all parenthesized nesting). Fail closed
        // before the native stack overflows on hostile macro bodies.
        self.depth += 1;
        let r = if self.depth > crate::parsing::c::preprocess::expr_parser::MAX_PP_EXPR_DEPTH {
            None
        } else {
            self.parse_conditional_expression_inner()
        };
        self.depth -= 1;
        r
    }

    fn parse_conditional_expression_inner(&mut self) -> Option<u32> {
        let condition = self.parse_logical_or_expression()?;
        self.skip_ws();
        if !self.consume_byte(b'?') {
            return Some(condition);
        }
        let if_true = self.parse_conditional_expression()?;
        self.skip_ws();
        if !self.consume_byte(b':') {
            return None;
        }
        let if_false = self.parse_conditional_expression()?;
        Some(if condition != 0 { if_true } else { if_false })
    }

    fn parse_logical_or_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_logical_and_expression()?;
        loop {
            self.skip_ws();
            if self.consume_pair(b'|', b'|') {
                let rhs = self.parse_logical_and_expression()?;
                value = u32::from(value != 0 || rhs != 0);
            } else {
                return Some(value);
            }
        }
    }

    fn parse_logical_and_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_bitwise_or_expression()?;
        loop {
            self.skip_ws();
            if self.consume_pair(b'&', b'&') {
                let rhs = self.parse_bitwise_or_expression()?;
                value = u32::from(value != 0 && rhs != 0);
            } else {
                return Some(value);
            }
        }
    }

    fn parse_bitwise_or_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_bitwise_xor_expression()?;
        loop {
            self.skip_ws();
            if self.consume_pair(b'|', b'|') {
                self.index = self.index.saturating_sub(2);
                return Some(value);
            }
            if self.consume_byte(b'|') {
                value |= self.parse_bitwise_xor_expression()?;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_bitwise_xor_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_bitwise_and_expression()?;
        loop {
            self.skip_ws();
            if self.consume_byte(b'^') {
                value ^= self.parse_bitwise_and_expression()?;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_bitwise_and_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_equality_expression()?;
        loop {
            self.skip_ws();
            if self.consume_pair(b'&', b'&') {
                self.index = self.index.saturating_sub(2);
                return Some(value);
            }
            if self.consume_byte(b'&') {
                value &= self.parse_equality_expression()?;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_equality_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_relational_expression()?;
        loop {
            self.skip_ws();
            if self.consume_pair(b'=', b'=') {
                value = u32::from(value == self.parse_relational_expression()?);
            } else if self.consume_pair(b'!', b'=') {
                value = u32::from(value != self.parse_relational_expression()?);
            } else {
                return Some(value);
            }
        }
    }

    fn parse_relational_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_shift_expression()?;
        loop {
            self.skip_ws();
            if self.consume_pair(b'<', b'=') {
                value = u32::from(value <= self.parse_shift_expression()?);
            } else if self.consume_pair(b'>', b'=') {
                value = u32::from(value >= self.parse_shift_expression()?);
            } else if self.consume_byte(b'<') {
                value = u32::from(value < self.parse_shift_expression()?);
            } else if self.consume_byte(b'>') {
                value = u32::from(value > self.parse_shift_expression()?);
            } else {
                return Some(value);
            }
        }
    }

    fn parse_shift_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_additive_expression()?;
        loop {
            self.skip_ws();
            if self.consume_pair(b'<', b'<') {
                let rhs = self.parse_additive_expression()?;
                value = value.checked_shl(rhs.min(31)).unwrap_or(0);
            } else if self.consume_pair(b'>', b'>') {
                let rhs = self.parse_additive_expression()?;
                value = value.checked_shr(rhs.min(31)).unwrap_or(0);
            } else {
                return Some(value);
            }
        }
    }

    fn parse_additive_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_multiplicative_expression()?;
        loop {
            self.skip_ws();
            if self.consume_byte(b'+') {
                value = value.wrapping_add(self.parse_multiplicative_expression()?);
            } else if self.consume_byte(b'-') {
                value = value.wrapping_sub(self.parse_multiplicative_expression()?);
            } else {
                return Some(value);
            }
        }
    }

    fn parse_multiplicative_expression(&mut self) -> Option<u32> {
        let mut value = self.parse_unary_expression()?;
        loop {
            self.skip_ws();
            if self.consume_byte(b'*') {
                value = value.wrapping_mul(self.parse_unary_expression()?);
            } else if self.consume_byte(b'/') {
                let rhs = self.parse_unary_expression()?;
                if rhs == 0 {
                    return None;
                }
                value /= rhs;
            } else if self.consume_byte(b'%') {
                let rhs = self.parse_unary_expression()?;
                if rhs == 0 {
                    return None;
                }
                value %= rhs;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_unary_expression(&mut self) -> Option<u32> {
        // `+ - ! ~` chains right-recurse here; guard on the shared counter so
        // `#define A ----...1` style bodies fail closed instead of crashing.
        self.depth += 1;
        let r = if self.depth > crate::parsing::c::preprocess::expr_parser::MAX_PP_EXPR_DEPTH {
            None
        } else {
            self.parse_unary_expression_inner()
        };
        self.depth -= 1;
        r
    }

    fn parse_unary_expression_inner(&mut self) -> Option<u32> {
        self.skip_ws();
        if self.consume_byte(b'+') {
            return self.parse_unary_expression();
        }
        if self.consume_byte(b'-') {
            return self
                .parse_unary_expression()
                .map(|value| 0u32.wrapping_sub(value));
        }
        if self.consume_byte(b'!') {
            return self
                .parse_unary_expression()
                .map(|value| u32::from(value == 0));
        }
        if self.consume_byte(b'~') {
            return self.parse_unary_expression().map(|value| !value);
        }
        self.parse_primary_expression()
    }

    fn parse_primary_expression(&mut self) -> Option<u32> {
        self.skip_ws();
        if self.consume_byte(b'+') {
            return self.parse_unary_expression();
        }
        if self.consume_byte(b'-') {
            return self
                .parse_unary_expression()
                .map(|value| 0u32.wrapping_sub(value));
        }
        if self.consume_byte(b'(') {
            let value = self.parse_conditional_expression()?;
            self.skip_ws();
            return self.consume_byte(b')').then_some(value);
        }
        self.consume_integer()
            .or_else(|| self.consume_identifier_value())
    }

    fn consume_integer(&mut self) -> Option<u32> {
        self.skip_ws();
        let start = self.index;
        let radix = if self.body.get(self.index..self.index + 2) == Some(b"0x")
            || self.body.get(self.index..self.index + 2) == Some(b"0X")
        {
            self.index += 2;
            16u32
        } else if self.body.get(self.index..self.index + 2) == Some(b"0b")
            || self.body.get(self.index..self.index + 2) == Some(b"0B")
        {
            self.index += 2;
            2u32
        } else if self.body.get(self.index).copied() == Some(b'0') {
            8u32
        } else {
            10u32
        };
        let digits_start = self.index;
        let mut value = 0u32;
        while let Some(byte) = self.body.get(self.index).copied() {
            let digit = match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'a'..=b'f' if radix == 16 => u32::from(byte - b'a' + 10),
                b'A'..=b'F' if radix == 16 => u32::from(byte - b'A' + 10),
                b'\'' => {
                    self.index += 1;
                    continue;
                }
                _ => break,
            };
            if digit >= radix {
                break;
            }
            value = value.saturating_mul(radix).saturating_add(digit);
            self.index += 1;
        }
        if self.index == digits_start {
            self.index = start;
            return None;
        }
        while matches!(self.body.get(self.index), Some(b'u' | b'U' | b'l' | b'L')) {
            self.index += 1;
        }
        Some(value)
    }

    fn consume_identifier_value(&mut self) -> Option<u32> {
        self.skip_ws();
        let start = self.index;
        if !self.body.get(start).copied().is_some_and(is_ident_start) {
            return None;
        }
        self.index += 1;
        self.index = scan_while(self.body, self.index, is_ident_continue);
        let ident = &self.body[start..self.index];
        Some(
            self.macro_indexes
                .get(ident)
                .and_then(|idx| self.values.get(*idx))
                .copied()
                .unwrap_or(0),
        )
    }

    fn skip_ws(&mut self) {
        loop {
            while matches!(
                self.body.get(self.index),
                Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c')
            ) {
                self.index += 1;
            }
            if self.body.get(self.index..self.index + 2) == Some(b"/*") {
                self.index += scan_block_comment(&self.body[self.index..]);
                continue;
            }
            if self.body.get(self.index..self.index + 2) == Some(b"//") {
                self.index = self.body.len();
                continue;
            }
            break;
        }
    }

    fn consume_pair(&mut self, first: u8, second: u8) -> bool {
        if self.body.get(self.index..self.index + 2) == Some(&[first, second]) {
            self.index += 2;
            true
        } else {
            false
        }
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.body.get(self.index).copied() == Some(byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

fn scan_while(body: &[u8], start: usize, predicate: impl Fn(u8) -> bool) -> usize {
    let mut index = start;
    while body.get(index).copied().is_some_and(&predicate) {
        index += 1;
    }
    index
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_macro(name: &[u8], body: &[u8]) -> MacroDef {
        MacroDef {
            name: name.to_vec(),
            args: Vec::new(),
            body: body.to_vec(),
            is_function_like: false,
        }
    }

    #[test]
    fn macro_integer_values_cover_preprocessor_operator_ladder() {
        let cases = [
            (b"3 * 7".as_slice(), 21),
            (b"22 / 5".as_slice(), 4),
            (b"22 % 5".as_slice(), 2),
            (b"1 << 5".as_slice(), 32),
            (b"8 > 3".as_slice(), 1),
            (b"8 <= 3".as_slice(), 0),
            (b"4 == 4".as_slice(), 1),
            (b"4 != 4".as_slice(), 0),
            (b"6 & 3".as_slice(), 2),
            (b"6 ^ 3".as_slice(), 5),
            (b"6 | 1".as_slice(), 7),
            (b"0 || 9".as_slice(), 1),
            (b"7 && 0".as_slice(), 0),
            (b"!0".as_slice(), 1),
            (b"~0u".as_slice(), u32::MAX),
            (b"0 ? 11 : 13".as_slice(), 13),
            (b"1 ? 11 : 13".as_slice(), 11),
        ];
        for (body, expected) in cases {
            let macros = [object_macro(b"VALUE", body)];
            assert_eq!(
                macro_integer_values(&macros).expect("Fix: reject preprocessor macros whose values do not fit u32; fail parse, do not truncate - macro integer values should fit"),
                vec![expected],
                "body `{}`",
                String::from_utf8_lossy(body)
            );
        }
    }

    #[test]
    fn macro_integer_values_resolve_object_like_identifier_dependencies() {
        let macros = [
            object_macro(b"A", b"1"),
            object_macro(b"B", b"A + 2"),
            object_macro(b"C", b"B == 3"),
            object_macro(b"D", b"MISSING"),
        ];
        assert_eq!(
            macro_integer_values(&macros).expect("Fix: reject preprocessor macros whose values do not fit u32; fail parse, do not truncate - macro integer values should fit"),
            vec![1, 3, 1, 0]
        );
    }

    #[test]
    fn macro_integer_values_resolve_linux_hz_alias_chain() {
        let macros = [
            object_macro(b"CONFIG_HZ", b"1000"),
            object_macro(b"HZ", b"CONFIG_HZ\t/* Internal kernel timer frequency */"),
        ];
        assert_eq!(
            macro_integer_values(&macros).expect("Fix: reject preprocessor macros whose values do not fit u32; fail parse, do not truncate - macro integer values should fit"),
            vec![1000, 1000]
        );
    }

    #[test]
    fn macro_integer_values_fail_closed_for_unstable_recursive_definitions() {
        let macros = [
            object_macro(b"STABLE", b"9"),
            object_macro(b"A", b"!A"),
            object_macro(b"DEPENDS_ON_STABLE", b"STABLE + 1"),
        ];
        assert_eq!(
            macro_integer_values(&macros).expect("Fix: reject preprocessor macros whose values do not fit u32; fail parse, do not truncate - macro integer values should fit"),
            vec![9, 0, 10]
        );
    }

    // --- Adversarial: deeply-nested object-like macro bodies must fail closed ---
    // The integer-macro evaluator is recursive descent (ternary in
    // parse_conditional_expression, `+ - ! ~` and parens reaching back through
    // parse_unary_expression / parse_primary_expression). Without a depth bound
    // a hostile `#define` body overflows the native stack and aborts the
    // process during macro-value resolution. The guard makes the parser return
    // `None`, which object_like_macro_value maps to 0 for a non-empty body, so
    // the value resolves to 0 without crashing. These tests are self-proving: a
    // regressed guard SIGABRTs the test binary rather than reporting a failure.

    #[test]
    fn macro_integer_values_fail_closed_on_deep_paren_nesting() {
        let n = 50_000;
        let mut body = Vec::new();
        body.extend(std::iter::repeat(b'(').take(n));
        body.push(b'1');
        body.extend(std::iter::repeat(b')').take(n));
        let macros = [object_macro(b"BOMB", &body)];
        assert_eq!(
            macro_integer_values(&macros)
                .expect("macro integer resolution must complete, not crash"),
            vec![0],
            "deep paren-nested macro body must fail closed to 0"
        );
    }

    #[test]
    fn macro_integer_values_fail_closed_on_deep_unary_nesting() {
        let mut body = vec![b'!'; 50_000];
        body.push(b'1');
        let macros = [object_macro(b"BOMB", &body)];
        assert_eq!(
            macro_integer_values(&macros)
                .expect("macro integer resolution must complete, not crash"),
            vec![0],
            "deep `!`-nested macro body must fail closed to 0"
        );
    }

    #[test]
    fn macro_integer_values_fail_closed_on_deep_ternary_nesting() {
        let n = 50_000;
        let mut body = Vec::new();
        for _ in 0..n {
            body.extend_from_slice(b"1?1:");
        }
        body.push(b'1');
        let macros = [object_macro(b"BOMB", &body)];
        assert_eq!(
            macro_integer_values(&macros)
                .expect("macro integer resolution must complete, not crash"),
            vec![0],
            "deep ternary-nested macro body must fail closed to 0"
        );
    }

    #[test]
    fn macro_integer_values_shallow_nesting_still_resolves() {
        // Guard must not over-reject: 16 parens around a value still resolves.
        let depth = 16;
        let mut body = Vec::new();
        body.extend(std::iter::repeat(b'(').take(depth));
        body.extend_from_slice(b"42");
        body.extend(std::iter::repeat(b')').take(depth));
        let macros = [object_macro(b"OK", &body)];
        assert_eq!(
            macro_integer_values(&macros).expect("macro integer resolution must complete"),
            vec![42],
            "a 16-deep parenthesized macro body is legal and must resolve"
        );
    }
}
