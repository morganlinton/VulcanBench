// CPU reference oracles  (#[cfg(test)] / feature = "cpu-parity")
// ------------------------------------------------------------------

use super::alloc_helpers::{push_dominator_vec, resize_dominator_vec};

/// Lengauer–Tarjan exact immediate dominators.
///
/// Returns `idom[v]` for every node `v`.  `idom[entry] == entry`.
/// Unreachable nodes receive `None`.
///
/// CPU-only reference algorithm. Gated with the rest of the CPU oracle
/// surface (`compress`, `eval`, `link`, `cpu_ref`) so default builds
/// don't pull the implementation through. (Without this gate, default
/// builds left the body of `lengauer_tarjan_idoms` referencing the
/// gated-out `eval`/`link`/`compress` helpers and failed with three
/// E0423/E0425 errors. Reproduced 2026-05-23.)
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn lengauer_tarjan_idoms(
    node_count: u32,
    entry: u32,
    edges: &[(u32, u32)],
) -> Vec<Option<u32>> {
    try_lengauer_tarjan_idoms(node_count, entry, edges).unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible Lengauer-Tarjan exact immediate dominators.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_lengauer_tarjan_idoms(
    node_count: u32,
    entry: u32,
    edges: &[(u32, u32)],
) -> Result<Vec<Option<u32>>, String> {
    let mut idom = Vec::new();
    let mut scratch = DominatorTreeCpuScratch::default();
    try_lengauer_tarjan_idoms_into(node_count, entry, edges, &mut idom, &mut scratch)?;
    Ok(idom)
}

/// Reusable workspace for dominator-tree CPU oracles.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Debug, Default)]
pub struct DominatorTreeCpuScratch {
    succ: Vec<Vec<usize>>,
    pred: Vec<Vec<usize>>,
    semi: Vec<usize>,
    vertex: Vec<usize>,
    parent: Vec<usize>,
    dfs_stack: Vec<(usize, usize)>,
    ancestor: Vec<usize>,
    label: Vec<usize>,
    bucket: Vec<Vec<usize>>,
    compress_stack: Vec<usize>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl DominatorTreeCpuScratch {
    /// Construct empty dominator-tree CPU scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-reserve outer workspace vectors (unit tests for reuse invariants).
    #[cfg(test)]
    pub fn reserve_outer_for_test(&mut self, hint: usize, bucket_hint: usize) {
        self.succ.reserve(hint);
        self.pred.reserve(hint);
        self.semi.reserve(hint);
        self.vertex.reserve(hint.saturating_add(1));
        self.parent.reserve(hint);
        self.dfs_stack.reserve(hint);
        self.ancestor.reserve(hint);
        self.label.reserve(hint);
        self.bucket.reserve(bucket_hint);
        self.compress_stack.reserve(hint);
    }

    /// Snapshot outer-vector capacities (unit tests for reuse invariants).
    #[cfg(test)]
    #[must_use]
    pub fn outer_capacities(&self) -> [usize; 10] {
        [
            self.succ.capacity(),
            self.pred.capacity(),
            self.semi.capacity(),
            self.vertex.capacity(),
            self.parent.capacity(),
            self.dfs_stack.capacity(),
            self.ancestor.capacity(),
            self.label.capacity(),
            self.bucket.capacity(),
            self.compress_stack.capacity(),
        ]
    }

    /// Successor adjacency row (unit tests only).
    #[cfg(test)]
    #[must_use]
    pub fn test_succ_row(&self, node: usize) -> &[usize] {
        &self.succ[node]
    }

    /// Predecessor adjacency row (unit tests only).
    #[cfg(test)]
    #[must_use]
    pub fn test_pred_row(&self, node: usize) -> &[usize] {
        &self.pred[node]
    }

    /// Successor row capacity after reuse (unit tests only).
    #[cfg(test)]
    #[must_use]
    pub fn test_succ_row_capacity(&self, node: usize) -> usize {
        self.succ[node].capacity()
    }

    /// Predecessor row capacity after reuse (unit tests only).
    #[cfg(test)]
    #[must_use]
    pub fn test_pred_row_capacity(&self, node: usize) -> usize {
        self.pred[node].capacity()
    }
}

/// Fallible Lengauer-Tarjan exact immediate dominators using caller-owned output and scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_lengauer_tarjan_idoms_into(
    node_count: u32,
    entry: u32,
    edges: &[(u32, u32)],
    idom: &mut Vec<Option<u32>>,
    scratch: &mut DominatorTreeCpuScratch,
) -> Result<(), String> {
    let n = node_count as usize;
    let entry = entry as usize;
    if n == 0 {
        idom.clear();
        return Ok(());
    }
    if entry >= n {
        idom.clear();
        resize_dominator_vec(idom, n, None, "dominator_tree entry-out-of-range idoms")?;
        return Ok(());
    }

    // Build adjacency.
    resize_dominator_vec(
        &mut scratch.succ,
        n,
        Vec::new(),
        "dominator_tree successor rows",
    )?;
    resize_dominator_vec(
        &mut scratch.pred,
        n,
        Vec::new(),
        "dominator_tree predecessor rows",
    )?;
    for row in scratch.succ.iter_mut().take(n) {
        row.clear();
    }
    for row in scratch.pred.iter_mut().take(n) {
        row.clear();
    }
    for &(u, v) in edges {
        let u = u as usize;
        let v = v as usize;
        if u < n && v < n {
            push_dominator_vec(&mut scratch.succ[u], v, "dominator_tree successor row")?;
            push_dominator_vec(&mut scratch.pred[v], u, "dominator_tree predecessor row")?;
        }
    }

    // DFS numbering.
    scratch.semi.clear();
    scratch.vertex.clear();
    scratch.parent.clear();
    resize_dominator_vec(&mut scratch.semi, n, 0usize, "dominator_tree semi numbers")?;
    resize_dominator_vec(
        &mut scratch.vertex,
        n + 1,
        0usize,
        "dominator_tree DFS vertices",
    )?;
    resize_dominator_vec(&mut scratch.parent, n, 0usize, "dominator_tree DFS parents")?;
    let mut dfs_num: usize = 0;

    // Iterative DFS to avoid stack overflow on million-node chains.
    scratch.dfs_stack.clear();
    push_dominator_vec(
        &mut scratch.dfs_stack,
        (entry, 0usize),
        "dominator_tree DFS stack",
    )?;
    while let Some((v, next_idx)) = scratch.dfs_stack.last_mut() {
        let v = *v;
        if *next_idx == 0 {
            dfs_num += 1;
            scratch.semi[v] = dfs_num;
            scratch.vertex[dfs_num] = v;
        }
        if *next_idx < scratch.succ[v].len() {
            let w = scratch.succ[v][*next_idx];
            *next_idx += 1;
            if scratch.semi[w] == 0 {
                scratch.parent[w] = v;
                push_dominator_vec(&mut scratch.dfs_stack, (w, 0), "dominator_tree DFS stack")?;
            }
        } else {
            scratch.dfs_stack.pop();
        }
    }

    if dfs_num == 0 {
        idom.clear();
        resize_dominator_vec(idom, n, None, "dominator_tree unreachable idoms")?;
        return Ok(());
    }

    idom.clear();
    scratch.ancestor.clear();
    scratch.label.clear();
    scratch.bucket.clear();
    resize_dominator_vec(idom, n, None, "dominator_tree idoms")?;
    resize_dominator_vec(&mut scratch.ancestor, n, 0usize, "dominator_tree ancestors")?;
    crate::graph::scratch::reserve_graph_items(
        &mut scratch.label,
        n,
        "dominator tree CPU oracle",
        "dominator_tree labels",
    )?;
    scratch.label.extend(0..n);
    resize_dominator_vec(
        &mut scratch.bucket,
        n + 1,
        Vec::new(),
        "dominator_tree buckets",
    )?;
    for row in scratch.bucket.iter_mut().take(n + 1) {
        row.clear();
    }
    scratch.compress_stack.clear();

    for i in (1..=dfs_num).rev() {
        let w = scratch.vertex[i];

        for &v in &scratch.pred[w] {
            if scratch.semi[v] > 0 {
                let u = try_eval_with_stack(
                    v,
                    &mut scratch.ancestor,
                    &mut scratch.label,
                    &scratch.semi,
                    &mut scratch.compress_stack,
                )?;
                if scratch.semi[u] < scratch.semi[w] {
                    scratch.semi[w] = scratch.semi[u];
                }
            }
        }

        push_dominator_vec(
            &mut scratch.bucket[scratch.vertex[scratch.semi[w]]],
            w,
            "dominator_tree bucket row",
        )?;

        link(
            scratch.parent[w],
            w,
            &mut scratch.ancestor,
            &mut scratch.label,
            &scratch.semi,
        );

        for &v in &scratch.bucket[scratch.parent[w]] {
            let u = try_eval_with_stack(
                v,
                &mut scratch.ancestor,
                &mut scratch.label,
                &scratch.semi,
                &mut scratch.compress_stack,
            )?;
            if scratch.semi[u] < scratch.semi[v] {
                idom[v] = Some(u as u32);
            } else {
                idom[v] = Some(scratch.parent[w] as u32);
            }
        }
        scratch.bucket[scratch.parent[w]].clear();
    }

    for i in 2..=dfs_num {
        let w = scratch.vertex[i];
        if idom[w].map(|x| x as usize) != Some(scratch.vertex[scratch.semi[w]]) {
            idom[w] = idom[w]
                .and_then(|parent| idom.get(parent as usize))
                .copied()
                .flatten();
        }
    }

    idom[entry] = Some(entry as u32);

    // unreachable nodes keep None
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn try_compress(
    v: usize,
    ancestor: &mut [usize],
    label: &mut [usize],
    semi: &[usize],
) -> Result<(), String> {
    let mut stack = Vec::new();
    try_compress_with_stack(v, ancestor, label, semi, &mut stack)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn try_compress_with_stack(
    v: usize,
    ancestor: &mut [usize],
    label: &mut [usize],
    semi: &[usize],
    stack: &mut Vec<usize>,
) -> Result<(), String> {
    if ancestor[v] == 0 {
        return Ok(());
    }

    // Iterative version of the recursive path-compression used in LT.
    // We walk up the ancestor chain, pushing vertices that are at least
    // two levels above the root.  When we hit a direct child of the root
    // we process it in-place (label update, no splice) and then walk
    // back down the stack, processing and splicing as we go.
    stack.clear();
    let mut u = v;
    while ancestor[u] != 0 {
        if ancestor[ancestor[u]] != 0 {
            push_dominator_vec(stack, u, "dominator_tree compression stack")?;
            u = ancestor[u];
        } else {
            // Direct child of the root – "else" branch of the recursive
            // formulation.  Update label but do NOT splice ancestor.
            if semi[label[ancestor[u]]] < semi[label[u]] {
                label[u] = label[ancestor[u]];
            }
            break;
        }
    }

    // Walk back down, using the freshly-updated labels of ancestors.
    while let Some(w) = stack.pop() {
        if semi[label[ancestor[w]]] < semi[label[w]] {
            label[w] = label[ancestor[w]];
        }
        ancestor[w] = ancestor[ancestor[w]];
    }
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn try_eval(
    v: usize,
    ancestor: &mut [usize],
    label: &mut [usize],
    semi: &[usize],
) -> Result<usize, String> {
    let mut stack = Vec::new();
    try_eval_with_stack(v, ancestor, label, semi, &mut stack)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn try_eval_with_stack(
    v: usize,
    ancestor: &mut [usize],
    label: &mut [usize],
    semi: &[usize],
    stack: &mut Vec<usize>,
) -> Result<usize, String> {
    if ancestor[v] == 0 {
        Ok(v)
    } else {
        try_compress_with_stack(v, ancestor, label, semi, stack)?;
        Ok(label[v])
    }
}

#[cfg(any(test, feature = "cpu-parity"))]
fn link(v: usize, w: usize, ancestor: &mut [usize], label: &mut [usize], _semi: &[usize]) {
    ancestor[w] = v;
    label[w] = w;
}
