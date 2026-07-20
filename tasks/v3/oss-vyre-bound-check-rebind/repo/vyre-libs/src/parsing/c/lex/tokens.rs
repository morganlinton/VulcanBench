//! C11 token type constants for the GPU lexer pipeline.
//!
//! Each constant maps to a token category. Stored as `u32` for GPU
//! alignment. The numbering scheme reserves:
//! - `0..9`  -  literals and identifiers
//! - `10..39`  -  single-char punctuation
//! - `40..59`  -  multi-char operators
//! - `100..199`  -  keywords
//! - `200..255`  -  meta-tokens (stripped before structural analysis)
#![allow(missing_docs)]

// Token type constants (fits in u8, stored as u32 for GPU alignment)
pub const TOK_EOF: u32 = 0;
pub const TOK_IDENTIFIER: u32 = 1;
pub const TOK_INTEGER: u32 = 2;
pub const TOK_FLOAT: u32 = 3;
pub const TOK_STRING: u32 = 4;
pub const TOK_CHAR: u32 = 5;

// Punctuation
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

// Multi-char operators
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
pub const TOK_ELLIPSIS: u32 = 54; // ...
pub const TOK_PERCENT_EQ: u32 = 55;
pub const TOK_AMP_EQ: u32 = 56;
pub const TOK_PIPE_EQ: u32 = 57;
pub const TOK_CARET_EQ: u32 = 58;
pub const TOK_LSHIFT_EQ: u32 = 59;
pub const TOK_RSHIFT_EQ: u32 = 60;
pub const TOK_HASHHASH: u32 = 61; // ##

// Keywords (use 100+ range)
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
pub const TOK_GNU_AUTO_TYPE: u32 = 153;
pub const TOK_GNU_TYPEOF_UNQUAL: u32 = 154;
pub const TOK_GNU_INT128: u32 = 155;
pub const TOK_GNU_BUILTIN_VA_LIST: u32 = 156;
pub const TOK_GNU_ADDRESS_SPACE: u32 = 157;
pub const TOK_GNU_LABEL: u32 = 158;

// C23 / TS-extension scalar type keywords. Distinct token ids let the
// parser keep IEEE-binary16 / bfloat16 / fp16 distinct from `float`
// when lowering to vyre `DataType::F16` / `BF16` / `F32` / `F64`.
/// `_BitInt(N)` (C23): arbitrary-width signed/unsigned integer.
pub const TOK_BITINT_KW: u32 = 159;
/// `_Float16` (TS 18661-3 / C23): IEEE-754 binary16.
pub const TOK_FLOAT16_KW: u32 = 160;
/// `_Float32` (TS 18661-3): IEEE-754 binary32 (alias of `float`).
pub const TOK_FLOAT32_KW: u32 = 161;
/// `_Float64` (TS 18661-3): IEEE-754 binary64 (alias of `double`).
pub const TOK_FLOAT64_KW: u32 = 162;
/// `_Float128` (TS 18661-3 / GCC): IEEE-754 binary128.
pub const TOK_FLOAT128_KW: u32 = 163;
/// `__float128` (GCC): IEEE-754 binary128 (synonym of `_Float128`).
pub const TOK_GNU_FLOAT128_KW: u32 = 164;
/// `__bf16` (GCC/clang): brain-float16.
pub const TOK_GNU_BF16_KW: u32 = 165;
/// `__fp16` (GCC/clang): half-precision float storage type.
pub const TOK_GNU_FP16_KW: u32 = 166;
/// `_Decimal32` (TS 18661-2): IEEE-754 decimal32.
pub const TOK_DECIMAL32_KW: u32 = 167;
/// `_Decimal64` (TS 18661-2): IEEE-754 decimal64.
pub const TOK_DECIMAL64_KW: u32 = 168;
/// `_Decimal128` (TS 18661-2): IEEE-754 decimal128.
pub const TOK_DECIMAL128_KW: u32 = 169;
/// MSVC-compatibility `__forceinline` qualifier.
pub const TOK_FORCEINLINE_KW: u32 = 170;
/// `_Nonnull` / `_Nullable` / `_Null_unspecified` clang nullability
/// qualifier family. One token id; downstream sema distinguishes via
/// the identifier text.
pub const TOK_NULLABILITY_KW: u32 = 171;

pub const TOK_COMMENT: u32 = 200; // will be stripped
pub const TOK_WHITESPACE: u32 = 201; // will be stripped
pub const TOK_PREPROC: u32 = 202; // preprocessor directive
pub const TOK_ERR_UNTERMINATED_STRING: u32 = 240;
pub const TOK_ERR_UNTERMINATED_CHAR: u32 = 241;
pub const TOK_ERR_UNTERMINATED_COMMENT: u32 = 242;
pub const TOK_ERR_INVALID_ESCAPE: u32 = 243;

// Preprocessor directive sub-kinds. The lexer keeps directive rows as
// `TOK_PREPROC`; these stable IDs are for directive metadata streams and
// host-side validation that must distinguish the directive spelling.
pub const TOK_PP_NULL: u32 = 203;
pub const TOK_PP_DEFINE: u32 = 204;
pub const TOK_PP_UNDEF: u32 = 205;
pub const TOK_PP_INCLUDE: u32 = 206;
pub const TOK_PP_IF: u32 = 207;
pub const TOK_PP_IFDEF: u32 = 208;
pub const TOK_PP_IFNDEF: u32 = 209;
pub const TOK_PP_ELIF: u32 = 210;
pub const TOK_PP_ELSE: u32 = 211;
pub const TOK_PP_ENDIF: u32 = 212;
pub const TOK_PP_PRAGMA: u32 = 213;
pub const TOK_PP_LINE: u32 = 214;
pub const TOK_PP_ERROR: u32 = 215;
pub const TOK_PP_INCLUDE_NEXT: u32 = 216;
pub const TOK_PP_WARNING: u32 = 217;
pub const TOK_PP_IDENT: u32 = 218;
pub const TOK_PP_SCCS: u32 = 219;

// Preprocessor side-effect metadata IDs. These never replace lexer tokens;
// callers use them in parallel metadata streams attached to `TOK_PREPROC`.
pub const TOK_PP_EFFECT_INCLUDE: u32 = 220;
pub const TOK_PP_EFFECT_INCLUDE_NEXT: u32 = 221;
pub const TOK_PP_EFFECT_PRAGMA: u32 = 222;
pub const TOK_PP_EFFECT_PRAGMA_ONCE: u32 = 223;
pub const TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_PUSH: u32 = 224;
pub const TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_POP: u32 = 225;
pub const TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_IGNORED: u32 = 226;
pub const TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_WARNING: u32 = 227;
pub const TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_ERROR: u32 = 228;
pub const TOK_PP_EFFECT_ERROR_DIAGNOSTIC: u32 = 229;
pub const TOK_PP_EFFECT_WARNING_DIAGNOSTIC: u32 = 230;
pub const TOK_PP_EFFECT_IDENT: u32 = 231;
pub const TOK_PP_EFFECT_SCCS: u32 = 232;
pub const TOK_PP_EFFECT_LINE: u32 = 233;

// C23 + clang directive tokens.
/// `#embed` (C23): bring binary file contents into the translation unit.
pub const TOK_PP_EMBED: u32 = 234;
/// `#elifdef` (C23): shorthand for `#elif defined(...)`.
pub const TOK_PP_ELIFDEF: u32 = 235;
/// `#elifndef` (C23): shorthand for `#elif !defined(...)`.
pub const TOK_PP_ELIFNDEF: u32 = 236;
/// `#import` (clang/Objective-C): include-once form.
pub const TOK_PP_IMPORT: u32 = 237;

pub fn is_c_lexer_error_token(token: u32) -> bool {
    matches!(
        token,
        TOK_ERR_UNTERMINATED_STRING
            | TOK_ERR_UNTERMINATED_CHAR
            | TOK_ERR_UNTERMINATED_COMMENT
            | TOK_ERR_INVALID_ESCAPE
    )
}
