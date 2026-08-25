use vyre_foundation::ir::{Expr, Node};
use vyre_foundation::MemoryOrdering;

pub(crate) const fn ceil_div_u32(value: u32, divisor: u32) -> u32 {
    let full = value / divisor;
    let tail = if value % divisor == 0 { 0 } else { 1 };
    full + tail
}

pub(crate) fn single_word_lineage_body(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    cells: u32,
    max_iterations: u32,
    lanes: u32,
) -> Vec<Node> {
    let lane = Expr::InvocationId { axis: 0 };
    let cell_chunks = ceil_div_u32(cells, lanes).max(1);
    let cell = Expr::add(
        Expr::mul(Expr::var("__sj_chunk"), Expr::u32(lanes)),
        lane.clone(),
    );
    let transfer_body = single_word_transfer_body(state, next, join_rules, n, cells, cell.clone());
    let compare_body = single_word_compare_body(state, next, changed, cells, cell);

    vec![Node::loop_for(
        "__sj_iter",
        Expr::u32(0),
        Expr::u32(max_iterations),
        vec![
            Node::if_then(
                Expr::eq(lane.clone(), Expr::u32(0)),
                vec![Node::store(changed, Expr::u32(0), Expr::u32(0))],
            ),
            workgroup_barrier(),
            Node::loop_for(
                "__sj_chunk",
                Expr::u32(0),
                Expr::u32(cell_chunks),
                transfer_body,
            ),
            workgroup_barrier(),
            Node::loop_for(
                "__sj_chunk",
                Expr::u32(0),
                Expr::u32(cell_chunks),
                compare_body,
            ),
            workgroup_barrier(),
            Node::if_then(
                Expr::eq(Expr::load(changed, Expr::u32(0)), Expr::u32(0)),
                vec![Node::Return],
            ),
        ],
    )]
}

pub(crate) fn single_word_lineage_grid_sync_body(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    cells: u32,
    max_iterations: u32,
) -> Vec<Node> {
    let lane = Expr::InvocationId { axis: 0 };
    let mut body = Vec::new();
    for iter in 0..max_iterations {
        body.push(Node::if_then(
            Expr::eq(lane.clone(), Expr::u32(0)),
            vec![Node::store(changed, Expr::u32(0), Expr::u32(0))],
        ));
        // Each phase body opens with its own `let __sj_cell`. The
        // workgroup path (single_word_lineage_body) isolates them in
        // separate `__sj_chunk` loops; the grid-sync path has no such
        // loop, so flattening both phases into this region would make
        // the two `let __sj_cell` declarations siblings and trip V032.
        // Wrap each phase in its own Block scope. The grid-sync barrier
        // stays at region level between them so it still synchronizes
        // the whole grid between the transfer and compare phases.
        body.push(Node::Block(single_word_transfer_body(
            state,
            next,
            join_rules,
            n,
            cells,
            lane.clone(),
        )));
        body.push(grid_sync_barrier());
        body.push(Node::Block(single_word_compare_body(
            state,
            next,
            changed,
            cells,
            lane.clone(),
        )));
        if iter + 1 < max_iterations {
            body.push(grid_sync_barrier());
        }
    }
    body
}

fn single_word_transfer_body(
    state: &str,
    next: &str,
    join_rules: &str,
    n: u32,
    cells: u32,
    cell: Expr,
) -> Vec<Node> {
    let transfer_cell = vec![
        Node::let_bind("__sj_i", Expr::div(Expr::var("__sj_cell"), Expr::u32(n))),
        Node::let_bind("__sj_j", Expr::rem(Expr::var("__sj_cell"), Expr::u32(n))),
        Node::let_bind("__sj_acc", Expr::u32(0)),
        Node::loop_for(
            "__sj_kk",
            Expr::u32(0),
            Expr::u32(n),
            vec![
                Node::let_bind(
                    "__sj_a",
                    Expr::load(
                        state,
                        Expr::add(
                            Expr::mul(Expr::var("__sj_i"), Expr::u32(n)),
                            Expr::var("__sj_kk"),
                        ),
                    ),
                ),
                Node::let_bind(
                    "__sj_b",
                    Expr::load(
                        join_rules,
                        Expr::add(
                            Expr::mul(Expr::var("__sj_kk"), Expr::u32(n)),
                            Expr::var("__sj_j"),
                        ),
                    ),
                ),
                Node::let_bind(
                    "__sj_combined",
                    Expr::select(
                        Expr::or(
                            Expr::eq(Expr::var("__sj_a"), Expr::u32(0)),
                            Expr::eq(Expr::var("__sj_b"), Expr::u32(0)),
                        ),
                        Expr::u32(0),
                        Expr::bitor(Expr::var("__sj_a"), Expr::var("__sj_b")),
                    ),
                ),
                Node::assign(
                    "__sj_acc",
                    Expr::bitor(Expr::var("__sj_acc"), Expr::var("__sj_combined")),
                ),
            ],
        ),
        Node::let_bind("__sj_seed", Expr::load(state, Expr::var("__sj_cell"))),
        Node::store(
            next,
            Expr::var("__sj_cell"),
            Expr::bitor(Expr::var("__sj_seed"), Expr::var("__sj_acc")),
        ),
    ];
    vec![
        Node::let_bind("__sj_cell", cell),
        Node::if_then(
            Expr::lt(Expr::var("__sj_cell"), Expr::u32(cells)),
            transfer_cell,
        ),
    ]
}

fn single_word_compare_body(
    state: &str,
    next: &str,
    changed: &str,
    cells: u32,
    cell: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind("__sj_cell", cell),
        Node::if_then(
            Expr::lt(Expr::var("__sj_cell"), Expr::u32(cells)),
            vec![
                Node::let_bind("__sj_current", Expr::load(state, Expr::var("__sj_cell"))),
                Node::let_bind("__sj_next", Expr::load(next, Expr::var("__sj_cell"))),
                Node::if_then(
                    Expr::ne(Expr::var("__sj_current"), Expr::var("__sj_next")),
                    vec![Node::let_bind(
                        "__sj_changed",
                        Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
                    )],
                ),
                Node::store(state, Expr::var("__sj_cell"), Expr::var("__sj_next")),
            ],
        ),
    ]
}

pub(crate) fn wide_lineage_body(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    w: u32,
    cells: u32,
    max_iterations: u32,
    lanes: u32,
) -> Vec<Node> {
    let lane = Expr::InvocationId { axis: 0 };
    let cell_chunks = ceil_div_u32(cells, lanes).max(1);
    let cell = Expr::add(
        Expr::mul(Expr::var("__sjw_chunk"), Expr::u32(lanes)),
        lane.clone(),
    );
    let transfer_body = wide_transfer_body(state, next, join_rules, n, w, cells, cell.clone());
    let compare_body = wide_compare_body(state, next, changed, w, cells, cell);

    vec![Node::loop_for(
        "__sjw_iter",
        Expr::u32(0),
        Expr::u32(max_iterations),
        vec![
            Node::if_then(
                Expr::eq(lane.clone(), Expr::u32(0)),
                vec![Node::store(changed, Expr::u32(0), Expr::u32(0))],
            ),
            workgroup_barrier(),
            Node::loop_for(
                "__sjw_chunk",
                Expr::u32(0),
                Expr::u32(cell_chunks),
                transfer_body,
            ),
            workgroup_barrier(),
            Node::loop_for(
                "__sjw_chunk",
                Expr::u32(0),
                Expr::u32(cell_chunks),
                compare_body,
            ),
            workgroup_barrier(),
            Node::if_then(
                Expr::eq(Expr::load(changed, Expr::u32(0)), Expr::u32(0)),
                vec![Node::Return],
            ),
        ],
    )]
}

fn wide_transfer_body(
    state: &str,
    next: &str,
    join_rules: &str,
    n: u32,
    w: u32,
    cells: u32,
    cell: Expr,
) -> Vec<Node> {
    let mut transfer_cell = vec![
        Node::let_bind("__sjw_i", Expr::div(Expr::var("__sjw_cell"), Expr::u32(n))),
        Node::let_bind("__sjw_j", Expr::rem(Expr::var("__sjw_cell"), Expr::u32(n))),
        Node::let_bind(
            "__sjw_cell_base",
            Expr::mul(Expr::var("__sjw_cell"), Expr::u32(w)),
        ),
    ];
    for word_idx in 0..w {
        transfer_cell.push(Node::let_bind(
            format!("__sjw_acc_{word_idx}"),
            Expr::load(
                state,
                Expr::add(Expr::var("__sjw_cell_base"), Expr::u32(word_idx)),
            ),
        ));
    }

    let mut kk_body = Vec::new();
    let mut a_is_zero = Expr::bool(true);
    let mut b_is_zero = Expr::bool(true);
    for word_idx in 0..w {
        let a_name = format!("__sjw_a_{word_idx}");
        let b_name = format!("__sjw_b_{word_idx}");
        kk_body.push(Node::let_bind(
            a_name.clone(),
            Expr::load(
                state,
                Expr::add(
                    Expr::mul(
                        Expr::add(
                            Expr::mul(Expr::var("__sjw_i"), Expr::u32(n)),
                            Expr::var("__sjw_kk"),
                        ),
                        Expr::u32(w),
                    ),
                    Expr::u32(word_idx),
                ),
            ),
        ));
        kk_body.push(Node::let_bind(
            b_name.clone(),
            Expr::load(
                join_rules,
                Expr::add(
                    Expr::mul(
                        Expr::add(
                            Expr::mul(Expr::var("__sjw_kk"), Expr::u32(n)),
                            Expr::var("__sjw_j"),
                        ),
                        Expr::u32(w),
                    ),
                    Expr::u32(word_idx),
                ),
            ),
        ));
        a_is_zero = Expr::and(a_is_zero, Expr::eq(Expr::var(a_name), Expr::u32(0)));
        b_is_zero = Expr::and(b_is_zero, Expr::eq(Expr::var(b_name), Expr::u32(0)));
    }
    let either_zero = Expr::or(a_is_zero, b_is_zero);
    for word_idx in 0..w {
        kk_body.push(Node::let_bind(
            format!("__sjw_combined_{word_idx}"),
            Expr::select(
                either_zero.clone(),
                Expr::u32(0),
                Expr::bitor(
                    Expr::var(format!("__sjw_a_{word_idx}")),
                    Expr::var(format!("__sjw_b_{word_idx}")),
                ),
            ),
        ));
        kk_body.push(Node::assign(
            format!("__sjw_acc_{word_idx}"),
            Expr::bitor(
                Expr::var(format!("__sjw_acc_{word_idx}")),
                Expr::var(format!("__sjw_combined_{word_idx}")),
            ),
        ));
    }
    transfer_cell.push(Node::loop_for(
        "__sjw_kk",
        Expr::u32(0),
        Expr::u32(n),
        kk_body,
    ));
    for word_idx in 0..w {
        transfer_cell.push(Node::store(
            next,
            Expr::add(Expr::var("__sjw_cell_base"), Expr::u32(word_idx)),
            Expr::var(format!("__sjw_acc_{word_idx}")),
        ));
    }

    vec![
        Node::let_bind("__sjw_cell", cell),
        Node::if_then(
            Expr::lt(Expr::var("__sjw_cell"), Expr::u32(cells)),
            transfer_cell,
        ),
    ]
}

fn wide_compare_body(
    state: &str,
    next: &str,
    changed: &str,
    w: u32,
    cells: u32,
    cell: Expr,
) -> Vec<Node> {
    let mut compare_cell = vec![Node::let_bind(
        "__sjw_cell_base",
        Expr::mul(Expr::var("__sjw_cell"), Expr::u32(w)),
    )];
    for word_idx in 0..w {
        let word_name = format!("__sjw_word_{word_idx}");
        let current_name = format!("__sjw_current_{word_idx}");
        let next_name = format!("__sjw_next_{word_idx}");
        let changed_name = format!("__sjw_changed_{word_idx}");
        compare_cell.extend([
            Node::let_bind(
                word_name.clone(),
                Expr::add(Expr::var("__sjw_cell_base"), Expr::u32(word_idx)),
            ),
            Node::let_bind(
                current_name.clone(),
                Expr::load(state, Expr::var(word_name.clone())),
            ),
            Node::let_bind(
                next_name.clone(),
                Expr::load(next, Expr::var(word_name.clone())),
            ),
            Node::if_then(
                Expr::ne(Expr::var(current_name), Expr::var(next_name.clone())),
                vec![Node::let_bind(
                    changed_name,
                    Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
                )],
            ),
            Node::store(state, Expr::var(word_name), Expr::var(next_name)),
        ]);
    }
    vec![
        Node::let_bind("__sjw_cell", cell),
        Node::if_then(
            Expr::lt(Expr::var("__sjw_cell"), Expr::u32(cells)),
            compare_cell,
        ),
    ]
}

pub(crate) fn wide_lineage_grid_sync_body(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    w: u32,
    cells: u32,
    max_iterations: u32,
) -> Vec<Node> {
    let lane = Expr::InvocationId { axis: 0 };
    let mut body = Vec::new();
    for iter in 0..max_iterations {
        body.push(Node::if_then(
            Expr::eq(lane.clone(), Expr::u32(0)),
            vec![Node::store(changed, Expr::u32(0), Expr::u32(0))],
        ));
        // See single_word_lineage_grid_sync_body: each phase opens with
        // its own `let __sjw_cell`; wrap each in a Block so the two are
        // not V032-colliding siblings, keeping the grid-sync barrier at
        // region level between the transfer and compare phases.
        body.push(Node::Block(wide_transfer_body(
            state,
            next,
            join_rules,
            n,
            w,
            cells,
            lane.clone(),
        )));
        body.push(grid_sync_barrier());
        body.push(Node::Block(wide_compare_body(
            state,
            next,
            changed,
            w,
            cells,
            lane.clone(),
        )));
        if iter + 1 < max_iterations {
            body.push(grid_sync_barrier());
        }
    }
    body
}

fn workgroup_barrier() -> Node {
    Node::Barrier {
        ordering: MemoryOrdering::SeqCst,
    }
}

fn grid_sync_barrier() -> Node {
    Node::Barrier {
        ordering: MemoryOrdering::GridSync,
    }
}
