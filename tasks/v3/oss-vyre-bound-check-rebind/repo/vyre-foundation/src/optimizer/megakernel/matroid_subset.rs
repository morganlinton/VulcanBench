//! Greedy matroid-style fusion subset selection for megakernel planning.

/// Select a bounded fusion subset from an exchange-compatibility graph.
///
/// `seed[i] != 0` preselects item `i`. `exchange_adj[i*n + j] != 0`
/// means items `i` and `j` can coexist in the same fused group. The
/// result is a 0/1 vector capped at `max_items`.
#[must_use]
pub fn max_fusion_subset(seed: &[u32], exchange_adj: &[u32], n: usize, max_items: u32) -> Vec<u32> {
    let Some(cells) = n.checked_mul(n) else {
        return Vec::new();
    };
    if seed.len() != n || exchange_adj.len() != cells {
        return vec![0; n];
    }
    let mut selected = vec![0u32; n];
    // Track the actually-selected indices so the compatibility check
    // iterates k items (k = current selection) instead of n. The
    // previous loop scanned the full `selected` row per candidate
    // even when only a handful were chosen  -  O(n²) regardless of
    // sparsity. Greedy fusion subsets land in the few-dozen range,
    // so this is the typical case.
    let mut chosen_indices: Vec<usize> = Vec::with_capacity(max_items as usize);
    let mut count = 0u32;
    for (idx, &value) in seed.iter().enumerate() {
        if value != 0 && count < max_items {
            selected[idx] = 1;
            chosen_indices.push(idx);
            count += 1;
        }
    }
    for candidate in 0..n {
        if count >= max_items {
            break;
        }
        if selected[candidate] != 0 {
            continue;
        }
        let compatible = chosen_indices.iter().all(|&chosen| {
            exchange_adj[chosen * n + candidate] != 0 || exchange_adj[candidate * n + chosen] != 0
        });
        if compatible {
            selected[candidate] = 1;
            chosen_indices.push(candidate);
            count += 1;
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_items_selected_first() {
        // 3 items, item 1 is seeded, all compatible.
        #[rustfmt::skip]
        let exchange_adj = vec![
            0, 1, 1,
            1, 0, 1,
            1, 1, 0,
        ];
        let seed = vec![0, 1, 0];
        let result = max_fusion_subset(&seed, &exchange_adj, 3, 3);
        assert_eq!(result[1], 1, "seeded item 1 must be selected");
    }

    #[test]
    fn greedy_expands_compatible() {
        // 3 items, all pairwise compatible, unlimited budget.
        #[rustfmt::skip]
        let exchange_adj = vec![
            0, 1, 1,
            1, 0, 1,
            1, 1, 0,
        ];
        let seed = vec![0, 0, 0]; // no seeds
        let result = max_fusion_subset(&seed, &exchange_adj, 3, 10);
        let total: u32 = result.iter().sum();
        assert_eq!(total, 3, "all items should be selected when all compatible");
    }

    #[test]
    fn budget_caps_selection() {
        // 4 items, all compatible, budget = 2.
        #[rustfmt::skip]
        let exchange_adj = vec![
            0, 1, 1, 1,
            1, 0, 1, 1,
            1, 1, 0, 1,
            1, 1, 1, 0,
        ];
        let seed = vec![0, 0, 0, 0];
        let result = max_fusion_subset(&seed, &exchange_adj, 4, 2);
        let total: u32 = result.iter().sum();
        assert_eq!(total, 2, "budget must cap at max_items");
    }

    #[test]
    fn incompatible_pair_excluded() {
        // 3 items: 0-1 compatible, 0-2 compatible, 1-2 NOT compatible.
        #[rustfmt::skip]
        let exchange_adj = vec![
            0, 1, 1,
            1, 0, 0,
            1, 0, 0,
        ];
        let seed = vec![0, 0, 0];
        let result = max_fusion_subset(&seed, &exchange_adj, 3, 3);
        // Should pick at most 2 items (e.g. 0+1 or 0+2).
        let total: u32 = result.iter().sum();
        assert!(
            total <= 2,
            "incompatible 1-2 should prevent selecting all 3"
        );
    }
}
