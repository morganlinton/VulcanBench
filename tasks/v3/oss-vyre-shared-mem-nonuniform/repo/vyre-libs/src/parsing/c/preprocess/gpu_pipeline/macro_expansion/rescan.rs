use super::*;

pub(crate) fn disabled_self_recursive_macro_names<'a>(
    expanded_this_pass: &'a [MacroDef],
) -> HashSet<&'a [u8]> {
    expanded_this_pass
        .iter()
        .filter(|mac| macro_body_mentions_name(&mac.body, &mac.name))
        .map(|mac| mac.name.as_slice())
        .collect::<HashSet<_>>()
}

pub(crate) fn macro_body_mentions_name(body: &[u8], name: &[u8]) -> bool {
    let mut idx = 0usize;
    while idx < body.len() {
        if is_ident_start(body[idx]) {
            let start = idx;
            idx += 1;
            while idx < body.len() && is_ident_continue(body[idx]) {
                idx += 1;
            }
            if &body[start..idx] == name {
                return true;
            }
        } else {
            idx += 1;
        }
    }
    false
}

pub(crate) fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

pub(crate) fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}
