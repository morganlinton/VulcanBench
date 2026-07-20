use crate::ir::{Ident, Node};
use im::HashSet;

pub(crate) struct LiveResult {
    pub(crate) nodes: Vec<Node>,
    pub(crate) live_in: HashSet<Ident>,
}
