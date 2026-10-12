use super::alloc_helpers::{push_dominator_vec, resize_dominator_vec};
use super::lengauer_tarjan::{
    lengauer_tarjan_idoms, try_lengauer_tarjan_idoms, try_lengauer_tarjan_idoms_into,
    DominatorTreeCpuScratch,
};

/// Canonical CPU oracle: exact Lengauer–Tarjan.
///
/// Returns a fresh `Vec<Option<u32>>` where index `v` is the immediate
/// dominator of `v` (or `None` for unreachable nodes).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(node_count: u32, entry: u32, edges: &[(u32, u32)]) -> Vec<Option<u32>> {
    lengauer_tarjan_idoms(node_count, entry, edges)
}

/// Fallible canonical CPU oracle: exact Lengauer-Tarjan.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref(
    node_count: u32,
    entry: u32,
    edges: &[(u32, u32)],
) -> Result<Vec<Option<u32>>, String> {
    try_lengauer_tarjan_idoms(node_count, entry, edges)
}

/// Fallible canonical CPU oracle using caller-owned output and scratch storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    node_count: u32,
    entry: u32,
    edges: &[(u32, u32)],
    out: &mut Vec<Option<u32>>,
    scratch: &mut DominatorTreeCpuScratch,
) -> Result<(), String> {
    try_lengauer_tarjan_idoms_into(node_count, entry, edges, out, scratch)
}

/// Convert an idom array to per-node dominator sets (sorted).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn idoms_to_dominator_sets(idoms: &[Option<u32>], node_count: u32) -> Vec<Vec<u32>> {
    try_idoms_to_dominator_sets(idoms, node_count).unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible conversion of an idom array to per-node dominator sets (sorted).
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_idoms_to_dominator_sets(
    idoms: &[Option<u32>],
    node_count: u32,
) -> Result<Vec<Vec<u32>>, String> {
    let n = node_count as usize;
    if idoms.len() < n {
        return Err(format!(
            "dominator_tree idom set conversion received idoms_len={} for node_count={node_count}. Fix: pass one idom slot per graph node.",
            idoms.len()
        ));
    }
    let mut sets: Vec<Vec<u32>> = Vec::new();
    resize_dominator_vec(&mut sets, n, Vec::new(), "dominator_tree dominator sets")?;
    for v in 0..n {
        let mut cur = v;
        let mut set = Vec::new();
        push_dominator_vec(&mut set, cur as u32, "dominator_tree per-node set")?;
        while let Some(p) = idoms[cur] {
            if p == cur as u32 {
                break;
            }
            push_dominator_vec(&mut set, p, "dominator_tree per-node set")?;
            cur = p as usize;
        }
        set.sort_unstable();
        sets[v] = set;
    }
    Ok(sets)
}
