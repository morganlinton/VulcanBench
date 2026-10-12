//! Tensor-network inspired contraction ordering for fusion planning.

/// Return a stable greedy contraction order.
///
/// Larger dimensions are contracted first to reduce large intermediate
/// buffers early; equal dimensions keep ascending index order.
#[must_use]
pub fn optimal_fusion_order(dimensions: &[u32]) -> Vec<usize> {
    let mut order = Vec::new();
    optimal_fusion_order_into(dimensions, &mut order);
    order
}

/// Return a stable greedy contraction order into caller-owned storage.
pub fn optimal_fusion_order_into(dimensions: &[u32], order: &mut Vec<usize>) {
    order.clear();
    order.extend(0..dimensions.len());
    order.sort_by(|&left, &right| {
        dimensions[right]
            .cmp(&dimensions[left])
            .then_with(|| left.cmp(&right))
    });
}

/// Score a proposed contraction order with saturating arithmetic.
#[must_use]
pub fn fusion_order_cost(dimensions: &[u32], order: &[usize]) -> u64 {
    let mut running = 1u64;
    let mut cost = 0u64;
    for &index in order {
        let Some(dimension) = dimensions.get(index).copied() else {
            continue;
        };
        let dimension = u64::from(dimension).max(1);
        running = running.saturating_mul(dimension);
        cost = cost.saturating_add(running);
    }
    cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_dimension_contracted_first() {
        let dims = [4, 16, 8, 2];
        let order = optimal_fusion_order(&dims);
        // Should be sorted by descending dimension: 16(1), 8(2), 4(0), 2(3).
        assert_eq!(order, vec![1, 2, 0, 3]);
    }

    #[test]
    fn optimal_fusion_order_into_reuses_storage() {
        let dims = [4, 16, 8, 2];
        let mut order = Vec::with_capacity(8);
        let ptr = order.as_ptr();
        optimal_fusion_order_into(&dims, &mut order);
        assert_eq!(order, vec![1, 2, 0, 3]);
        assert_eq!(order.as_ptr(), ptr);

        optimal_fusion_order_into(&[3, 3, 1], &mut order);
        assert_eq!(order, vec![0, 1, 2]);
        assert_eq!(order.as_ptr(), ptr);
    }

    #[test]
    fn generated_order_is_descending_and_stable() {
        for len in 0usize..64 {
            let dims = (0..len)
                .map(|idx| ((idx as u32 * 17 + len as u32 * 31) % 11) + 1)
                .collect::<Vec<_>>();
            let order = optimal_fusion_order(&dims);
            assert_eq!(order.len(), dims.len());
            for pair in order.windows(2) {
                let left = pair[0];
                let right = pair[1];
                assert!(
                    dims[left] > dims[right] || (dims[left] == dims[right] && left < right),
                    "dims={dims:?} order={order:?}"
                );
            }
        }
    }

    #[test]
    fn equal_dimensions_preserve_index_order() {
        let dims = [4, 4, 4];
        let order = optimal_fusion_order(&dims);
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn cost_is_positive() {
        let dims = [8, 4, 2];
        let order = optimal_fusion_order(&dims);
        let cost = fusion_order_cost(&dims, &order);
        assert!(cost > 0);
    }

    #[test]
    fn empty_dimensions() {
        let order = optimal_fusion_order(&[]);
        assert!(order.is_empty());
        assert_eq!(fusion_order_cost(&[], &[]), 0);
    }

    #[test]
    fn single_dimension() {
        let dims = [42];
        let order = optimal_fusion_order(&dims);
        assert_eq!(order, vec![0]);
        let cost = fusion_order_cost(&dims, &order);
        assert_eq!(cost, 42);
    }

    #[test]
    fn cost_saturates_and_ignores_out_of_range_order_entries() {
        let dims = [u32::MAX, u32::MAX, u32::MAX];
        let cost = fusion_order_cost(&dims, &[0, 9, 1, 2, 99]);
        assert_eq!(cost, u64::MAX);
    }
}
