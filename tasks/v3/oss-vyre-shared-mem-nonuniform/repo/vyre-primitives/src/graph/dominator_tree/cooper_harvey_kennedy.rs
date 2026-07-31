/// Cooper–Harvey–Kennedy iterative immediate dominators (bitset formulation).
///
/// Implements the classical dataflow algorithm using dense bitsets so the
/// result is exact and comparable to [`lengauer_tarjan_idoms`].  Memory is
/// `O(n²/32)` - acceptable for the `#[cfg(test)]` differential oracle.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cooper_harvey_kennedy_idoms(
    node_count: u32,
    entry: u32,
    edges: &[(u32, u32)],
) -> Vec<Option<u32>> {
    let n = node_count as usize;
    let entry = entry as usize;
    if n == 0 {
        return Vec::new();
    }
    if entry >= n {
        return vec![None; n];
    }

    let words = ((n + 31) / 32).max(1);
    let last_mask = if n % 32 == 0 {
        u32::MAX
    } else {
        (1u32 << (n % 32)) - 1
    };

    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(u, v) in edges {
        let u = u as usize;
        let v = v as usize;
        if u < n && v < n {
            succ[u].push(v);
            pred[v].push(u);
        }
    }

    // Flat bitset matrix: row v starts at v * words.
    let mut dom = vec![0u32; n * words];

    // Initialize: Dom(entry) = {entry}; Dom(v≠entry) = ALL.
    for v in 0..n {
        let row = v * words;
        if v == entry {
            dom[row + v / 32] |= 1u32 << (v % 32);
        } else {
            for w in 0..words {
                dom[row + w] = u32::MAX;
            }
            if last_mask != u32::MAX {
                dom[row + words - 1] = last_mask;
            }
            dom[row + v / 32] |= 1u32 << (v % 32);
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for v in 0..n {
            if v == entry {
                continue;
            }
            let row = v * words;
            let mut new = vec![u32::MAX; words];
            if last_mask != u32::MAX {
                new[words - 1] = last_mask;
            }
            for &p in &pred[v] {
                let prow = p * words;
                for w in 0..words {
                    new[w] &= dom[prow + w];
                }
            }
            new[v / 32] |= 1u32 << (v % 32);
            if &new[..] != &dom[row..row + words] {
                dom[row..row + words].copy_from_slice(&new);
                changed = true;
            }
        }
    }

    // Compute reachability from entry using BFS.
    let mut reachable = vec![false; n];
    let mut queue = vec![entry];
    reachable[entry] = true;
    while let Some(u) = queue.pop() {
        for &v in &succ[u] {
            if !reachable[v] {
                reachable[v] = true;
                queue.push(v);
            }
        }
    }

    // Convert bitsets to idoms for reachable nodes only.
    let mut idom = vec![None; n];
    idom[entry] = Some(entry as u32);
    for v in 0..n {
        if v == entry || !reachable[v] {
            continue;
        }
        let row = v * words;
        let mut strict = Vec::new();
        for d in 0..n {
            if d == v {
                continue;
            }
            if dom[row + d / 32] & (1u32 << (d % 32)) != 0 {
                strict.push(d);
            }
        }
        // idom(v) = strict dominator not strictly dominated by any other strict dom.
        for &d in &strict {
            let mut is_idom = true;
            for &c in &strict {
                if c == d {
                    continue;
                }
                if dom[c * words + d / 32] & (1u32 << (d % 32)) != 0 {
                    is_idom = false;
                    break;
                }
            }
            if is_idom {
                idom[v] = Some(d as u32);
                break;
            }
        }
    }

    idom
}
