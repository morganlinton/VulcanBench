use super::*;
pub(super) fn push_stack(name: &str, sp: &str, value: Expr) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(STACK_DEPTH as usize);
    for slot in 0..STACK_DEPTH {
        nodes.push(Node::if_then(
            Expr::eq(Expr::var(sp), Expr::u32(slot)),
            vec![Node::assign(&format!("{name}_{slot}"), value.clone())],
        ));
    }
    // Bump SP. If full, leave SP at STACK_DEPTH so subsequent pushes
    // also no-op; the final state still produces a value the caller
    // can safely emit (0 / failsafe).
    nodes.push(Node::if_then(
        Expr::lt(Expr::var(sp), Expr::u32(STACK_DEPTH)),
        vec![Node::assign(sp, Expr::add(Expr::var(sp), Expr::u32(1)))],
    ));
    nodes
}

pub(super) fn pop_stack(name: &str, sp: &str, out_var: &str) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(STACK_DEPTH as usize + 2);
    // Decrement SP first (saturating at 0).
    nodes.push(Node::if_then(
        Expr::gt(Expr::var(sp), Expr::u32(0)),
        vec![Node::assign(sp, Expr::sub(Expr::var(sp), Expr::u32(1)))],
    ));
    for slot in 0..STACK_DEPTH {
        nodes.push(Node::if_then(
            Expr::eq(Expr::var(sp), Expr::u32(slot)),
            vec![Node::assign(out_var, Expr::var(&format!("{name}_{slot}")))],
        ));
    }
    nodes
}

pub(super) fn peek_stack(name: &str, sp: &str, out_var: &str) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(STACK_DEPTH as usize);
    for slot in 0..STACK_DEPTH {
        nodes.push(Node::if_then(
            Expr::and(
                Expr::gt(Expr::var(sp), Expr::u32(0)),
                Expr::eq(Expr::var(sp), Expr::u32(slot + 1)),
            ),
            vec![Node::assign(out_var, Expr::var(&format!("{name}_{slot}")))],
        ));
    }
    nodes
}
