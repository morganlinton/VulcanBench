//! Nano-subset Rust parser.

use crate::parsing::rust::lex::lexer::core::Token;
use crate::parsing::rust::lex::tokens::*;

/// Expression AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Integer literal with source offset and value.
    LiteralInt(u32, u64),
    /// Boolean literal with source offset and value.
    LiteralBool(u32, bool),
    /// Variable reference by source offset.
    Var(u32),
    /// Binary operation.
    Binary {
        /// Operator token kind.
        op: u16,
        /// Left-hand side.
        lhs: Box<Expr>,
        /// Right-hand side.
        rhs: Box<Expr>,
    },
    /// Borrow expression.
    Borrow {
        /// Whether the borrow is mutable.
        mutable: bool,
        /// Borrowed expression.
        expr: Box<Expr>,
    },
    /// Dereference.
    Deref(Box<Expr>),
    /// Logical negation (`!expr`).
    Not(Box<Expr>),
    /// Arithmetic negation (`-expr`).
    Neg(Box<Expr>),
    /// Function call.
    Call {
        /// Function name source offset.
        name: u32,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// Block expression.
    Block(Vec<Stmt>),
    /// Conditional.
    If {
        /// Condition.
        cond: Box<Expr>,
        /// Then block.
        then_block: Box<Expr>,
        /// Else block (optional).
        else_block: Option<Box<Expr>>,
    },
}

/// Statement AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Let binding.
    Let {
        /// Whether the binding is mutable.
        mutable: bool,
        /// Name source offset.
        name: u32,
        /// Declared type.
        ty: Type,
        /// Initializer expression.
        init: Expr,
    },
    /// Expression statement.
    Expr(Expr),
    /// Assignment to an existing binding (`name = value;`).
    Assign {
        /// Target name source offset.
        name: u32,
        /// Assigned value.
        value: Expr,
    },
    /// Return statement.
    Return(Option<Expr>),
    /// While loop (`while cond { body }`).
    While {
        /// Loop condition.
        cond: Expr,
        /// Loop body.
        body: Vec<Stmt>,
    },
    /// Half-open range loop (`for name in start..end { body }`).
    For {
        /// Loop variable name source offset.
        name: u32,
        /// Inclusive start expression.
        start: Expr,
        /// Exclusive end expression.
        end: Expr,
        /// Loop body.
        body: Vec<Stmt>,
    },
}

/// Types in the nano-subset.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// 32-bit signed integer.
    I32,
    /// Boolean.
    Bool,
    /// Unit type.
    Unit,
    /// Reference type.
    Ref {
        /// Whether the reference is mutable.
        mutable: bool,
        /// Inner type.
        inner: Box<Type>,
    },
}

/// Function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// Name source offset.
    pub name: u32,
    /// Parameters: (name offset, type).
    pub params: Vec<(u32, Type)>,
    /// Return type.
    pub ret: Type,
    /// Body statements.
    pub body: Vec<Stmt>,
}

/// A parsed module.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    /// Functions in the module.
    pub functions: Vec<Function>,
}

/// Parse error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Error message.
    pub message: String,
    /// Token index where the error occurred.
    pub token_index: usize,
}

/// Maximum recursive-descent nesting depth. Hostile input (e.g. thousands of
/// nested parens or `* ! &mut` chains) would otherwise recurse until the native
/// stack overflows — an uncatchable process abort and a clean algorithmic-DoS
/// vector for the frontend. We fail closed with a typed `ParseError` well below
/// any stack limit; real programs never approach this depth.
const MAX_PARSE_DEPTH: usize = 256;

/// Parse a token stream into a `Module`.
pub fn parse(source: &[u8], tokens: &[Token]) -> Result<Module, ParseError> {
    let mut p = Parser {
        source,
        tokens,
        pos: 0,
        depth: 0,
    };
    p.parse_module()
}

struct Parser<'a> {
    source: &'a [u8],
    tokens: &'a [Token],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect_token(&mut self, kind: u16) -> Result<&Token, ParseError> {
        let tok = self.peek();
        if tok.kind == kind {
            Ok(self.advance())
        } else {
            Err(ParseError {
                message: format!("expected token kind {}, got {}", kind, tok.kind),
                token_index: self.pos,
            })
        }
    }

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut functions = Vec::new();
        while self.peek().kind != EOF {
            functions.push(self.parse_function()?);
        }
        Ok(Module { functions })
    }

    fn parse_function(&mut self) -> Result<Function, ParseError> {
        self.expect_token(KW_FN)?;
        let name = self.expect_token(IDENT)?.start;
        self.expect_token(LPAREN)?;
        let params = self.parse_params()?;
        self.expect_token(RPAREN)?;
        let ret = if self.peek().kind == ARROW {
            self.advance();
            self.parse_type()?
        } else {
            Type::Unit
        };
        let body = self.parse_block()?;
        Ok(Function {
            name,
            params,
            ret,
            body,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<(u32, Type)>, ParseError> {
        let mut params = Vec::new();
        if self.peek().kind == RPAREN {
            return Ok(params);
        }
        loop {
            let name = self.expect_token(IDENT)?.start;
            self.expect_token(COLON)?;
            let ty = self.parse_type()?;
            params.push((name, ty));
            if self.peek().kind == COMMA {
                self.advance();
            } else {
                break;
            }
        }
        Ok(params)
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        // `&mut &mut ... T` right-recurses here; guard it on the shared counter.
        self.depth += 1;
        let r = if self.depth > MAX_PARSE_DEPTH {
            Err(ParseError {
                message: "type nesting too deep".into(),
                token_index: self.pos,
            })
        } else {
            self.parse_type_inner()
        };
        self.depth -= 1;
        r
    }

    fn parse_type_inner(&mut self) -> Result<Type, ParseError> {
        match self.peek().kind {
            KW_I32 => {
                self.advance();
                Ok(Type::I32)
            }
            KW_BOOL => {
                self.advance();
                Ok(Type::Bool)
            }
            AMP | AMP_MUT => {
                let mutable = self.peek().kind == AMP_MUT;
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Ref {
                    mutable,
                    inner: Box::new(inner),
                })
            }
            _ => Err(ParseError {
                message: "expected type".into(),
                token_index: self.pos,
            }),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        // `parse_block` is the single convergence point for ALL block nesting:
        // `while`/`loop` bodies, `if`/`else` arms, the fn body, and bare block
        // expressions. The `while` body in particular is reached by a direct
        // `parse_block` call (the cond's `parse_expr` has already decremented),
        // so without guarding here, `while c { while c { ... } }` recurses
        // unbounded and overflows the native stack. Guard at the block so every
        // nesting construct — present and future — fails closed.
        self.depth += 1;
        let r = if self.depth > MAX_PARSE_DEPTH {
            Err(ParseError {
                message: "block nesting too deep".into(),
                token_index: self.pos,
            })
        } else {
            self.parse_block_inner()
        };
        self.depth -= 1;
        r
    }

    fn parse_block_inner(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect_token(LBRACE)?;
        let mut stmts = Vec::new();
        while self.peek().kind != RBRACE && self.peek().kind != EOF {
            stmts.push(self.parse_stmt()?);
        }
        self.expect_token(RBRACE)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek().kind {
            KW_LET => self.parse_let(),
            KW_RETURN => self.parse_return(),
            KW_WHILE => {
                self.advance();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::While { cond, body })
            }
            KW_FOR => self.parse_for(),
            _ => {
                let expr = self.parse_expr()?;
                // `name = value;` is an assignment to an existing binding.
                if let Expr::Var(name) = expr {
                    if self.peek().kind == ASSIGN {
                        self.advance();
                        let value = self.parse_expr()?;
                        self.expect_token(SEMI)?;
                        return Ok(Stmt::Assign { name, value });
                    }
                    // Compound assignment `name += e` / `name -= e` desugars to
                    // `name = name <op> e`, mirroring rustc's i32 semantics with
                    // no new AST/IR surface. The synthetic `Var(name)` read
                    // reuses the target offset; this is sound for the i32-only
                    // subset because `+=`/`-=` never operate on references, so
                    // the read can never register a borrow loan.
                    if matches!(self.peek().kind, PLUS_EQ | MINUS_EQ) {
                        let op = if self.advance().kind == PLUS_EQ {
                            PLUS
                        } else {
                            MINUS
                        };
                        let rhs = self.parse_expr()?;
                        self.expect_token(SEMI)?;
                        let value = Expr::Binary {
                            op,
                            lhs: Box::new(Expr::Var(name)),
                            rhs: Box::new(rhs),
                        };
                        return Ok(Stmt::Assign { name, value });
                    }
                }
                // Block-like expression statements (`if`/`else`, `{ ... }`) are
                // valid without a trailing semicolon, matching Rust; any other
                // expression statement still requires one.
                if matches!(expr, Expr::If { .. } | Expr::Block(_)) {
                    if self.peek().kind == SEMI {
                        self.advance();
                    }
                } else {
                    self.expect_token(SEMI)?;
                }
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, ParseError> {
        self.expect_token(KW_LET)?;
        let mutable = if self.peek().kind == KW_MUT {
            self.advance();
            true
        } else {
            false
        };
        let name = self.expect_token(IDENT)?.start;
        self.expect_token(COLON)?;
        let ty = self.parse_type()?;
        self.expect_token(ASSIGN)?;
        let init = self.parse_expr()?;
        self.expect_token(SEMI)?;
        Ok(Stmt::Let {
            mutable,
            name,
            ty,
            init,
        })
    }

    fn parse_for(&mut self) -> Result<Stmt, ParseError> {
        self.expect_token(KW_FOR)?;
        let name = self.expect_token(IDENT)?.start;
        self.expect_token(KW_IN)?;
        let start = self.parse_expr()?;
        self.expect_token(DOTDOT)?;
        let end = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::For {
            name,
            start,
            end,
            body,
        })
    }

    fn parse_return(&mut self) -> Result<Stmt, ParseError> {
        self.expect_token(KW_RETURN)?;
        let expr = if self.peek().kind != SEMI {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_token(SEMI)?;
        Ok(Stmt::Return(expr))
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        // Depth-guard the single transitive recursion point for all
        // paren/call/if/block/while nesting; fail closed before the native
        // stack overflows on hostile input.
        self.depth += 1;
        let r = if self.depth > MAX_PARSE_DEPTH {
            Err(ParseError {
                message: "expression nesting too deep".into(),
                token_index: self.pos,
            })
        } else {
            self.parse_or()
        };
        self.depth -= 1;
        r
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and()?;
        while self.peek().kind == OROR {
            let op = self.advance().kind;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(self.parse_and()?),
            };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_cmp()?;
        while self.peek().kind == ANDAND {
            let op = self.advance().kind;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(self.parse_cmp()?),
            };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_term()?;
        while matches!(self.peek().kind, EQ | LT | NE | GT | LE | GE) {
            let op = self.advance().kind;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(self.parse_term()?),
            };
        }
        Ok(lhs)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_factor()?;
        while matches!(self.peek().kind, PLUS | MINUS) {
            let op = self.advance().kind;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(self.parse_factor()?),
            };
        }
        Ok(lhs)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        while matches!(self.peek().kind, STAR | SLASH | PERCENT) {
            let op = self.advance().kind;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(self.parse_unary()?),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        // `* ! &` chains right-recurse here without going through parse_expr,
        // so this self-recursion needs its own depth guard (shared counter).
        self.depth += 1;
        let r = if self.depth > MAX_PARSE_DEPTH {
            Err(ParseError {
                message: "expression nesting too deep".into(),
                token_index: self.pos,
            })
        } else {
            self.parse_unary_inner()
        };
        self.depth -= 1;
        r
    }

    fn parse_unary_inner(&mut self) -> Result<Expr, ParseError> {
        match self.peek().kind {
            AMP | AMP_MUT => {
                let mutable = self.peek().kind == AMP_MUT;
                self.advance();
                Ok(Expr::Borrow {
                    mutable,
                    expr: Box::new(self.parse_unary()?),
                })
            }
            STAR => {
                self.advance();
                Ok(Expr::Deref(Box::new(self.parse_unary()?)))
            }
            BANG => {
                self.advance();
                Ok(Expr::Not(Box::new(self.parse_unary()?)))
            }
            MINUS => {
                self.advance();
                Ok(Expr::Neg(Box::new(self.parse_unary()?)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().kind {
            LPAREN => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect_token(RPAREN)?;
                Ok(inner)
            }
            LITERAL_INT => {
                let tok = *self.advance();
                // rustc treats a literal exceeding u128 as an unconditional hard
                // error ("integer literal is too large"), which `--cap-lints
                // allow` cannot suppress; literals within u128 are merely the
                // capped `overflowing_literals` lint (accepted, then wrapped to
                // the target type). Match that boundary exactly: parse as u128,
                // reject on overflow. Storing the low 64 bits is value-faithful
                // for the i32-only subset because `v as u64 as i32 == v as i32`.
                let text = tok.try_text(self.source).map_err(|offset| ParseError {
                    message: format!("invalid token text span at byte {offset}"),
                    token_index: self.pos,
                })?;
                match text.parse::<u128>() {
                    Ok(v) => Ok(Expr::LiteralInt(tok.start, v as u64)),
                    Err(_) => Err(ParseError {
                        message: "integer literal is too large".into(),
                        token_index: self.pos,
                    }),
                }
            }
            LITERAL_BOOL => {
                let tok = *self.advance();
                let b = tok.try_text(self.source).map_err(|offset| ParseError {
                    message: format!("invalid token text span at byte {offset}"),
                    token_index: self.pos,
                })? == "true";
                Ok(Expr::LiteralBool(tok.start, b))
            }
            IDENT => {
                let name = self.advance().start;
                if self.peek().kind == LPAREN {
                    self.advance();
                    let mut args = Vec::new();
                    if self.peek().kind != RPAREN {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.peek().kind == COMMA {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect_token(RPAREN)?;
                    Ok(Expr::Call { name, args })
                } else {
                    Ok(Expr::Var(name))
                }
            }
            LBRACE => Ok(Expr::Block(self.parse_block()?)),
            KW_IF => {
                self.advance();
                let cond = Box::new(self.parse_expr()?);
                let then_block = Box::new(Expr::Block(self.parse_block()?));
                let else_block = if self.peek().kind == KW_ELSE {
                    self.advance();
                    if self.peek().kind == KW_IF {
                        Some(Box::new(self.parse_expr()?))
                    } else {
                        Some(Box::new(Expr::Block(self.parse_block()?)))
                    }
                } else {
                    None
                };
                Ok(Expr::If {
                    cond,
                    then_block,
                    else_block,
                })
            }
            _ => Err(ParseError {
                message: "unexpected token in expression".into(),
                token_index: self.pos,
            }),
        }
    }
}
