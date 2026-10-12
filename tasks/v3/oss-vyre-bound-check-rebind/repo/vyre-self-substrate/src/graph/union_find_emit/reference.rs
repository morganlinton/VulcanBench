/// Path-compress every entry in `parent` so each cell holds the
/// canonical root of its component. Pure reference helper used by parity
/// tests to compare partitions independent of intermediate parent
/// links.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn canonicalize_parent_to_roots(parent: &[u32]) -> Vec<u32> {
    let mut roots = parent.to_vec();
    for i in 0..roots.len() {
        let mut node = i as u32;
        while (node as usize) < roots.len() && roots[node as usize] != node {
            node = roots[node as usize];
        }
        roots[i] = node;
    }
    roots
}

/// Reference oracle for the union-find batch: starting from `parent_init`
/// (typically the identity vector `[0, 1, 2, ...]`), apply each
/// `(edge_a[k], edge_b[k])` union via path-compressed find. Returns
/// the final parent vector (NOT root-canonicalised  -  feed to
/// [`canonicalize_parent_to_roots`] for partition comparison).
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_union_find_alias(parent_init: &[u32], edge_a: &[u32], edge_b: &[u32]) -> Vec<u32> {
    assert_eq!(
        edge_a.len(),
        edge_b.len(),
        "Fix: edge_a / edge_b must have matching length; got {} vs {}.",
        edge_a.len(),
        edge_b.len()
    );
    let mut parent = parent_init.to_vec();
    fn find(parent: &mut [u32], mut x: u32) -> u32 {
        while parent[x as usize] != x {
            let next = parent[x as usize];
            parent[x as usize] = parent[next as usize];
            x = next;
        }
        x
    }
    for (&a, &b) in edge_a.iter().zip(edge_b.iter()) {
        let ra = find(&mut parent, a);
        let rb = find(&mut parent, b);
        if ra != rb {
            // Union by min  -  matches the GPU CAS-min contract so root
            // identifiers agree exactly modulo `canonicalize_parent_to_roots`.
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            parent[hi as usize] = lo;
        }
    }
    parent
}
