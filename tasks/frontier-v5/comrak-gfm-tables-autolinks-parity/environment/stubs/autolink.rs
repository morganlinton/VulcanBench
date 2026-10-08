//! GitHub Flavored Markdown extended autolinks (www., URL and email).
//!
//! Not yet ported: this target must produce exactly the output of the
//! reference implementation for extended autolinks. Until then none is
//! recognised.

use typed_arena::Arena;

use crate::nodes::{AstNode, Node, Sourcepos};
use crate::parser::inlines::Subject;
use crate::parser::Spx;

/// Turn email addresses inside a text node into links.
pub(crate) fn process_email_autolinks<'a>(
    _arena: &'a Arena<AstNode<'a>>,
    _node: Node<'a>,
    _contents: &mut String,
    _relaxed_autolinks: bool,
    _sourcepos: &mut Sourcepos,
    _spx: &mut Spx,
) {
}

/// Match a `www.` autolink at the subject's position.
pub fn www_match<'a>(
    _subject: &mut Subject<'a, '_, '_, '_, '_, '_>,
) -> Option<(Node<'a>, usize, usize)> {
    None
}

/// Match a scheme (`http://`, `https://`, `ftp://`) autolink at the subject's position.
pub fn url_match<'a>(
    _subject: &mut Subject<'a, '_, '_, '_, '_, '_>,
) -> Option<(Node<'a>, usize, usize)> {
    None
}
