use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExprTok {
    Num(i128),
    Not,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    BitNot,
    Star,
    Slash,
    Percent,
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    Question,
    Colon,
    LParen,
    RParen,
}

pub(super) const MAX_PREPROC_EXPR_MACRO_DEPTH: u32 = 32;

pub(super) fn tokenize_preproc_expr(
    expr: &str,
    macros: &HashMap<String, MacroDef>,
) -> Vec<ExprTok> {
    tokenize_preproc_expr_inner(expr, macros, 0, &[])
}

pub(super) fn tokenize_preproc_expr_inner(
    expr: &str,
    macros: &HashMap<String, MacroDef>,
    depth: u32,
    disabled: &[String],
) -> Vec<ExprTok> {
    if depth > MAX_PREPROC_EXPR_MACRO_DEPTH {
        return vec![ExprTok::Num(0)];
    }
    let bytes = expr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
        } else if bytes[i] == b'\'' {
            let (value, end) = parse_preproc_char_literal(expr, i);
            out.push(ExprTok::Num(value));
            i = end;
        } else if bytes[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push(ExprTok::Num(parse_preproc_integer_literal(&expr[start..i])));
        } else if is_ident_start(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let ident = &expr[start..i];
            if ident == "defined" {
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                let paren = bytes.get(i).copied() == Some(b'(');
                if paren {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                let name_start = i;
                if !bytes.get(i).is_some_and(|b| is_ident_start(*b)) {
                    panic!("malformed preprocessor defined operator: expected identifier");
                }
                i += 1;
                while i < bytes.len() && is_ident_continue(bytes[i]) {
                    i += 1;
                }
                let name = &expr[name_start..i];
                if paren {
                    while i < bytes.len() && bytes[i] != b')' {
                        i += 1;
                    }
                    if i >= bytes.len() {
                        panic!("malformed preprocessor defined operator: missing `)`");
                    }
                    i += 1;
                }
                out.push(ExprTok::Num(i128::from(macros.contains_key(name))));
            } else if is_preprocessor_probe_builtin(ident) {
                let mut call_start = i;
                while call_start < bytes.len() && bytes[call_start].is_ascii_whitespace() {
                    call_start += 1;
                }
                let Some((args, end)) = parse_expr_macro_args(expr, call_start) else {
                    out.push(ExprTok::Num(0));
                    continue;
                };
                i = end;
                let value = if ident == "__is_identifier" {
                    if args.len() != 1 || !is_plain_identifier(args[0].as_bytes()) {
                        out.push(ExprTok::Num(0));
                        continue;
                    }
                    i128::from(
                        !vyre_libs::parsing::c::preprocess::is_reserved_preprocessor_identifier(
                            args[0].as_bytes(),
                        ),
                    )
                } else {
                    0
                };
                out.push(ExprTok::Num(value));
            } else {
                let Some(def) = macros.get(ident) else {
                    out.push(ExprTok::Num(0));
                    continue;
                };
                if disabled.iter().any(|name| name == ident) {
                    out.push(ExprTok::Num(0));
                    continue;
                }
                let replacement = if let Some(params) = &def.params {
                    let mut call_start = i;
                    while call_start < bytes.len() && bytes[call_start].is_ascii_whitespace() {
                        call_start += 1;
                    }
                    let Some((mut args, end)) = parse_expr_macro_args(expr, call_start) else {
                        out.push(ExprTok::Num(0));
                        continue;
                    };
                    if params.is_empty() && args.len() == 1 && args[0].is_empty() {
                        args.clear();
                    }
                    i = end;
                    substitute_expr_macro_params(
                        ident,
                        &def.replacement,
                        params,
                        def.variadic.as_deref(),
                        &args,
                    )
                } else {
                    def.replacement.clone()
                };
                let replacement = replacement.trim();
                if replacement.is_empty() {
                    out.push(ExprTok::Num(0));
                    continue;
                }
                let mut next_disabled = disabled.to_vec();
                next_disabled.push(ident.to_string());
                out.extend(tokenize_preproc_expr_inner(
                    replacement,
                    macros,
                    depth + 1,
                    &next_disabled,
                ));
            }
        } else {
            match bytes[i] {
                b'!' if bytes.get(i + 1).copied() == Some(b'=') => {
                    out.push(ExprTok::Ne);
                    i += 2;
                }
                b'=' if bytes.get(i + 1).copied() == Some(b'=') => {
                    out.push(ExprTok::Eq);
                    i += 2;
                }
                b'<' if bytes.get(i + 1).copied() == Some(b'=') => {
                    out.push(ExprTok::Le);
                    i += 2;
                }
                b'>' if bytes.get(i + 1).copied() == Some(b'=') => {
                    out.push(ExprTok::Ge);
                    i += 2;
                }
                b'<' if bytes.get(i + 1).copied() == Some(b'<') => {
                    out.push(ExprTok::Shl);
                    i += 2;
                }
                b'>' if bytes.get(i + 1).copied() == Some(b'>') => {
                    out.push(ExprTok::Shr);
                    i += 2;
                }
                b'<' => {
                    out.push(ExprTok::Lt);
                    i += 1;
                }
                b'>' => {
                    out.push(ExprTok::Gt);
                    i += 1;
                }
                b'&' if bytes.get(i + 1).copied() == Some(b'&') => {
                    out.push(ExprTok::And);
                    i += 2;
                }
                b'|' if bytes.get(i + 1).copied() == Some(b'|') => {
                    out.push(ExprTok::Or);
                    i += 2;
                }
                b'&' => {
                    out.push(ExprTok::BitAnd);
                    i += 1;
                }
                b'|' => {
                    out.push(ExprTok::BitOr);
                    i += 1;
                }
                b'^' => {
                    out.push(ExprTok::BitXor);
                    i += 1;
                }
                b'~' => {
                    out.push(ExprTok::BitNot);
                    i += 1;
                }
                b'+' => {
                    out.push(ExprTok::Plus);
                    i += 1;
                }
                b'-' => {
                    out.push(ExprTok::Minus);
                    i += 1;
                }
                b'*' => {
                    out.push(ExprTok::Star);
                    i += 1;
                }
                b'/' => {
                    out.push(ExprTok::Slash);
                    i += 1;
                }
                b'%' => {
                    out.push(ExprTok::Percent);
                    i += 1;
                }
                b'?' => {
                    out.push(ExprTok::Question);
                    i += 1;
                }
                b':' => {
                    out.push(ExprTok::Colon);
                    i += 1;
                }
                b'!' => {
                    out.push(ExprTok::Not);
                    i += 1;
                }
                b'(' => {
                    out.push(ExprTok::LParen);
                    i += 1;
                }
                b')' => {
                    out.push(ExprTok::RParen);
                    i += 1;
                }
                other => {
                    let _ = other;
                    i += 1;
                }
            }
        }
    }
    out
}

pub(super) fn is_preprocessor_probe_builtin(ident: &str) -> bool {
    matches!(
        ident,
        "__has_include"
            | "__has_include_next"
            | "__has_attribute"
            | "__has_builtin"
            | "__has_feature"
            | "__has_extension"
            | "__has_embed"
            | "__has_warning"
            | "__has_c_attribute"
            | "__has_cpp_attribute"
            | "__has_declspec_attribute"
            | "__is_identifier"
    )
}

fn is_plain_identifier(bytes: &[u8]) -> bool {
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    is_ident_start(first) && rest.iter().copied().all(is_ident_continue)
}

pub(super) fn parse_expr_macro_args(src: &str, open_idx: usize) -> Option<(Vec<String>, usize)> {
    let bytes = src.as_bytes();
    if bytes.get(open_idx).copied() != Some(b'(') {
        return None;
    }
    let mut args = Vec::new();
    let mut depth = 0u32;
    let mut start = open_idx + 1;
    let mut i = open_idx + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i = i.saturating_add(2);
                        continue;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'(' => depth = depth.saturating_add(1),
            b')' if depth == 0 => {
                args.push(src[start..i].trim().to_string());
                return Some((args, i + 1));
            }
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                args.push(src[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

pub(super) fn substitute_expr_macro_params(
    macro_name: &str,
    replacement: &str,
    params: &[String],
    variadic: Option<&str>,
    args: &[String],
) -> String {
    if variadic.is_some() {
        if args.len() < params.len() {
            panic!(
                "macro `{macro_name}` requires at least {} arguments but received {}",
                params.len(),
                args.len()
            );
        }
    } else if args.len() != params.len() {
        panic!(
            "macro `{macro_name}` expects {} arguments but received {} arguments",
            params.len(),
            args.len()
        );
    }
    let _ = macro_name;
    let variadic_replacement = variadic.map(|_| args.get(params.len()..).unwrap_or(&[]).join(","));
    let bytes = replacement.as_bytes();
    let mut out = String::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if is_ident_start(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let name = &replacement[start..i];
            if let Some(param_index) = params.iter().position(|param| param == name) {
                let Some(arg) = args.get(param_index) else {
                    return "0".to_string();
                };
                out.push_str(arg);
            } else if Some(name) == variadic || name == "__VA_ARGS__" {
                out.push_str(variadic_replacement.as_deref().unwrap_or(""));
            } else {
                out.push_str(name);
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
