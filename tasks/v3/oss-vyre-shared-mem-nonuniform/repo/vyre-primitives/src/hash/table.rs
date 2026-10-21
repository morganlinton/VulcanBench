//! GPU-native lock-free hash table primitives.
//!
//! Tier 2.5 LEGO components returning `Vec<Node>` fragments.
//! Program construction and harness registration belong to higher-level
//! composition crates that choose table shape and dispatch policy.

use vyre_foundation::ir::{Expr, Node};
use vyre_foundation::MemoryOrdering;

use super::fnv1a::{fnv1a32_initial_expr, fnv1a32_update_byte_expr};

/// Empty key sentinel for the in-place hash table representation.
///
/// Tables using these fragments must initialize `table_keys` to this value.
/// `u32::MAX` and `u32::MAX - 1` are reserved by the probing protocol.
pub const EMPTY_KEY: u32 = u32::MAX;
/// Transient reservation marker used while an inserter publishes the value.
pub const RESERVED_KEY: u32 = u32::MAX - 1;
/// Value written by `hash_lookup` when a key is absent.
pub const MISS_VALUE: u32 = u32::MAX;

/// GPU-Native Lock-Free Perfect Hash Table Insert
///
/// Intended for O(1) Macro and Keyword lookups.
/// Uses bounded linear probing and compare-exchange reservations. The
/// returned fragment is wait-free for readers and bounded for writers:
/// every lane probes at most `table_capacity` slots.
///
/// Returns the body nodes for insertion. Caller wraps in a Program.
#[must_use]
pub fn hash_insert(
    in_keys: &str,
    in_values: &str,
    table_keys: &str,
    table_values: &str,
    table_capacity: u32,
    t: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind("key", Expr::load(in_keys, t.clone())),
        Node::let_bind("val", Expr::load(in_values, t.clone())),
        Node::let_bind("hash", fnv1a32_u32_expr(Expr::var("key"))),
        Node::let_bind(
            "slot",
            Expr::rem(Expr::var("hash"), Expr::u32(table_capacity)),
        ),
        Node::let_bind("inserted", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::ne(Expr::var("key"), Expr::u32(EMPTY_KEY)),
                Expr::ne(Expr::var("key"), Expr::u32(RESERVED_KEY)),
            ),
            vec![Node::loop_for(
                "probe",
                Expr::u32(0),
                Expr::u32(table_capacity),
                vec![Node::if_then(
                    Expr::eq(Expr::var("inserted"), Expr::u32(0)),
                    vec![
                        Node::let_bind(
                            "probe_slot",
                            Expr::rem(
                                Expr::add(Expr::var("slot"), Expr::var("probe")),
                                Expr::u32(table_capacity),
                            ),
                        ),
                        Node::let_bind(
                            "previous_key",
                            Expr::atomic_compare_exchange_ordered(
                                table_keys,
                                Expr::var("probe_slot"),
                                Expr::u32(EMPTY_KEY),
                                Expr::u32(RESERVED_KEY),
                                MemoryOrdering::AcqRel,
                            ),
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("previous_key"), Expr::u32(EMPTY_KEY)),
                            vec![
                                Node::store(
                                    table_values,
                                    Expr::var("probe_slot"),
                                    Expr::var("val"),
                                ),
                                Node::let_bind(
                                    "_publish_key",
                                    Expr::atomic_exchange(
                                        table_keys,
                                        Expr::var("probe_slot"),
                                        Expr::var("key"),
                                    ),
                                ),
                                Node::assign("inserted", Expr::u32(1)),
                            ],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("previous_key"), Expr::var("key")),
                            vec![
                                Node::store(
                                    table_values,
                                    Expr::var("probe_slot"),
                                    Expr::var("val"),
                                ),
                                Node::assign("inserted", Expr::u32(1)),
                            ],
                        ),
                    ],
                )],
            )],
        ),
    ]
}

/// GPU-Native Lock-Free Perfect Hash Table Lookup
///
/// Returns the body nodes for lookup. Caller wraps in a Program.
#[must_use]
pub fn hash_lookup(
    queries: &str,
    table_keys: &str,
    table_values: &str,
    out_results: &str,
    table_capacity: u32,
    t: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind("query", Expr::load(queries, t.clone())),
        Node::store(out_results, t.clone(), Expr::u32(MISS_VALUE)),
        Node::let_bind("found", Expr::u32(0)),
        Node::let_bind("hash", fnv1a32_u32_expr(Expr::var("query"))),
        Node::let_bind(
            "slot",
            Expr::rem(Expr::var("hash"), Expr::u32(table_capacity)),
        ),
        Node::loop_for(
            "probe",
            Expr::u32(0),
            Expr::u32(table_capacity),
            vec![Node::if_then(
                Expr::eq(Expr::var("found"), Expr::u32(0)),
                vec![
                    Node::let_bind(
                        "probe_slot",
                        Expr::rem(
                            Expr::add(Expr::var("slot"), Expr::var("probe")),
                            Expr::u32(table_capacity),
                        ),
                    ),
                    Node::let_bind("found_key", Expr::load(table_keys, Expr::var("probe_slot"))),
                    Node::if_then(
                        Expr::eq(Expr::var("found_key"), Expr::var("query")),
                        vec![
                            Node::store(
                                out_results,
                                t.clone(),
                                Expr::load(table_values, Expr::var("probe_slot")),
                            ),
                            Node::assign("found", Expr::u32(1)),
                        ],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("found_key"), Expr::u32(EMPTY_KEY)),
                        vec![Node::assign("found", Expr::u32(1))],
                    ),
                ],
            )],
        ),
    ]
}

fn fnv1a32_u32_expr(value: Expr) -> Expr {
    let byte0 = Expr::bitand(value.clone(), Expr::u32(0xFF));
    let byte1 = Expr::bitand(Expr::shr(value.clone(), Expr::u32(8)), Expr::u32(0xFF));
    let byte2 = Expr::bitand(Expr::shr(value.clone(), Expr::u32(16)), Expr::u32(0xFF));
    let byte3 = Expr::bitand(Expr::shr(value, Expr::u32(24)), Expr::u32(0xFF));
    fnv1a32_update_byte_expr(
        fnv1a32_update_byte_expr(
            fnv1a32_update_byte_expr(
                fnv1a32_update_byte_expr(fnv1a32_initial_expr(), byte0),
                byte1,
            ),
            byte2,
        ),
        byte3,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(nodes: &[Node]) -> String {
        format!("{nodes:?}")
    }

    #[test]
    fn hash_insert_uses_real_bounded_cas_probe() {
        let nodes = hash_insert("keys", "vals", "table_keys", "table_vals", 64, Expr::u32(0));
        let dbg = rendered(&nodes);
        assert!(
            dbg.contains("CompareExchange"),
            "Fix: hash_insert must claim slots with CAS instead of blind stores: {dbg}"
        );
        assert!(
            dbg.contains("RESERVED") || dbg.contains(&(RESERVED_KEY).to_string()),
            "Fix: hash_insert must reserve a key slot before publishing the key: {dbg}"
        );
        assert!(
            !dbg.contains("vyre-primitives::crypto::fnv1a"),
            "Fix: hash_insert must not call a fake hash op id: {dbg}"
        );
    }

    #[test]
    fn table_hash_uses_canonical_fnv1a_helper_expression() {
        let source = include_str!("table.rs");
        assert!(
            source.contains("fnv1a32_update_byte_expr"),
            "Fix: hash table fragments must reuse the canonical FNV-1a32 helper."
        );
        assert!(
            !source.contains(concat!("FNV1A32", "_PRIME")),
            "Fix: hash table fragments must not fork FNV-1a32 constants."
        );
        assert!(
            !source.contains(concat!("fn ", "fnv1a32_step")),
            "Fix: hash table fragments must not carry a private FNV-1a32 step."
        );
    }

    #[test]
    fn hash_lookup_probes_until_match_or_empty_and_sets_miss() {
        let nodes = hash_lookup(
            "queries",
            "table_keys",
            "table_vals",
            "out",
            64,
            Expr::u32(0),
        );
        let dbg = rendered(&nodes);
        assert!(
            dbg.contains("Loop"),
            "Fix: hash_lookup must probe collision chains, not inspect only the home slot: {dbg}"
        );
        assert!(
            dbg.contains(&MISS_VALUE.to_string()),
            "Fix: hash_lookup must deterministically initialize misses: {dbg}"
        );
    }
}
