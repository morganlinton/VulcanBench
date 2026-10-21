use super::lexer_plan::source_can_use_regular_sparse_lexer;

pub(super) fn regular_c_lexer_fast_path_safe(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'#' | b'"' | b'\'' => return false,
            b'%' => return false,
            b'/' if bytes
                .get(i + 1)
                .copied()
                .is_some_and(|next| next == b'/' || next == b'*') =>
            {
                return false;
            }
            b'.' if bytes.get(i + 1).copied() == Some(b'.') => return false,
            b'<' if bytes
                .get(i + 1)
                .copied()
                .is_some_and(|next| next == b':' || next == b'%') =>
            {
                return false;
            }
            b':' if bytes.get(i + 1).copied() == Some(b'>') => return false,
            b'+' if bytes.get(i + 1).copied() == Some(b'+') => return false,
            b'-' if bytes
                .get(i + 1)
                .copied()
                .is_some_and(|next| next == b'-' || next == b'=') =>
            {
                return false;
            }
            b'&' if bytes.get(i + 1).copied() == Some(b'=') => return false,
            b'=' | b'!' | b'*' | b'/' | b'|' | b'^' if bytes.get(i + 1).copied() == Some(b'=') => {
                return false;
            }
            b'|' if bytes.get(i + 1).copied() == Some(b'|') => return false,
            b'<' if bytes.get(i + 1).copied() == Some(b'<')
                || bytes.get(i + 1).copied() == Some(b'=') =>
            {
                return false;
            }
            b'>' if bytes.get(i + 1).copied() == Some(b'>')
                || bytes.get(i + 1).copied() == Some(b'=') =>
            {
                return false;
            }
            b'.' if bytes
                .get(i + 1)
                .copied()
                .is_some_and(|next| next.is_ascii_digit()) =>
            {
                return false;
            }
            byte if byte.is_ascii_digit() => {
                if i > 0 && bytes[i - 1].is_ascii_alphabetic() {
                    i += 1;
                    continue;
                }
                let mut j = i + 1;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if bytes.get(j).copied().is_some_and(|next| {
                    matches!(next, b'.' | b'x' | b'X' | b'e' | b'E' | b'p' | b'P')
                }) {
                    return false;
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    true
}

pub(super) fn regular_c_ranked_lexer_fast_path_safe(source: &str) -> bool {
    source.len() <= 4096 && regular_c_lexer_fast_path_safe(source)
}

pub(super) fn regular_c_sparse_lexer_fast_path_safe(source: &str) -> bool {
    source_can_use_regular_sparse_lexer(source.as_bytes())
}

#[cfg(test)]
mod lexer_fast_path_tests {
    use super::regular_c_sparse_lexer_fast_path_safe;

    #[test]
    fn sparse_gpu_lexer_has_no_whole_translation_unit_size_cap() {
        let mut source = String::with_capacity(1024 * 1024 + 4096);
        while source.len() <= 1024 * 1024 + 1024 {
            source.push_str("int generated_binding = 7;\n");
        }
        assert!(
            regular_c_sparse_lexer_fast_path_safe(&source),
            "large safe translation units must stay on the sparse GPU lexer instead of falling through to slower dense plans"
        );
    }

    #[test]
    fn sparse_gpu_lexer_keeps_large_supported_literals_on_gpu_path() {
        let source = format!("const char *s = \"{}\";\n", "x".repeat(32 * 1024));
        assert!(
            regular_c_sparse_lexer_fast_path_safe(&source),
            "supported large literals must use the central CUDA sparse planner instead of an obsolete smaller guard"
        );
    }

    #[test]
    fn sparse_gpu_lexer_rejects_only_unscannable_literal_runs() {
        let source = format!("const char *s = \"{}\";\n", "x".repeat(65_536 + 1));
        assert!(
            !regular_c_sparse_lexer_fast_path_safe(&source),
            "a single literal beyond the CUDA sparse token scan window must still avoid the sparse lexer"
        );
    }
}
