//! Thin self-substrate wrapper for foundation-owned tensor-network fusion order.

use vyre_foundation::pass_substrate::tensor_network_fusion_order as foundation_tn_order;

/// Return a stable greedy contraction order.
#[must_use]
pub fn optimal_fusion_order(dimensions: &[u32]) -> Vec<usize> {
    use crate::observability::{bump, tensor_network_fusion_order_calls};
    bump(&tensor_network_fusion_order_calls);
    foundation_tn_order::optimal_fusion_order(dimensions)
}

/// Return a stable greedy contraction order into caller-owned storage.
pub fn optimal_fusion_order_into(dimensions: &[u32], order: &mut Vec<usize>) {
    use crate::observability::{bump, tensor_network_fusion_order_calls};
    bump(&tensor_network_fusion_order_calls);
    foundation_tn_order::optimal_fusion_order_into(dimensions, order);
}

/// Score a proposed contraction order with saturating arithmetic.
#[must_use]
pub fn fusion_order_cost(dimensions: &[u32], order: &[usize]) -> u64 {
    use crate::observability::{bump, tensor_network_fusion_order_calls};
    bump(&tensor_network_fusion_order_calls);
    foundation_tn_order::fusion_order_cost(dimensions, order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrappers_match_foundation_authority() {
        for len in 0usize..64 {
            let dims = (0..len)
                .map(|idx| ((idx as u32 * 19 + len as u32 * 7) % 17) + 1)
                .collect::<Vec<_>>();
            let order = optimal_fusion_order(&dims);
            assert_eq!(order, foundation_tn_order::optimal_fusion_order(&dims));
            assert_eq!(
                fusion_order_cost(&dims, &order),
                foundation_tn_order::fusion_order_cost(&dims, &order)
            );
        }
    }

    #[test]
    fn into_wrapper_reuses_storage_and_matches_owned() {
        let dims = [5, 13, 13, 2, 8];
        let mut order = Vec::with_capacity(8);
        let ptr = order.as_ptr();
        optimal_fusion_order_into(&dims, &mut order);
        assert_eq!(order, optimal_fusion_order(&dims));
        assert_eq!(order.as_ptr(), ptr);
    }
}
