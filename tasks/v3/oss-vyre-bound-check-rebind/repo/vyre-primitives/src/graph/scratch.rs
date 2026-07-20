//! Shared graph scratch-buffer reservation helpers.
//!
//! Graph CPU oracles and primitive-owned structure-of-arrays builders all need
//! the same property: reserve before mutating caller-owned scratch, report the
//! domain and context on allocation failure, and never hide OOM/overflow behind
//! truncation or saturation.

/// Reserve additional items in a graph scratch vector with a standard,
/// actionable allocation diagnostic.
///
/// # Errors
///
/// Returns a message naming the graph primitive owner, scratch context, and
/// allocator failure.
pub(crate) fn reserve_graph_items<T>(
    buffer: &mut Vec<T>,
    additional: usize,
    owner: &str,
    context: &str,
) -> Result<(), String> {
    buffer.try_reserve(additional).map_err(|error| {
        format!(
            "Fix: {owner} could not reserve {additional} item(s) for {context}: {error}. Split the graph batch or reuse a smaller scratch buffer."
        )
    })
}

/// Reserve graph scratch and map the shared diagnostic into a domain-specific
/// error type.
///
/// # Errors
///
/// Returns the mapped allocation error when `Vec::try_reserve` fails.
pub(crate) fn reserve_graph_items_with<T, E>(
    buffer: &mut Vec<T>,
    additional: usize,
    owner: &str,
    context: &str,
    map: impl FnOnce(String) -> E,
) -> Result<(), E> {
    reserve_graph_items(buffer, additional, owner, context).map_err(map)
}

#[cfg(test)]
mod tests {
    use super::reserve_graph_items;

    #[test]
    fn reserve_graph_items_reuses_existing_capacity() {
        let mut scratch = Vec::<u32>::with_capacity(8);

        reserve_graph_items(&mut scratch, 4, "test graph primitive", "frontier")
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - existing capacity should satisfy reservation");

        assert_eq!(scratch.capacity(), 8);
        assert!(scratch.is_empty());
    }

    #[test]
    fn reserve_graph_items_reports_owner_and_context_on_capacity_overflow() {
        let mut scratch = Vec::<u8>::new();

        let err = reserve_graph_items(
            &mut scratch,
            usize::MAX,
            "test graph primitive",
            "adversarial huge scratch",
        )
        .expect_err("usize::MAX reservation must fail without allocating");

        assert!(err.contains("test graph primitive"));
        assert!(err.contains("adversarial huge scratch"));
        assert!(err.contains("usize::MAX") || err.contains("capacity"));
    }

    #[test]
    fn reserve_graph_items_with_preserves_domain_error_mapping() {
        #[derive(Debug, PartialEq, Eq)]
        struct DomainError(String);

        let mut scratch = Vec::<u8>::new();
        let err = super::reserve_graph_items_with(
            &mut scratch,
            usize::MAX,
            "mapped graph primitive",
            "mapped scratch",
            DomainError,
        )
        .expect_err("usize::MAX reservation must fail without allocating");

        assert!(err.0.contains("mapped graph primitive"));
        assert!(err.0.contains("mapped scratch"));
    }
}
