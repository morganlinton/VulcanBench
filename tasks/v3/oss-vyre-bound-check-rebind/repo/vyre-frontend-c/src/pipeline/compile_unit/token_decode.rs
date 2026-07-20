use super::*;

pub(super) type DecodedObjectTokens = DecodedC11Tokens;

pub(super) fn decode_object_tokens(
    path: &Path,
    source: &str,
    lexed: &lex_stage::ObjectLexTokens,
    trace: &mut trace::CompileTrace,
) -> Result<DecodedObjectTokens, String> {
    decode_c11_tokens(
        path,
        source,
        &lexed.types,
        &lexed.starts,
        &lexed.lens,
        lexed.n_tokens,
        |stage| trace.log(stage),
    )
}
