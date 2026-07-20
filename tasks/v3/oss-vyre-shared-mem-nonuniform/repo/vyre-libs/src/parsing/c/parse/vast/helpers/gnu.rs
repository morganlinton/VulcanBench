use super::super::*;
use super::tokens::balanced_or;
use crate::parsing::c::lex::tokens::*;
use vyre::ir::Expr;

pub(crate) fn is_gnu_typeof_symbol_hash(symbol_hash: Expr) -> Expr {
    balanced_or(
        C_GNU_TYPEOF_HASHES
            .iter()
            .copied()
            .map(|hash| Expr::eq(symbol_hash.clone(), Expr::u32(hash)))
            .collect(),
    )
}

pub(crate) fn is_typeof_operator_token(token: Expr, symbol_hash: Expr) -> Expr {
    Expr::or(
        Expr::or(
            Expr::eq(token.clone(), Expr::u32(TOK_GNU_TYPEOF)),
            Expr::eq(token.clone(), Expr::u32(TOK_GNU_TYPEOF_UNQUAL)),
        ),
        Expr::and(
            Expr::eq(token, Expr::u32(TOK_IDENTIFIER)),
            is_gnu_typeof_symbol_hash(symbol_hash),
        ),
    )
}

pub(crate) fn is_gnu_auto_type_symbol_hash(symbol_hash: Expr) -> Expr {
    Expr::eq(symbol_hash, Expr::u32(C_GNU_AUTO_TYPE_HASH))
}

pub(crate) fn c_attribute_kind_from_hash(symbol_hash: Expr) -> Expr {
    balanced_attribute_kind_from_hash(&symbol_hash, C_ATTRIBUTE_KIND_HASHES)
}

fn balanced_attribute_kind_from_hash(symbol_hash: &Expr, entries: &[(u32, u32)]) -> Expr {
    match entries.len() {
        0 => Expr::u32(0),
        1 => {
            let (hash, kind) = entries[0];
            Expr::select(
                Expr::eq(symbol_hash.clone(), Expr::u32(hash)),
                Expr::u32(kind),
                Expr::u32(0),
            )
        }
        _ => {
            let (left, right) = entries.split_at(entries.len() / 2);
            let left_match = balanced_or(
                left.iter()
                    .map(|(hash, _)| Expr::eq(symbol_hash.clone(), Expr::u32(*hash)))
                    .collect(),
            );
            Expr::select(
                left_match,
                balanced_attribute_kind_from_hash(symbol_hash, left),
                balanced_attribute_kind_from_hash(symbol_hash, right),
            )
        }
    }
}
