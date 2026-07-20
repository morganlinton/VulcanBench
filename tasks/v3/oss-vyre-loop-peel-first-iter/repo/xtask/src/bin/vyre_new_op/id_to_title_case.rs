#![allow(missing_docs)]
pub(crate) fn id_to_title_case(id: &str) -> String {
    id.rsplit('.')
        .next()
        .unwrap_or(id)
        .split('_')
        .map(|piece| {
            let mut chars = piece.chars();
            match chars.next() {
                Some(first) => {
                    format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
