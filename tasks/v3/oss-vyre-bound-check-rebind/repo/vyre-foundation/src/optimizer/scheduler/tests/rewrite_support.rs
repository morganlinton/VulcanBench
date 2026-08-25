use super::*;

pub(super) fn rewrite_first_store_value(nodes: &mut [Node]) -> bool {
    for node in nodes {
        match node {
            Node::Store { value, .. } => {
                *value = Expr::u32(43);
                return true;
            }
            Node::If {
                then, otherwise, ..
            } => {
                if rewrite_first_store_value(then) || rewrite_first_store_value(otherwise) {
                    return true;
                }
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                if rewrite_first_store_value(body) {
                    return true;
                }
            }
            Node::Region { body, .. } => {
                let body_vec: &mut Vec<Node> = Arc::make_mut(body);
                if rewrite_first_store_value(body_vec.as_mut_slice()) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

pub(super) fn rewrite_matching_stores(program: Program, batch: Option<&RewriteBatch>) -> PassResult {
    let mut entry = Clone::clone(&program).into_entry_vec();
    let mut changed = false;
    match batch {
        Some(batch) => {
            let selected = batch
                .items()
                .iter()
                .map(|item| item.col as usize)
                .collect::<Vec<_>>();
            let mut ordinal = 0usize;
            changed |= rewrite_selected_store_ordinals(&mut entry, &selected, &mut ordinal);
        }
        None => {
            changed |= rewrite_all_matching_stores(&mut entry);
        }
    }
    if changed {
        PassResult {
            program: program.with_rewritten_entry(entry),
            changed: true,
        }
    } else {
        PassResult::unchanged(program)
    }
}

pub(super) fn rewrite_store_value_if_matches(node: &mut Node, old: u32, new: u32) -> bool {
    match node {
        Node::Store { value, .. } if *value == Expr::u32(old) => {
            *value = Expr::u32(new);
            true
        }
        _ => false,
    }
}

pub(super) fn rewrite_store_values(nodes: &mut [Node], old: u32, new: u32) -> bool {
    let mut changed = false;
    for node in nodes {
        changed |= match node {
            Node::Store { .. } => rewrite_store_value_if_matches(node, old, new),
            Node::If {
                then, otherwise, ..
            } => rewrite_store_values(then, old, new) | rewrite_store_values(otherwise, old, new),
            Node::Loop { body, .. } | Node::Block(body) => rewrite_store_values(body, old, new),
            Node::Region { body, .. } => {
                let body_vec: &mut Vec<Node> = Arc::make_mut(body);
                rewrite_store_values(body_vec.as_mut_slice(), old, new)
            }
            _ => false,
        };
    }
    changed
}

pub(super) fn store_value_is(node: &Node, expected: u32) -> bool {
    matches!(node, Node::Store { value, .. } if *value == Expr::u32(expected))
}

pub(super) fn all_stores_have_value(nodes: &[Node], expected: u32) -> bool {
    nodes.iter().all(|node| match node {
        Node::Store { .. } => store_value_is(node, expected),
        Node::If {
            then, otherwise, ..
        } => all_stores_have_value(then, expected) && all_stores_have_value(otherwise, expected),
        Node::Loop { body, .. } | Node::Block(body) => all_stores_have_value(body, expected),
        Node::Region { body, .. } => all_stores_have_value(body, expected),
        _ => true,
    })
}

pub(super) fn collect_store_candidates(nodes: &[Node], candidates: &mut Vec<RewriteCandidate>) {
    for node in nodes {
        match node {
            Node::Store { value, .. } if *value == Expr::u32(42) => {
                candidates.push(RewriteCandidate::new(0, candidates.len() as u32));
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_store_candidates(then, candidates);
                collect_store_candidates(otherwise, candidates);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                collect_store_candidates(body, candidates);
            }
            Node::Region { body, .. } => {
                collect_store_candidates(body, candidates);
            }
            _ => {}
        }
    }
}

pub(super) fn rewrite_all_matching_stores(nodes: &mut [Node]) -> bool {
    let mut changed = false;
    for node in nodes {
        changed |= match node {
            Node::Store { .. } => rewrite_store_value_if_matches(node, 42, 43),
            Node::If {
                then, otherwise, ..
            } => rewrite_all_matching_stores(then) | rewrite_all_matching_stores(otherwise),
            Node::Loop { body, .. } | Node::Block(body) => rewrite_all_matching_stores(body),
            Node::Region { body, .. } => {
                let body_vec: &mut Vec<Node> = Arc::make_mut(body);
                rewrite_all_matching_stores(body_vec.as_mut_slice())
            }
            _ => false,
        };
    }
    changed
}

pub(super) fn rewrite_selected_store_ordinals(
    nodes: &mut [Node],
    selected: &[usize],
    ordinal: &mut usize,
) -> bool {
    let mut changed = false;
    for node in nodes {
        changed |= match node {
            Node::Store { value, .. } => {
                let current = *ordinal;
                *ordinal += 1;
                if *value == Expr::u32(42) && selected.contains(&current) {
                    rewrite_store_value_if_matches(node, 42, 43)
                } else {
                    false
                }
            }
            Node::If {
                then, otherwise, ..
            } => {
                rewrite_selected_store_ordinals(then, selected, ordinal)
                    | rewrite_selected_store_ordinals(otherwise, selected, ordinal)
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                rewrite_selected_store_ordinals(body, selected, ordinal)
            }
            Node::Region { body, .. } => {
                let body_vec: &mut Vec<Node> = Arc::make_mut(body);
                rewrite_selected_store_ordinals(body_vec.as_mut_slice(), selected, ordinal)
            }
            _ => false,
        };
    }
    changed
}

pub(super) fn repeated_store_program(count: usize) -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(count as u32)],
        [1, 1, 1],
        (0..count)
            .map(|index| Node::store("out", Expr::u32(index as u32), Expr::u32(42)))
            .collect::<Vec<_>>(),
    )
}
