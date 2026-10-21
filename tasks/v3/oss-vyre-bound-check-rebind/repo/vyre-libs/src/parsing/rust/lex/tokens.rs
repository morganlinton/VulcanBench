//! Token constants for the Rust nano-subset lexer.

/// End of file.
pub const EOF: u16 = 0;
/// Integer literal.
pub const LITERAL_INT: u16 = 1;
/// Boolean literal.
pub const LITERAL_BOOL: u16 = 2;
/// Identifier.
pub const IDENT: u16 = 10;
/// `fn` keyword.
pub const KW_FN: u16 = 20;
/// `let` keyword.
pub const KW_LET: u16 = 21;
/// `mut` keyword.
pub const KW_MUT: u16 = 22;
/// `if` keyword.
pub const KW_IF: u16 = 23;
/// `else` keyword.
pub const KW_ELSE: u16 = 24;
/// `return` keyword.
pub const KW_RETURN: u16 = 25;
/// `while` keyword.
pub const KW_WHILE: u16 = 26;
/// `for` keyword.
pub const KW_FOR: u16 = 27;
/// `in` keyword.
pub const KW_IN: u16 = 28;
/// `i32` type keyword.
pub const KW_I32: u16 = 30;
/// `bool` type keyword.
pub const KW_BOOL: u16 = 31;
/// `+` operator.
pub const PLUS: u16 = 40;
/// `-` operator.
pub const MINUS: u16 = 41;
/// `*` operator.
pub const STAR: u16 = 42;
/// `/` operator.
pub const SLASH: u16 = 43;
/// `==` operator.
pub const EQ: u16 = 44;
/// `<` operator.
pub const LT: u16 = 45;
/// `=` operator.
pub const ASSIGN: u16 = 46;
/// `;` punctuation.
pub const SEMI: u16 = 47;
/// `:` punctuation.
pub const COLON: u16 = 48;
/// `,` punctuation.
pub const COMMA: u16 = 49;
/// `->` punctuation.
pub const ARROW: u16 = 50;
/// `&` borrow operator.
pub const AMP: u16 = 51;
/// `&mut` borrow operator.
pub const AMP_MUT: u16 = 52;
/// `!` operator.
pub const BANG: u16 = 53;
/// `%` operator.
pub const PERCENT: u16 = 54;
/// `>` operator.
pub const GT: u16 = 55;
/// `<=` operator.
pub const LE: u16 = 56;
/// `>=` operator.
pub const GE: u16 = 57;
/// `!=` operator.
pub const NE: u16 = 58;
/// `(` delimiter.
pub const LPAREN: u16 = 60;
/// `)` delimiter.
pub const RPAREN: u16 = 61;
/// `{` delimiter.
pub const LBRACE: u16 = 62;
/// `}` delimiter.
pub const RBRACE: u16 = 63;
/// `&&` logical-and operator.
pub const ANDAND: u16 = 64;
/// `||` logical-or operator.
pub const OROR: u16 = 65;
/// `+=` compound-assignment operator.
pub const PLUS_EQ: u16 = 66;
/// `-=` compound-assignment operator.
pub const MINUS_EQ: u16 = 67;
/// `..` half-open range operator.
pub const DOTDOT: u16 = 68;
/// Unrecognised token.
pub const ERROR: u16 = 0xFFFE;
/// Token outside the nano-subset.
pub const UNSUPPORTED: u16 = 0xFFFF;

/// True if the token id represents a literal.
pub const fn is_literal(tok: u16) -> bool {
    matches!(tok, LITERAL_INT | LITERAL_BOOL)
}

/// True if the token id represents a binary operator.
pub const fn is_binop(tok: u16) -> bool {
    matches!(
        tok,
        PLUS | MINUS | STAR | SLASH | EQ | LT | PERCENT | GT | LE | GE | NE
    )
}
