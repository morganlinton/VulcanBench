use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pipeline) enum SparseLexerSourceClass {
    Rejected,
    Megakernel,
    FastNoLiterals,
}

impl SparseLexerSourceClass {
    pub(in crate::pipeline) fn accepts_sparse_lexer(self) -> bool {
        !matches!(self, Self::Rejected)
    }

    #[cfg(test)]
    pub(in crate::pipeline) fn skips_literal_backscan(self) -> bool {
        matches!(self, Self::FastNoLiterals)
    }
}

pub(in crate::pipeline) fn classify_regular_sparse_lexer_source(
    source: &[u8],
) -> SparseLexerSourceClass {
    let trace_reject = std::env::var_os("VYRE_SPARSE_LEXER_TRACE_REJECT").is_some();
    let mut i = 0usize;
    let mut line_allows_directive = true;
    let mut can_skip_literal_backscan = true;
    while i < source.len() {
        match source[i] {
            b'\n' | b'\r' => {
                line_allows_directive = true;
                i += 1;
                continue;
            }
            b' ' | b'\t' if line_allows_directive => {}
            b'#' if line_allows_directive => {
                line_allows_directive = false;
                can_skip_literal_backscan = false;
            }
            _ => line_allows_directive = false,
        }
        match source[i] {
            b'L' | b'U' | b'u' => {
                if let Some(end) = sparse_prefixed_char_literal_end(source, i) {
                    can_skip_literal_backscan = false;
                    i = end;
                    continue;
                }
            }
            b'"' => {
                let Some(end) = sparse_string_literal_end(source, i) else {
                    reject_sparse_source(trace_reject, source, i, "unsupported string literal");
                    return SparseLexerSourceClass::Rejected;
                };
                can_skip_literal_backscan = false;
                i = end;
                continue;
            }
            b'\'' => {
                let Some(end) = sparse_char_literal_end(source, i) else {
                    if matches!(
                        source.get(i.wrapping_sub(1)).copied(),
                        Some(b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9')
                    ) {
                        i += 1;
                        continue;
                    }
                    reject_sparse_source(trace_reject, source, i, "unsupported char literal");
                    return SparseLexerSourceClass::Rejected;
                };
                can_skip_literal_backscan = false;
                i = end;
                continue;
            }
            b'\\' => {
                reject_sparse_source(trace_reject, source, i, "raw backslash");
                return SparseLexerSourceClass::Rejected;
            }
            b'#' => {
                // The sparse GPU lexer classifies `##` through the shared max-munch
                // operator table and keeps directive-line paste operators inside one
                // TOK_PREPROC row, so token paste is not a sparse-lexer correctness gap.
            }
            b'0'..=b'9' => {
                if !matches!(
                    source.get(i.wrapping_sub(1)).copied(),
                    Some(b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9')
                ) {
                    if !sparse_numeric_literal_supported(source, i) {
                        reject_sparse_source(
                            trace_reject,
                            source,
                            i,
                            "unsupported numeric literal",
                        );
                        return SparseLexerSourceClass::Rejected;
                    }
                    i = sparse_numeric_literal_end(source, i);
                    continue;
                }
            }
            b'.' => {
                if matches!(source.get(i + 1).copied(), Some(b'0'..=b'9')) {
                    if !sparse_numeric_literal_supported(source, i) {
                        reject_sparse_source(
                            trace_reject,
                            source,
                            i,
                            "unsupported numeric literal",
                        );
                        return SparseLexerSourceClass::Rejected;
                    }
                    i = sparse_numeric_literal_end(source, i);
                    continue;
                }
                if matches!(source.get(i + 1).copied(), Some(b'.'))
                    && matches!(source.get(i + 2).copied(), Some(b'.'))
                    && !matches!(source.get(i.wrapping_sub(1)).copied(), Some(b'.'))
                {
                    i += 3;
                    continue;
                }
                if matches!(source.get(i + 1).copied(), Some(b'.')) {
                    reject_sparse_source(trace_reject, source, i, "unsupported dot literal");
                    return SparseLexerSourceClass::Rejected;
                }
            }
            b'/' if matches!(source.get(i + 1).copied(), Some(b'/')) => {
                can_skip_literal_backscan = false;
                i = sparse_line_comment_end(source, i);
                line_allows_directive = false;
                continue;
            }
            b'/' if matches!(source.get(i + 1).copied(), Some(b'*')) => {
                let Some(end) = sparse_block_comment_end(source, i) else {
                    reject_sparse_source(trace_reject, source, i, "unterminated block comment");
                    return SparseLexerSourceClass::Rejected;
                };
                can_skip_literal_backscan = false;
                i = end;
                line_allows_directive = false;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    if can_skip_literal_backscan {
        SparseLexerSourceClass::FastNoLiterals
    } else {
        SparseLexerSourceClass::Megakernel
    }
}

pub(in crate::pipeline) fn source_can_use_regular_sparse_lexer(source: &[u8]) -> bool {
    classify_regular_sparse_lexer_source(source).accepts_sparse_lexer()
}

#[cfg(test)]
pub(in crate::pipeline) fn source_can_skip_literal_backscan(source: &[u8]) -> bool {
    classify_regular_sparse_lexer_source(source).skips_literal_backscan()
}

pub(super) fn reject_sparse_source(
    trace_reject: bool,
    source: &[u8],
    offset: usize,
    reason: &str,
) -> bool {
    if trace_reject {
        let start = offset.saturating_sub(48);
        let end = source.len().min(offset + 48);
        let snippet = String::from_utf8_lossy(&source[start..end]);
        eprintln!("[sparse-lexer-plan] reject offset={offset} reason={reason} context={snippet:?}",);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        classify_regular_sparse_lexer_source, source_can_skip_literal_backscan,
        source_can_use_regular_sparse_lexer, SparseLexerSourceClass,
    };

    #[test]
    fn no_literal_backscan_accepts_plain_c_tokens() {
        assert!(source_can_skip_literal_backscan(
            b"typedef int T;\nstatic int f(void) { int x = 7; return x; }\n"
        ));
    }

    #[test]
    fn no_literal_backscan_rejects_literals_comments_and_directives() {
        for source in [
            b"const char *s = \"x\";\n".as_slice(),
            b"int c = 'x';\n".as_slice(),
            b"int x; // comment\n".as_slice(),
            b"int x; /* comment */\n".as_slice(),
            b"#define X 1\nint x = X;\n".as_slice(),
            b"int x = \\\n1;\n".as_slice(),
        ] {
            assert!(!source_can_skip_literal_backscan(source));
        }
    }

    #[test]
    fn sparse_cuda_lexer_planner_allows_integer_regular_c() {
        assert!(source_can_use_regular_sparse_lexer(
            b"int f(int x){return x / 2;}"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"int f(int x){x /= 2; return x;}"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"int f(struct s v){return v.x;}"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"int f(struct s v1){return v1.x;}"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"int printf(const char *fmt, ...);"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"__attribute__((section(\".data\"))) int x;"
        ));
        assert!(source_can_use_regular_sparse_lexer(b"char *s = \"x\";"));
        assert!(source_can_use_regular_sparse_lexer(b"char c = 'x';"));
        assert!(source_can_use_regular_sparse_lexer(b"char c = '\\n';"));
        assert!(source_can_use_regular_sparse_lexer(b"wchar_t c = L'\\n';"));
        assert!(source_can_use_regular_sparse_lexer(
            b"char16_t c = u'\\u1234';"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"char32_t c = U'\\U00001234';"
        ));
        assert!(source_can_use_regular_sparse_lexer(b"char8_t c = u8'\\n';"));
        assert!(source_can_use_regular_sparse_lexer(
            b"unsigned long x = 0x10UL;"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"unsigned x = 0b1010U;"
        ));
        assert!(source_can_use_regular_sparse_lexer(b"unsigned x = 1'000U;"));
        assert!(source_can_use_regular_sparse_lexer(b"double x = 3.14;"));
        assert!(source_can_use_regular_sparse_lexer(b"double x = 3.;"));
        assert!(source_can_use_regular_sparse_lexer(b"double x = .5;"));
        assert!(source_can_use_regular_sparse_lexer(b"double x = 1e10;"));
        assert!(source_can_use_regular_sparse_lexer(b"double x = 1e+10;"));
        assert!(source_can_use_regular_sparse_lexer(b"double x = 0x1p4;"));
        assert!(source_can_use_regular_sparse_lexer(b"double x = 0x1.8p+2;"));
        assert!(source_can_use_regular_sparse_lexer(
            b"int x; // comment\nint y;"
        ));
        assert!(source_can_use_regular_sparse_lexer(
            b"int x; /* comment */ int y;"
        ));
    }

    #[test]
    fn sparse_cuda_lexer_planner_classifies_strategy_in_one_pass() {
        assert_eq!(
            classify_regular_sparse_lexer_source(
                b"typedef int T;\nstatic int f(void) { int x = 7; return x; }\n"
            ),
            SparseLexerSourceClass::FastNoLiterals
        );
        assert_eq!(
            classify_regular_sparse_lexer_source(b"const char *s = \"x\";\n"),
            SparseLexerSourceClass::Megakernel
        );
        assert_eq!(
            classify_regular_sparse_lexer_source(b"char *s = \"unterminated;"),
            SparseLexerSourceClass::Rejected
        );
    }

    #[test]
    fn sparse_cuda_lexer_planner_accepts_directive_lines() {
        // After the directive-line rejection was lifted (preprocessed C
        // sources still contain #define lines that the sparse lexer
        // tokenizes as regular tokens), directive lines are accepted.
        assert!(source_can_use_regular_sparse_lexer(b"#define X 1\n"));
        assert!(source_can_use_regular_sparse_lexer(
            b"#define CAT(a,b) a ## b\n"
        ));
        assert!(source_can_use_regular_sparse_lexer(b"a ## b\n"));
    }

    #[test]
    fn sparse_cuda_lexer_planner_has_no_whole_source_size_cap() {
        let mut source = Vec::with_capacity(1024 * 1024 + 4096);
        while source.len() <= 1024 * 1024 + 1024 {
            source.extend_from_slice(b"int generated_binding = 7;\n");
        }
        assert!(
            source_can_use_regular_sparse_lexer(&source),
            "large safe C sources must stay eligible for CUDA sparse lexing; only per-token backscan length is a correctness limit"
        );
    }

    #[test]
    fn sparse_cuda_lexer_planner_rejects_regular_lexer_gaps() {
        assert!(!source_can_use_regular_sparse_lexer(
            b"char *s = \"unterminated;"
        ));
        assert!(!source_can_use_regular_sparse_lexer(
            b"char c = 'unterminated;"
        ));
        assert!(!source_can_use_regular_sparse_lexer(b"int x = 0x1g;"));
        assert!(!source_can_use_regular_sparse_lexer(b"int x = 0b102;"));
        assert!(!source_can_use_regular_sparse_lexer(b"int x = 0x;"));
        assert!(!source_can_use_regular_sparse_lexer(b"int x = 0b;"));
        assert!(!source_can_use_regular_sparse_lexer(b"int x = 1''0;"));
        assert!(!source_can_use_regular_sparse_lexer(b"int x = 1';"));
        assert!(!source_can_use_regular_sparse_lexer(b"double x = 1e;"));
        assert!(!source_can_use_regular_sparse_lexer(b"double x = 1e+;"));
        assert!(!source_can_use_regular_sparse_lexer(b"double x = 0x1p;"));
        assert!(!source_can_use_regular_sparse_lexer(b"double x = 0x1.;"));
        assert!(!source_can_use_regular_sparse_lexer(b"int bad = ..;"));
        assert!(!source_can_use_regular_sparse_lexer(
            b"int x; /* unterminated"
        ));
    }
}
