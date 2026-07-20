#![allow(missing_docs)]

use crate::dfa::{DfaBuilder, DfaTable};
use regex_automata::MatchKind;

pub const TOK_IDENTIFIER: u32 = 1;
pub const TOK_INTEGER: u32 = 2;
//pub const TOK_FLOAT: u32 = 3;
pub const TOK_STRING: u32 = 4;
//pub const TOK_CHAR: u32 = 5;

pub const TOK_LPAREN: u32 = 10;
pub const TOK_RPAREN: u32 = 11;
pub const TOK_LBRACE: u32 = 12;
pub const TOK_RBRACE: u32 = 13;
pub const TOK_LBRACKET: u32 = 14;
pub const TOK_RBRACKET: u32 = 15;
pub const TOK_SEMICOLON: u32 = 16;
pub const TOK_COMMA: u32 = 17;
pub const TOK_DOT: u32 = 18;
pub const TOK_ARROW: u32 = 19; // ->
pub const TOK_PLUS: u32 = 20;
pub const TOK_MINUS: u32 = 21;
pub const TOK_STAR: u32 = 22;
pub const TOK_SLASH: u32 = 23;
pub const TOK_PERCENT: u32 = 24;
pub const TOK_AMP: u32 = 25;
pub const TOK_PIPE: u32 = 26;
pub const TOK_CARET: u32 = 27;
pub const TOK_TILDE: u32 = 28;
pub const TOK_BANG: u32 = 29;
pub const TOK_ASSIGN: u32 = 30; // =
pub const TOK_LT: u32 = 31;
pub const TOK_GT: u32 = 32;
pub const TOK_HASH: u32 = 33; // preprocessor
pub const TOK_QUESTION: u32 = 34;
pub const TOK_COLON: u32 = 35;

pub const TOK_EQ: u32 = 40; // ==
pub const TOK_NE: u32 = 41; // !=
pub const TOK_LE: u32 = 42; // <=
pub const TOK_GE: u32 = 43; // >=
pub const TOK_AND: u32 = 44; // &&
pub const TOK_OR: u32 = 45; // ||
pub const TOK_LSHIFT: u32 = 46; // <<
pub const TOK_RSHIFT: u32 = 47; // >>
pub const TOK_INC: u32 = 48; // ++
pub const TOK_DEC: u32 = 49; // --
pub const TOK_PLUS_EQ: u32 = 50;
pub const TOK_MINUS_EQ: u32 = 51;
pub const TOK_STAR_EQ: u32 = 52;
pub const TOK_SLASH_EQ: u32 = 53;
pub const TOK_ELLIPSIS: u32 = 54;
pub const TOK_PERCENT_EQ: u32 = 55;
pub const TOK_AMP_EQ: u32 = 56;
pub const TOK_PIPE_EQ: u32 = 57;
pub const TOK_CARET_EQ: u32 = 58;
pub const TOK_LSHIFT_EQ: u32 = 59;
pub const TOK_RSHIFT_EQ: u32 = 60;
pub const TOK_HASHHASH: u32 = 61;

pub const TOK_IF: u32 = 100;
pub const TOK_ELSE: u32 = 101;
pub const TOK_FOR: u32 = 102;
pub const TOK_WHILE: u32 = 103;
pub const TOK_RETURN: u32 = 104;
pub const TOK_STRUCT: u32 = 105;
pub const TOK_TYPEDEF: u32 = 106;
pub const TOK_INT: u32 = 107;
pub const TOK_CHAR_KW: u32 = 108;
pub const TOK_VOID: u32 = 109;
pub const TOK_DO: u32 = 110;
pub const TOK_SWITCH: u32 = 111;
pub const TOK_CASE: u32 = 112;
pub const TOK_DEFAULT: u32 = 113;
pub const TOK_BREAK: u32 = 114;
pub const TOK_CONTINUE: u32 = 115;
pub const TOK_GOTO: u32 = 116;
pub const TOK_SIZEOF: u32 = 117;
pub const TOK_AUTO: u32 = 118;
pub const TOK_CONST: u32 = 119;
pub const TOK_DOUBLE: u32 = 120;
pub const TOK_ENUM: u32 = 121;
pub const TOK_EXTERN: u32 = 122;
pub const TOK_FLOAT_KW: u32 = 123;
pub const TOK_INLINE: u32 = 124;
pub const TOK_LONG: u32 = 125;
pub const TOK_REGISTER: u32 = 126;
pub const TOK_RESTRICT: u32 = 127;
pub const TOK_SHORT: u32 = 128;
pub const TOK_SIGNED: u32 = 129;
pub const TOK_STATIC: u32 = 130;
pub const TOK_UNION: u32 = 131;
pub const TOK_UNSIGNED: u32 = 132;
pub const TOK_VOLATILE: u32 = 133;
pub const TOK_ALIGNAS: u32 = 134;
pub const TOK_ALIGNOF: u32 = 135;
pub const TOK_ATOMIC: u32 = 136;
pub const TOK_BOOL: u32 = 137;
pub const TOK_COMPLEX: u32 = 138;
pub const TOK_GENERIC: u32 = 139;
pub const TOK_IMAGINARY: u32 = 140;
pub const TOK_NORETURN: u32 = 141;
pub const TOK_STATIC_ASSERT: u32 = 142;
pub const TOK_THREAD_LOCAL: u32 = 143;
pub const TOK_GNU_ASM: u32 = 144;
pub const TOK_GNU_ATTRIBUTE: u32 = 145;
pub const TOK_GNU_TYPEOF: u32 = 146;
pub const TOK_GNU_EXTENSION: u32 = 147;
pub const TOK_GNU_REAL: u32 = 148;
pub const TOK_GNU_IMAG: u32 = 149;
pub const TOK_BUILTIN_CONSTANT_P: u32 = 150;
pub const TOK_BUILTIN_CHOOSE_EXPR: u32 = 151;
pub const TOK_BUILTIN_TYPES_COMPATIBLE_P: u32 = 152;
pub const TOK_COMMENT: u32 = 200; // will be stripped
pub const TOK_WHITESPACE: u32 = 201; // will be stripped
pub const TOK_PREPROC: u32 = 202; // preprocessor directive

/// `(token_id, regex source)` in **priority** order: earlier wins on tie length
/// in [`crate::lex_c11_max_munch`].
pub const C11_PATTERNS: &[(u32, &str)] = &[
    (TOK_AUTO, r"auto"),
    (TOK_BREAK, r"break"),
    (TOK_CASE, r"case"),
    (TOK_CHAR_KW, r"char"),
    (TOK_CONST, r"const"),
    (TOK_CONTINUE, r"continue"),
    (TOK_DEFAULT, r"default"),
    (TOK_DO, r"do"),
    (TOK_DOUBLE, r"double"),
    (TOK_ELSE, r"else"),
    (TOK_ENUM, r"enum"),
    (TOK_EXTERN, r"extern"),
    (TOK_FLOAT_KW, r"float"),
    (TOK_FOR, r"for"),
    (TOK_GOTO, r"goto"),
    (TOK_IF, r"if"),
    (TOK_INLINE, r"inline"),
    (TOK_INT, r"int"),
    (TOK_LONG, r"long"),
    (TOK_REGISTER, r"register"),
    (TOK_RESTRICT, r"restrict"),
    (TOK_RETURN, r"return"),
    (TOK_SHORT, r"short"),
    (TOK_SIGNED, r"signed"),
    (TOK_SIZEOF, r"sizeof"),
    (TOK_STATIC, r"static"),
    (TOK_STRUCT, r"struct"),
    (TOK_SWITCH, r"switch"),
    (TOK_TYPEDEF, r"typedef"),
    (TOK_UNION, r"union"),
    (TOK_UNSIGNED, r"unsigned"),
    (TOK_VOID, r"void"),
    (TOK_VOLATILE, r"volatile"),
    (TOK_WHILE, r"while"),
    (TOK_ALIGNAS, r"_Alignas"),
    (TOK_ALIGNOF, r"_Alignof"),
    (TOK_ATOMIC, r"_Atomic"),
    (TOK_BOOL, r"_Bool"),
    (TOK_COMPLEX, r"_Complex"),
    (TOK_GENERIC, r"_Generic"),
    (TOK_IMAGINARY, r"_Imaginary"),
    (TOK_NORETURN, r"_Noreturn"),
    (TOK_STATIC_ASSERT, r"_Static_assert"),
    (TOK_THREAD_LOCAL, r"_Thread_local"),
    (TOK_GNU_ASM, r"asm"),
    (TOK_GNU_ASM, r"__asm"),
    (TOK_GNU_ASM, r"__asm__"),
    (TOK_GNU_ATTRIBUTE, r"__attribute"),
    (TOK_GNU_ATTRIBUTE, r"__attribute__"),
    (TOK_GNU_TYPEOF, r"typeof"),
    (TOK_GNU_TYPEOF, r"__typeof"),
    (TOK_GNU_TYPEOF, r"__typeof__"),
    (TOK_GNU_EXTENSION, r"__extension__"),
    (TOK_ALIGNOF, r"__alignof"),
    (TOK_ALIGNOF, r"__alignof__"),
    (TOK_INLINE, r"__inline"),
    (TOK_INLINE, r"__inline__"),
    (TOK_COMPLEX, r"__complex__"),
    (TOK_GNU_REAL, r"__real__"),
    (TOK_GNU_IMAG, r"__imag__"),
    (TOK_VOLATILE, r"__volatile__"),
    (TOK_BUILTIN_CONSTANT_P, r"__builtin_constant_p"),
    (TOK_BUILTIN_CHOOSE_EXPR, r"__builtin_choose_expr"),
    (
        TOK_BUILTIN_TYPES_COMPATIBLE_P,
        r"__builtin_types_compatible_p",
    ),
    (TOK_IDENTIFIER, r"[a-zA-Z_][a-zA-Z0-9_]*"),
    (TOK_INTEGER, r"0[xX][0-9a-fA-F]+|0[0-7]*|[1-9][0-9]*"),
    (TOK_STRING, r#""([^"\\]|\\.)*""#),
    (TOK_WHITESPACE, r"[ \t\n\r\v\f]+"),
    (TOK_COMMENT, r"//[^\n]*"),
    (TOK_COMMENT, r"/\*([^*]|\*[^/])*\*/"),
    (TOK_PREPROC, r"#[^\n]*"),
    (TOK_HASH, r"#"),
    (TOK_ARROW, r"->"),
    (TOK_INC, r"\+\+"),
    (TOK_DEC, r"--"),
    (TOK_PLUS_EQ, r"\+="),
    (TOK_MINUS_EQ, r"-="),
    (TOK_STAR_EQ, r"\*="),
    (TOK_SLASH_EQ, r"/="),
    (TOK_LSHIFT_EQ, r"<<="),
    (TOK_RSHIFT_EQ, r">>="),
    (TOK_PERCENT_EQ, r"%="),
    (TOK_AMP_EQ, r"&="),
    (TOK_PIPE_EQ, r"\|="),
    (TOK_CARET_EQ, r"\^="),
    (TOK_HASHHASH, r"##"),
    (TOK_EQ, r"=="),
    (TOK_NE, r"!="),
    (TOK_LE, r"<="),
    (TOK_GE, r">="),
    (TOK_AND, r"&&"),
    (TOK_OR, r"\|\|"),
    (TOK_LSHIFT, r"<<"),
    (TOK_RSHIFT, r">>"),
    (TOK_ELLIPSIS, r"\.\.\."),
    (TOK_LPAREN, r"\("),
    (TOK_RPAREN, r"\)"),
    (TOK_LBRACE, r"\{"),
    (TOK_RBRACE, r"\}"),
    (TOK_LBRACKET, r"\["),
    (TOK_RBRACKET, r"\]"),
    (TOK_SEMICOLON, r";"),
    (TOK_COMMA, r","),
    (TOK_DOT, r"\."),
    (TOK_PLUS, r"\+"),
    (TOK_MINUS, r"-"),
    (TOK_STAR, r"\*"),
    (TOK_SLASH, r"/"),
    (TOK_PERCENT, r"%"),
    (TOK_AMP, r"&"),
    (TOK_PIPE, r"\|"),
    (TOK_CARET, r"\^"),
    (TOK_TILDE, r"~"),
    (TOK_BANG, r"!"),
    (TOK_ASSIGN, r"="),
    (TOK_LT, r"<"),
    (TOK_GT, r">"),
    (TOK_QUESTION, r"\?"),
    (TOK_COLON, r":"),
];

fn add_c11_patterns(b: &mut DfaBuilder) {
    for &(id, p) in C11_PATTERNS {
        b.add_pattern(id, p);
    }
}

/// DFA for GPU / `SGGC` blobs: [`MatchKind::All`], wire-stable with existing paths.
///
/// # Panics
///
/// Panics if any pattern in [`C11_PATTERNS`] fails to compile. All patterns are
/// compile-time constants; a failure here is a programmer error (broken pattern
/// in the source), not a runtime condition. Silent recovery would produce a
/// zero-recall DFA that rejects all input on the GPU.
// INTENTIONAL: C11_PATTERNS is a compile-time constant; any build failure is a
// programmer error that must abort loudly, not silently degrade to an all-Error DFA.
#[allow(clippy::expect_used)]
pub fn build_c11_lexer_dfa() -> DfaTable {
    let mut b = DfaBuilder::new(0, 0);
    add_c11_patterns(&mut b);
    b.build().expect(
        "Fix: C11 lexer DFA build failed. A pattern in C11_PATTERNS is invalid. \
         All patterns are compile-time constants — fix the broken pattern.",
    )
}

/// **Host** DFA: [`MatchKind::LeftmostFirst`], for DFA table experiments (not
/// the regex-based [`crate::lex_c11_max_munch`]).
///
/// # Panics
///
/// Panics if any pattern in [`C11_PATTERNS`] fails to compile. All patterns are
/// compile-time constants; a failure here is a programmer error, not a runtime
/// condition. Silent recovery would produce a zero-recall DFA.
// INTENTIONAL: same programmer-error contract as build_c11_lexer_dfa.
#[must_use]
#[allow(clippy::expect_used)]
pub fn build_c11_lexer_dfa_for_host() -> DfaTable {
    let mut b = DfaBuilder::new(0, 0);
    add_c11_patterns(&mut b);
    b.build_with_match_kind(MatchKind::LeftmostFirst).expect(
        "Fix: C11 host DFA build failed. A pattern in C11_PATTERNS is invalid. \
         All patterns are compile-time constants — fix the broken pattern.",
    )
}
