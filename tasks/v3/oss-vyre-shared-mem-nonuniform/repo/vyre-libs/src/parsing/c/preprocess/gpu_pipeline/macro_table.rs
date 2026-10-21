use super::macro_values::macro_integer_values_with_builtin_prefix;
use crate::parsing::c::lex::tokens::*;
use crate::parsing::c::preprocess::expansion::{
    C_MACRO_KIND_FUNCTION_LIKE, C_MACRO_KIND_OBJECT_LIKE, C_MACRO_REPLACEMENT_LITERAL,
    EMPTY_MACRO_SLOT, MACRO_TABLE_MASK, MACRO_TABLE_SLOTS,
};
use crate::parsing::c::preprocess::gpu_pipeline::buffers::{checked_gpu_u32, pack_u32_words};
use crate::parsing::c::preprocess::gpu_pipeline::MacroDef;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use vyre_primitives::hash::fnv1a::fnv1a32 as primitive_fnv1a32;

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct PackedMacroTable {
    pub(super) names_padded: Vec<u8>,
    pub(super) offsets_le: Vec<u8>,
    pub(super) values_le: Vec<u8>,
    pub(super) expansion_name_hashes_le: Vec<u8>,
    pub(super) expansion_name_starts_le: Vec<u8>,
    pub(super) expansion_name_lens_le: Vec<u8>,
    pub(super) expansion_name_bytes: Vec<u8>,
    pub(super) expansion_vals_le: Vec<u8>,
    pub(super) expansion_sizes_le: Vec<u8>,
    pub(super) expansion_kinds_le: Vec<u8>,
    pub(super) expansion_param_counts_le: Vec<u8>,
    pub(super) expansion_replacement_params_le: Vec<u8>,
    pub(super) expansion_replacement_starts_le: Vec<u8>,
    pub(super) expansion_replacement_lens_le: Vec<u8>,
    pub(super) expansion_replacement_bytes: Vec<u8>,
    pub(super) expansion_replacement_source_len: u32,
    pub(super) expansion_max_replacement_tokens: u32,
    pub(super) names_len: u32,
    pub(super) count: u32,
}

impl PackedMacroTable {
    pub(crate) fn from_definitions(macros: &[MacroDef]) -> Result<Self, String> {
        let mut seen_names = HashSet::default();
        seen_names.try_reserve(macros.len()).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {} macro table dedupe slots: {error:?}. Fix: shard macro table packing before GPU preprocessing.",
                macros.len()
            )
        })?;
        for mac in macros {
            if mac.name.is_empty() {
                return Err(
                    "vyre-libs::gpu_pipeline: empty live macro name reached GPU macro table packing. Fix: reject malformed #define rows before packing."
                        .to_string(),
                );
            }
            if !seen_names.insert(mac.name.as_slice()) {
                let name = String::from_utf8_lossy(&mac.name);
                return Err(format!(
                    "vyre-libs::gpu_pipeline: duplicate live macro `{name}` reached GPU macro table packing. Fix: replace existing definitions during preprocessing instead of appending duplicates."
                ));
            }
        }
        let mut names = Vec::new();
        let offset_slots = macros.len().checked_add(1).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: macro table offset slot count overflows usize. Fix: shard macro table packing before GPU preprocessing.".to_string()
        })?;
        let mut offsets = Vec::new();
        reserve_macro_table_vec(&mut offsets, offset_slots, "macro table offsets")?;
        let total_name_bytes = macros.iter().try_fold(0usize, |total, mac| {
            total.checked_add(mac.name.len()).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro table name bytes overflow usize. Fix: shard macro table packing before GPU preprocessing.".to_string()
            })
        })?;
        reserve_macro_table_vec(&mut names, total_name_bytes, "macro table name bytes")?;
        let values = macro_integer_values_with_builtin_prefix(macros)?;
        offsets.push(0);
        for mac in macros {
            names.extend_from_slice(&mac.name);
            offsets.push(checked_gpu_u32(
                "live conditional macro-name table byte length",
                names.len(),
            )?);
        }
        let names_len =
            checked_gpu_u32("live conditional macro-name table byte length", names.len())?;
        let count = checked_gpu_u32("live conditional macro definition count", macros.len())?;
        let names_target = padded_macro_table_byte_len(names.len(), "macro table names")?;
        let mut names_padded = Vec::new();
        reserve_macro_table_vec(&mut names_padded, names_target, "macro table padded names")?;
        names_padded.resize(names_target, 0);
        names_padded[..names.len()].copy_from_slice(&names);
        let offsets_le = pack_u32_words(&offsets, offsets.len())?;
        let values_le = pack_u32_words(&values, values.len().max(1))?;
        let expansion = PackedExpansionMacroTable::from_definitions(macros)?;
        Ok(Self {
            names_padded,
            offsets_le,
            values_le,
            expansion_name_hashes_le: pack_u32_words(
                &expansion.name_hashes,
                MACRO_TABLE_SLOTS as usize,
            )?,
            expansion_name_starts_le: pack_u32_words(
                &expansion.name_starts,
                MACRO_TABLE_SLOTS as usize,
            )?,
            expansion_name_lens_le: pack_u32_words(
                &expansion.name_lens,
                MACRO_TABLE_SLOTS as usize,
            )?,
            expansion_name_bytes: expansion.name_bytes,
            expansion_vals_le: pack_u32_words(&expansion.vals, MACRO_TABLE_SLOTS as usize)?,
            expansion_sizes_le: pack_u32_words(&expansion.sizes, MACRO_TABLE_SLOTS as usize)?,
            expansion_kinds_le: pack_u32_words(&expansion.kinds, MACRO_TABLE_SLOTS as usize)?,
            expansion_param_counts_le: pack_u32_words(
                &expansion.param_counts,
                MACRO_TABLE_SLOTS as usize,
            )?,
            expansion_replacement_params_le: pack_u32_words(
                &expansion.replacement_params,
                MACRO_TABLE_SLOTS as usize,
            )?,
            expansion_replacement_starts_le: pack_u32_words(
                &expansion.replacement_starts,
                MACRO_TABLE_SLOTS as usize,
            )?,
            expansion_replacement_lens_le: pack_u32_words(
                &expansion.replacement_lens,
                MACRO_TABLE_SLOTS as usize,
            )?,
            expansion_replacement_bytes: expansion.replacement_bytes,
            expansion_replacement_source_len: expansion.replacement_source_len,
            expansion_max_replacement_tokens: expansion.max_replacement_tokens,
            names_len,
            count,
        })
    }

    pub(crate) fn byte_len(&self) -> usize {
        [
            self.names_padded.len(),
            self.offsets_le.len(),
            self.values_le.len(),
            self.expansion_name_hashes_le.len(),
            self.expansion_name_starts_le.len(),
            self.expansion_name_lens_le.len(),
            self.expansion_name_bytes.len(),
            self.expansion_vals_le.len(),
            self.expansion_sizes_le.len(),
            self.expansion_kinds_le.len(),
            self.expansion_param_counts_le.len(),
            self.expansion_replacement_params_le.len(),
            self.expansion_replacement_starts_le.len(),
            self.expansion_replacement_lens_le.len(),
            self.expansion_replacement_bytes.len(),
        ]
        .into_iter()
        .try_fold(0usize, |acc, len| acc.checked_add(len))
        .unwrap_or(usize::MAX)
    }
}

fn reserve_macro_table_vec<T>(
    out: &mut Vec<T>,
    additional: usize,
    label: &'static str,
) -> Result<(), String> {
    out.try_reserve_exact(additional).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {additional} {label}: {error:?}. Fix: shard macro table packing before GPU preprocessing."
        )
    })
}

fn filled_macro_table_vec<T: Clone>(
    len: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    reserve_macro_table_vec(&mut out, len, label)?;
    out.resize(len, value);
    Ok(out)
}

fn padded_macro_table_byte_len(byte_len: usize, label: &'static str) -> Result<usize, String> {
    byte_len
        .checked_add(3)
        .and_then(|value| value.checked_div(4))
        .and_then(|words| words.checked_mul(4))
        .map(|bytes| bytes.max(4))
        .ok_or_else(|| {
            format!(
                "vyre-libs::gpu_pipeline: {label} byte length {byte_len} overflows u32 padding. Fix: shard macro table packing before GPU preprocessing."
            )
        })
}

struct PackedExpansionMacroTable {
    name_hashes: Vec<u32>,
    name_starts: Vec<u32>,
    name_lens: Vec<u32>,
    name_bytes: Vec<u8>,
    vals: Vec<u32>,
    sizes: Vec<u32>,
    kinds: Vec<u32>,
    param_counts: Vec<u32>,
    replacement_params: Vec<u32>,
    replacement_starts: Vec<u32>,
    replacement_lens: Vec<u32>,
    replacement_bytes: Vec<u8>,
    replacement_source_len: u32,
    max_replacement_tokens: u32,
}

impl PackedExpansionMacroTable {
    fn from_definitions(macros: &[MacroDef]) -> Result<Self, String> {
        let slots = MACRO_TABLE_SLOTS as usize;
        let name_byte_slots = macros
            .iter()
            .try_fold(0usize, |total, mac| {
                total.checked_add(mac.name.len()).ok_or_else(|| {
                    "vyre-libs::gpu_pipeline: macro expansion name table length overflowed usize. Fix: shard macro table packing before GPU preprocessing.".to_string()
                })
            })?
            .max(1);
        let replacement_byte_slots = macros
            .iter()
            .try_fold(0usize, |total, mac| {
                total.checked_add(mac.body.len()).ok_or_else(|| {
                    "vyre-libs::gpu_pipeline: macro expansion replacement table length overflowed usize. Fix: shard macro table packing before GPU preprocessing.".to_string()
                })
            })?
            .max(1);
        let mut table = Self {
            name_hashes: filled_macro_table_vec(
                slots,
                EMPTY_MACRO_SLOT,
                "macro expansion name hash slots",
            )?,
            name_starts: filled_macro_table_vec(slots, 0, "macro expansion name starts")?,
            name_lens: filled_macro_table_vec(slots, 0, "macro expansion name lengths")?,
            name_bytes: filled_macro_table_vec(name_byte_slots, 0, "macro expansion name bytes")?,
            vals: filled_macro_table_vec(slots, EMPTY_MACRO_SLOT, "macro expansion value slots")?,
            sizes: filled_macro_table_vec(slots, 0, "macro expansion sizes")?,
            kinds: filled_macro_table_vec(
                slots,
                C_MACRO_KIND_OBJECT_LIKE,
                "macro expansion kinds",
            )?,
            param_counts: filled_macro_table_vec(slots, 0, "macro expansion parameter counts")?,
            replacement_params: filled_macro_table_vec(
                slots,
                C_MACRO_REPLACEMENT_LITERAL,
                "macro expansion replacement parameters",
            )?,
            replacement_starts: filled_macro_table_vec(
                slots,
                0,
                "macro expansion replacement starts",
            )?,
            replacement_lens: filled_macro_table_vec(slots, 0, "macro expansion replacement lens")?,
            replacement_bytes: {
                let mut bytes = Vec::new();
                reserve_macro_table_vec(
                    &mut bytes,
                    replacement_byte_slots,
                    "macro expansion replacement bytes",
                )?;
                bytes
            },
            replacement_source_len: 0,
            max_replacement_tokens: 0,
        };
        let mut occupied_slots =
            filled_macro_table_vec(slots, false, "macro expansion occupancy slots")?;
        let mut pending = Vec::new();
        reserve_macro_table_vec(
            &mut pending,
            macros.len(),
            "pending macro expansion definitions",
        )?;
        let mut name_cursor = 0usize;
        for mac in macros {
            if mac.name.is_empty() {
                continue;
            }
            let hash = fnv1a32(&mac.name)?;
            let slot = table.insert_name_slot(hash)?;
            occupied_slots[slot] = true;
            let name_len = mac.name.len();
            let name_end = name_cursor.checked_add(name_len).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-name table length overflowed usize".to_string()
            })?;
            table.name_bytes[name_cursor..name_end].copy_from_slice(&mac.name);
            table.name_starts[slot] = checked_gpu_u32("macro expansion name start", name_cursor)?;
            table.name_lens[slot] = checked_gpu_u32("macro expansion name length", name_len)?;
            table.name_hashes[slot] = hash;
            table.kinds[slot] = if mac.is_function_like {
                C_MACRO_KIND_FUNCTION_LIKE
            } else {
                C_MACRO_KIND_OBJECT_LIKE
            };
            table.param_counts[slot] = macro_param_count(&mac.args)?;
            pending.push((slot, mac));
            name_cursor = name_end;
        }

        let mut replacement_cursor = 0usize;
        for (slot, mac) in pending {
            let params = macro_params(&mac.args)?;
            let tokens = replacement_tokens(&mac.body, &params)?;
            let span_len = tokens.len().max(1);
            let repl_index = next_replacement_span(replacement_cursor, span_len, &occupied_slots, slots)
                .ok_or_else(|| {
                    "vyre-libs::gpu_pipeline: macro expansion table exhausted while assigning replacement slots. Fix: shard macro expansion or raise MACRO_TABLE_SLOTS."
                        .to_string()
                })?;
            for idx in repl_index..repl_index + span_len {
                occupied_slots[idx] = true;
            }
            replacement_cursor = repl_index + span_len;
            table.vals[slot] = checked_gpu_u32("macro replacement slot", repl_index)?;
            table.sizes[repl_index] =
                checked_gpu_u32("macro replacement token count", tokens.len())?;
            table.max_replacement_tokens = table.max_replacement_tokens.max(checked_gpu_u32(
                "macro replacement token count",
                tokens.len(),
            )?);
            for (token_idx, token) in tokens.iter().enumerate() {
                let row = repl_index + token_idx;
                table.vals[row] = token.kind;
                table.replacement_params[row] = token.param.unwrap_or(C_MACRO_REPLACEMENT_LITERAL);
                table.replacement_starts[row] = table.replacement_source_len;
                table.replacement_lens[row] =
                    checked_gpu_u32("macro replacement token length", token.len)?;
                table
                    .replacement_bytes
                    .extend_from_slice(&mac.body[token.start..token.start + token.len]);
                table.replacement_source_len = table
                    .replacement_source_len
                    .checked_add(checked_gpu_u32(
                        "macro replacement token length",
                        token.len,
                    )?)
                    .ok_or_else(|| {
                        "vyre-libs::gpu_pipeline: macro replacement source length overflowed u32"
                            .to_string()
                    })?;
            }
        }
        if table.replacement_bytes.is_empty() {
            table.replacement_bytes.push(0);
        }
        Ok(table)
    }

    fn insert_name_slot(&self, hash: u32) -> Result<usize, String> {
        let mut slot = ((hash.wrapping_mul(2_654_435_769)) & MACRO_TABLE_MASK) as usize;
        for _ in 0..MACRO_TABLE_SLOTS {
            if self.name_hashes[slot] == EMPTY_MACRO_SLOT {
                return Ok(slot);
            }
            slot = (slot + 1) & MACRO_TABLE_MASK as usize;
        }
        Err(
            "vyre-libs::gpu_pipeline: macro expansion name hash table is full. Fix: shard macro expansion or raise MACRO_TABLE_SLOTS."
                .to_string(),
        )
    }
}

fn next_replacement_span(
    start: usize,
    len: usize,
    occupied_slots: &[bool],
    slots: usize,
) -> Option<usize> {
    if len > slots {
        return None;
    }
    let mut idx = start;
    while idx < slots {
        while idx < slots && occupied_slots[idx] {
            idx += 1;
        }
        let span_start = idx;
        let mut run_len = 0usize;
        while idx < slots && !occupied_slots[idx] && run_len < len {
            idx += 1;
            run_len += 1;
        }
        if run_len == len {
            return Some(span_start);
        }
    }
    None
}

fn fnv1a32(bytes: &[u8]) -> Result<u32, String> {
    let hash = primitive_fnv1a32(bytes);
    if hash == EMPTY_MACRO_SLOT {
        return Err(
            "vyre-libs::gpu_pipeline: macro name hashed to the empty-slot sentinel. Fix: add sentinel remapping to both host packing and GPU lookup."
                .to_string(),
        );
    }
    Ok(hash)
}

fn macro_param_count(args: &[u8]) -> Result<u32, String> {
    if args.is_empty() {
        return Ok(0);
    }
    let count = args
        .split(|byte| *byte == b',')
        .filter(|param| !param.iter().all(|byte| byte.is_ascii_whitespace()))
        .count();
    let count = checked_gpu_u32("macro parameter count", count)?;
    let is_variadic = args
        .split(|byte| *byte == b',')
        .map(trim_ascii)
        .any(|param| param == b"..." || param.ends_with(b"..."));
    Ok(count | if is_variadic { 0x8000_0000 } else { 0 })
}

struct ReplacementToken {
    kind: u32,
    start: usize,
    len: usize,
    param: Option<u32>,
}

fn macro_params(args: &[u8]) -> Result<Vec<&[u8]>, String> {
    if args.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(Vec::new());
    }
    let mut params = Vec::new();
    for raw in args.split(|byte| *byte == b',') {
        let param = trim_ascii(raw);
        if param.is_empty() {
            continue;
        }
        let normalized = if param == b"..." {
            b"__VA_ARGS__".as_slice()
        } else if let Some(name) = param.strip_suffix(b"...") {
            trim_ascii(name)
        } else {
            param
        };
        if !is_identifier(normalized) {
            let param = String::from_utf8_lossy(param);
            return Err(format!(
                "vyre-libs::gpu_pipeline: invalid function-like macro parameter `{param}` reached expansion packing"
            ));
        }
        params.push(normalized);
    }
    Ok(params)
}

fn replacement_tokens(body: &[u8], params: &[&[u8]]) -> Result<Vec<ReplacementToken>, String> {
    let param_indexes = parameter_indexes(params)?;
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < body.len() {
        while body.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if i >= body.len() {
            break;
        }
        let start = i;
        let (kind, len) = classify_replacement_token(body, i)?;
        let slice = &body[start..start + len];
        let param = if kind == TOK_IDENTIFIER {
            parameter_index(slice, &param_indexes)
        } else {
            None
        };
        tokens.push(ReplacementToken {
            kind,
            start,
            len,
            param,
        });
        i = start + len;
    }
    Ok(tokens)
}

fn classify_replacement_token(body: &[u8], start: usize) -> Result<(u32, usize), String> {
    let byte = body[start];
    if is_ident_start(byte) {
        let end = scan_while(body, start + 1, is_ident_continue);
        return Ok((TOK_IDENTIFIER, end - start));
    }
    if byte.is_ascii_digit()
        || (byte == b'.' && body.get(start + 1).is_some_and(u8::is_ascii_digit))
    {
        let end = scan_number(body, start);
        let token = &body[start..end];
        let kind = if token
            .iter()
            .any(|b| matches!(*b, b'.' | b'e' | b'E' | b'p' | b'P'))
        {
            TOK_FLOAT
        } else {
            TOK_INTEGER
        };
        return Ok((kind, end - start));
    }
    if byte == b'"' || byte == b'\'' {
        let end = scan_quoted(body, start, byte)?;
        return Ok((
            if byte == b'"' { TOK_STRING } else { TOK_CHAR },
            end - start,
        ));
    }
    if let Some((kind, len)) = replacement_operator(body, start) {
        return Ok((kind, len));
    }
    let body = String::from_utf8_lossy(body);
    Err(format!(
        "vyre-libs::gpu_pipeline: macro replacement body `{body}` contains a token the GPU expansion packer cannot classify at byte {start}. Fix: extend replacement token classification instead of routing this macro through a CPU expander."
    ))
}

fn replacement_operator(body: &[u8], start: usize) -> Option<(u32, usize)> {
    for (token, bytes) in [
        (TOK_LSHIFT_EQ, b"<<=".as_slice()),
        (TOK_RSHIFT_EQ, b">>=".as_slice()),
        (TOK_ELLIPSIS, b"...".as_slice()),
        (TOK_ARROW, b"->".as_slice()),
        (TOK_INC, b"++".as_slice()),
        (TOK_DEC, b"--".as_slice()),
        (TOK_PLUS_EQ, b"+=".as_slice()),
        (TOK_MINUS_EQ, b"-=".as_slice()),
        (TOK_STAR_EQ, b"*=".as_slice()),
        (TOK_SLASH_EQ, b"/=".as_slice()),
        (TOK_PERCENT_EQ, b"%=".as_slice()),
        (TOK_AMP_EQ, b"&=".as_slice()),
        (TOK_PIPE_EQ, b"|=".as_slice()),
        (TOK_CARET_EQ, b"^=".as_slice()),
        (TOK_HASHHASH, b"##".as_slice()),
        (TOK_EQ, b"==".as_slice()),
        (TOK_NE, b"!=".as_slice()),
        (TOK_LE, b"<=".as_slice()),
        (TOK_GE, b">=".as_slice()),
        (TOK_AND, b"&&".as_slice()),
        (TOK_OR, b"||".as_slice()),
        (TOK_LSHIFT, b"<<".as_slice()),
        (TOK_RSHIFT, b">>".as_slice()),
    ] {
        if body.get(start..start + bytes.len()) == Some(bytes) {
            return Some((token, bytes.len()));
        }
    }
    Some(match body[start] {
        b'(' => (TOK_LPAREN, 1),
        b')' => (TOK_RPAREN, 1),
        b'{' => (TOK_LBRACE, 1),
        b'}' => (TOK_RBRACE, 1),
        b'[' => (TOK_LBRACKET, 1),
        b']' => (TOK_RBRACKET, 1),
        b';' => (TOK_SEMICOLON, 1),
        b',' => (TOK_COMMA, 1),
        b'.' => (TOK_DOT, 1),
        b'+' => (TOK_PLUS, 1),
        b'-' => (TOK_MINUS, 1),
        b'*' => (TOK_STAR, 1),
        b'/' => (TOK_SLASH, 1),
        b'%' => (TOK_PERCENT, 1),
        b'&' => (TOK_AMP, 1),
        b'|' => (TOK_PIPE, 1),
        b'^' => (TOK_CARET, 1),
        b'~' => (TOK_TILDE, 1),
        b'!' => (TOK_BANG, 1),
        b'=' => (TOK_ASSIGN, 1),
        b'<' => (TOK_LT, 1),
        b'>' => (TOK_GT, 1),
        b'#' => (TOK_HASH, 1),
        b'?' => (TOK_QUESTION, 1),
        b':' => (TOK_COLON, 1),
        _ => return None,
    })
}

fn parameter_indexes<'a>(params: &'a [&'a [u8]]) -> Result<HashMap<&'a [u8], u32>, String> {
    let mut indexes = HashMap::default();
    indexes.try_reserve(params.len()).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {} macro parameter index slots: {error:?}. Fix: reject or shard oversized function-like macros before GPU preprocessing.",
            params.len()
        )
    })?;
    for (idx, param) in params.iter().enumerate() {
        indexes.insert(
            *param,
            checked_gpu_u32("macro replacement parameter index", idx)?,
        );
    }
    Ok(indexes)
}

fn parameter_index(token: &[u8], params: &HashMap<&[u8], u32>) -> Option<u32> {
    params.get(token).copied()
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|idx| idx + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

fn is_identifier(bytes: &[u8]) -> bool {
    bytes.first().is_some_and(|byte| is_ident_start(*byte))
        && bytes.iter().skip(1).all(|byte| is_ident_continue(*byte))
}

fn scan_while(body: &[u8], mut index: usize, predicate: impl Fn(u8) -> bool) -> usize {
    while body.get(index).is_some_and(|byte| predicate(*byte)) {
        index += 1;
    }
    index
}

fn scan_number(body: &[u8], start: usize) -> usize {
    let mut index = start;
    while let Some(byte) = body.get(index).copied() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'\'' | b'.' | b'+' | b'-') {
            index += 1;
        } else {
            break;
        }
    }
    index
}

fn scan_quoted(body: &[u8], start: usize, quote: u8) -> Result<usize, String> {
    let mut index = start + 1;
    while let Some(byte) = body.get(index).copied() {
        index += 1;
        if byte == b'\\' {
            index += usize::from(index < body.len());
            continue;
        }
        if byte == quote {
            return Ok(index);
        }
    }
    let body = String::from_utf8_lossy(body);
    Err(format!(
        "vyre-libs::gpu_pipeline: unterminated quoted token in macro replacement body `{body}`"
    ))
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
    fn duplicate_live_macro_names_are_rejected_before_gpu_packing() {
        let err = PackedMacroTable::from_definitions(&[
            object_macro(b"FEATURE", b"1"),
            object_macro(b"FEATURE", b"0"),
        ])
        .expect_err("duplicate live macros must not reach GPU table packing");
        assert!(
            err.contains("duplicate live macro `FEATURE`"),
            "unexpected diagnostic: {err}"
        );
    }

    #[test]
    fn empty_live_macro_name_is_rejected_before_gpu_packing() {
        let err = PackedMacroTable::from_definitions(&[object_macro(b"", b"1")])
            .expect_err("empty live macro names must not reach GPU table packing");
        assert!(
            err.contains("empty live macro name"),
            "unexpected diagnostic: {err}"
        );
    }

    #[test]
    fn macro_expansion_table_preserves_distinct_names_with_same_u32_hash() {
        let left = b"ynO";
        let right = b"Wgca";
        let hash = fnv1a32(left).unwrap();
        assert_eq!(hash, fnv1a32(right).unwrap());
        let table = PackedExpansionMacroTable::from_definitions(&[
            object_macro(left, b"1"),
            object_macro(right, b"2"),
        ])
        .expect("Fix: u32 macro hash collisions must remain representable because GPU lookup compares candidate bytes after hash match");
        let slots = table
            .name_hashes
            .iter()
            .enumerate()
            .filter_map(|(slot, candidate)| (*candidate == hash).then_some(slot))
            .collect::<Vec<_>>();
        assert_eq!(slots.len(), 2);
        let mut names = slots
            .iter()
            .map(|slot| {
                let start = table.name_starts[*slot] as usize;
                let len = table.name_lens[*slot] as usize;
                table.name_bytes[start..start + len].to_vec()
            })
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec![right.to_vec(), left.to_vec()]);
    }

    #[test]
    fn macro_expansion_name_pool_grows_past_legacy_16k_cap() {
        let long_name = vec![b'A'; 16_384 + 257];
        let table = PackedMacroTable::from_definitions(&[object_macro(&long_name, b"1")])
            .expect("Fix: macro-name pool must size to the live translation-unit macros");

        assert!(
            table.expansion_name_bytes.len() > 16_384,
            "raw macro-name bytes must grow past the legacy fixed 16 KiB pool"
        );
    }
}
