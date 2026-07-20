use super::MacroDef;
use rustc_hash::FxHashMap as HashMap;

pub(super) fn fast_kernel_config_if_truth(
    row_bytes: &[u8],
    macros: &[MacroDef],
) -> Result<Option<bool>, String> {
    let Some(expr) = strip_if_directive_keyword(row_bytes) else {
        return Ok(None);
    };
    let expr = trim_ascii(expr);
    if expr.is_empty() {
        return Ok(None);
    }
    let truth_table = MacroTruthTable::try_new(macros)?;
    Ok(fast_kernel_config_if_truth_inner(expr, &truth_table))
}

fn fast_kernel_config_if_truth_inner(
    expr: &[u8],
    truth_table: &MacroTruthTable<'_>,
) -> Option<bool> {
    let expr = trim_ascii(expr);
    if expr.is_empty() {
        return None;
    }
    if let Some(inner) = strip_balanced_parens(expr) {
        return fast_kernel_config_if_truth_inner(inner, truth_table);
    }
    match eval_top_level_operator(expr, truth_table, b"||", true) {
        TopLevelEval::Value(value) => return value,
        TopLevelEval::Absent => {}
    }
    match eval_top_level_operator(expr, truth_table, b"&&", false) {
        TopLevelEval::Value(value) => return value,
        TopLevelEval::Absent => {}
    }
    if let Some(inner) = expr.strip_prefix(b"!") {
        return fast_kernel_config_if_truth_inner(inner, truth_table).map(|value| !value);
    }
    if is_identifier(expr) {
        return Some(truth_table.macro_truth(expr));
    }
    if let Some(name) = parse_defined_operand(expr) {
        return Some(truth_table.is_defined(name));
    }
    for function in [
        b"IS_ENABLED".as_slice(),
        b"IS_BUILTIN".as_slice(),
        b"IS_REACHABLE".as_slice(),
        b"IS_DEFINED".as_slice(),
    ] {
        if let Some(name) = parse_single_macro_call(expr, function) {
            return Some(truth_table.macro_truth(name));
        }
    }
    if let Some(name) = parse_single_macro_call(expr, b"IS_MODULE") {
        return Some(truth_table.module_truth(name));
    }
    None
}

struct MacroTruthTable<'a> {
    bodies: HashMap<&'a [u8], &'a [u8]>,
    module_bodies: HashMap<&'a [u8], &'a [u8]>,
}

impl<'a> MacroTruthTable<'a> {
    fn try_new(macros: &'a [MacroDef]) -> Result<Self, String> {
        let mut bodies = HashMap::default();
        let module_count = macros
            .iter()
            .filter(|mac| mac.name.ends_with(b"_MODULE"))
            .count();
        let mut module_bodies = HashMap::default();
        bodies.try_reserve(macros.len()).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {} conditional macro truth entries: {error:?}. Fix: shard conditional evaluation before GPU preprocessing.",
                macros.len()
            )
        })?;
        module_bodies.try_reserve(module_count).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {module_count} conditional module truth entries: {error:?}. Fix: shard conditional evaluation before GPU preprocessing.",
            )
        })?;
        for mac in macros {
            bodies.insert(mac.name.as_slice(), mac.body.as_slice());
            if let Some(module_base) = mac.name.strip_suffix(b"_MODULE") {
                module_bodies.insert(module_base, mac.body.as_slice());
            }
        }
        Ok(Self {
            bodies,
            module_bodies,
        })
    }

    fn is_defined(&self, name: &[u8]) -> bool {
        self.bodies.contains_key(name)
    }

    fn macro_truth(&self, name: &[u8]) -> bool {
        self.bodies
            .get(name)
            .is_some_and(|body| macro_body_truth(body))
    }

    fn module_truth(&self, name: &[u8]) -> bool {
        self.module_bodies
            .get(name)
            .is_some_and(|body| macro_body_truth(body))
    }
}

enum TopLevelEval {
    Absent,
    Value(Option<bool>),
}

fn eval_top_level_operator(
    expr: &[u8],
    truth_table: &MacroTruthTable<'_>,
    operator: &[u8; 2],
    short_circuit_value: bool,
) -> TopLevelEval {
    let mut depth = 0_u32;
    let mut start = 0_usize;
    let mut cursor = 0_usize;
    let mut found = false;
    while cursor + 1 < expr.len() {
        match expr[cursor] {
            b'(' => depth = depth.saturating_add(1),
            b')' => depth = depth.saturating_sub(1),
            _ if depth == 0 && &expr[cursor..cursor + 2] == operator => {
                let part = trim_ascii(&expr[start..cursor]);
                if part.is_empty() {
                    return TopLevelEval::Value(None);
                }
                found = true;
                match fast_kernel_config_if_truth_inner(part, truth_table) {
                    Some(value) if value == short_circuit_value => {
                        return TopLevelEval::Value(Some(short_circuit_value));
                    }
                    Some(_) => {}
                    None => return TopLevelEval::Value(None),
                }
                cursor += 2;
                start = cursor;
                continue;
            }
            _ => {}
        }
        cursor += 1;
    }
    if !found {
        return TopLevelEval::Absent;
    }
    let tail = trim_ascii(&expr[start..]);
    if tail.is_empty() {
        return TopLevelEval::Value(None);
    }
    match fast_kernel_config_if_truth_inner(tail, truth_table) {
        Some(value) if value == short_circuit_value => {
            TopLevelEval::Value(Some(short_circuit_value))
        }
        Some(_) => TopLevelEval::Value(Some(!short_circuit_value)),
        None => TopLevelEval::Value(None),
    }
}

fn strip_balanced_parens(expr: &[u8]) -> Option<&[u8]> {
    let inner = expr.strip_prefix(b"(")?.strip_suffix(b")")?;
    let mut depth = 0_u32;
    for (index, byte) in expr.iter().enumerate() {
        match *byte {
            b'(' => depth = depth.saturating_add(1),
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && index != expr.len() - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }
    (depth == 0).then_some(trim_ascii(inner))
}

fn strip_if_directive_keyword(row: &[u8]) -> Option<&[u8]> {
    let row = trim_ascii(row);
    let row = row.strip_prefix(b"#")?;
    let row = trim_ascii(row);
    if let Some(rest) = row.strip_prefix(b"if") {
        if rest.first().is_some_and(|byte| is_ident_continue(*byte)) {
            return None;
        }
        return Some(rest);
    }
    if let Some(rest) = row.strip_prefix(b"elif") {
        if rest.first().is_some_and(|byte| is_ident_continue(*byte)) {
            return None;
        }
        return Some(rest);
    }
    None
}

fn parse_defined_operand(expr: &[u8]) -> Option<&[u8]> {
    let rest = trim_ascii(expr.strip_prefix(b"defined")?);
    if let Some(paren) = rest.strip_prefix(b"(") {
        let name = trim_ascii(paren.strip_suffix(b")")?);
        return is_identifier(name).then_some(name);
    }
    is_identifier(rest).then_some(rest)
}

fn parse_single_macro_call<'a>(expr: &'a [u8], function: &[u8]) -> Option<&'a [u8]> {
    let rest = expr.strip_prefix(function)?;
    let rest = trim_ascii(rest);
    let inner = rest.strip_prefix(b"(")?.strip_suffix(b")")?;
    let name = trim_ascii(inner);
    is_identifier(name).then_some(name)
}

fn macro_body_truth(body: &[u8]) -> bool {
    let body = trim_ascii(body);
    if body.is_empty() {
        return true;
    }
    body != b"0"
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0_usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}

fn is_identifier(bytes: &[u8]) -> bool {
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    is_ident_start(first) && rest.iter().all(|byte| is_ident_continue(*byte))
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
    fn linux_config_boolean_fast_path_handles_top_level_operators() {
        let macros = vec![
            object_macro(b"CONFIG_ON", b"1"),
            object_macro(b"CONFIG_ZERO", b"0"),
            object_macro(b"CONFIG_MOD_MODULE", b"1"),
        ];
        assert_eq!(
            fast_kernel_config_if_truth(
                b"#if IS_ENABLED(CONFIG_ON) && !defined(CONFIG_OFF)",
                &macros
            )
            .expect("Fix: conditional_eval truth tables must fit fixed storage; reject oversized macro expansions - truth table should fit"),
            Some(true)
        );
        assert_eq!(
            fast_kernel_config_if_truth(
                b"#if IS_ENABLED(CONFIG_ZERO) || IS_MODULE(CONFIG_MOD)",
                &macros
            )
            .expect("Fix: conditional_eval truth tables must fit fixed storage; reject oversized macro expansions - truth table should fit"),
            Some(true)
        );
        assert_eq!(
            fast_kernel_config_if_truth(b"#elif (CONFIG_ON) && (defined(CONFIG_ZERO))", &macros)
                .expect("Fix: conditional_eval truth tables must fit fixed storage; reject oversized macro expansions - truth table should fit"),
            Some(true)
        );
    }

    #[test]
    fn generated_module_truth_uses_preindexed_suffix_aliases_for_8192_macros() {
        let macros: Vec<MacroDef> = (0..8192)
            .map(|index| {
                let name = format!("CONFIG_GENERATED_{index}_MODULE");
                let body = if index % 7 == 0 { b"0" } else { b"1" };
                object_macro(name.as_bytes(), body)
            })
            .collect();
        let table = MacroTruthTable::try_new(&macros)
            .expect("Fix: generated module truth table should reserve.");

        for index in 0..8192 {
            let name = format!("CONFIG_GENERATED_{index}");
            assert_eq!(table.module_truth(name.as_bytes()), index % 7 != 0);
        }
        assert!(
            !table.module_truth(b"CONFIG_GENERATED_MISSING"),
            "Fix: missing module aliases must fail closed."
        );
    }
}
