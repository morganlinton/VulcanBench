//! Brute-force substring search  -  each invocation checks whether the
//! needle matches at its starting byte offset, writes `1` to the
//! match bitmap at that offset on hit.
//!
//! Category A composition. Sufficient for short needles; long
//! needles should compile to a DFA via the future `dfa_compile`
//! function and use that as a prefilter.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

/// Canonical scan op id.
pub const SCAN_SUBSTRING_OP_ID: &str = "vyre-libs::scan::substring_search";
/// Deprecated matching op id retained only by the matching compatibility path.
pub(crate) const LEGACY_MATCHING_SUBSTRING_OP_ID: &str = "vyre-libs::matching::substring_search";

/// Build a Program that writes `1` to `matches[i]` when `haystack[i..]`
/// starts with `needle`, else `0`. Both buffers are u32 byte arrays
/// packed one byte per u32 for simplicity (a future packed-u8 version
/// is Category A over `DataType::U8`).
#[must_use]
pub fn substring_search(
    haystack: &str,
    needle: &str,
    matches: &str,
    haystack_len: u32,
    needle_len: u32,
) -> Program {
    substring_search_with_op_id(
        SCAN_SUBSTRING_OP_ID,
        haystack,
        needle,
        matches,
        haystack_len,
        needle_len,
    )
}

/// Build a substring Program with an explicit compatibility op id.
#[must_use]
pub(crate) fn substring_search_with_op_id(
    op_id: &str,
    haystack: &str,
    needle: &str,
    matches: &str,
    haystack_len: u32,
    needle_len: u32,
) -> Program {
    let counted_storage = |name: &str, binding, count| {
        let decl = BufferDecl::storage(name, binding, BufferAccess::ReadOnly, DataType::U32);
        if count == 0 {
            decl
        } else {
            decl.with_count(count)
        }
    };
    let output_count = haystack_len.max(1);
    let visible_output_bytes = (haystack_len as usize).saturating_mul(4);
    let output = BufferDecl::output(matches, 2, DataType::U32)
        .with_count(output_count)
        .with_output_byte_range(0..visible_output_bytes);

    let i = Expr::var("i");
    // ok accumulates AND of per-byte equality checks. Start at 1; each
    // byte mismatch AND-s in 0 and latches the match bit off.
    let mut check_body: Vec<Node> = vec![Node::let_bind("ok", Expr::u32(1))];
    // Walk the needle one byte at a time. bytes are packed u32/byte for
    // simplicity  -  a packed-u8 variant is Category A over DataType::U8.
    check_body.push(Node::loop_for(
        "k",
        Expr::u32(0),
        Expr::u32(needle_len),
        vec![Node::assign(
            "ok",
            Expr::bitand(
                Expr::var("ok"),
                // Select turns the bool comparison into u32 {0,1} so
                // the accumulator stays in integer arithmetic.
                Expr::select(
                    Expr::eq(
                        Expr::load(haystack, Expr::add(i.clone(), Expr::var("k"))),
                        Expr::load(needle, Expr::var("k")),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
        )],
    ));
    check_body.push(Node::Store {
        buffer: matches.into(),
        index: i.clone(),
        value: Expr::var("ok"),
    });

    // Overflow-safe guard. The straight expression `i + needle_len <= buf_len`
    // can wrap at i ≈ u32::MAX − needle_len, producing a false positive on the
    // last few offsets. The correct invariant is `i <= buf_len - needle_len`;
    // we rewrite it as a subtraction-free chain of comparisons by reasoning
    // through `buf_len` only:
    //
    //   needle_len <= buf_len  ∧  i + needle_len <= buf_len
    //
    // Passing both conjuncts also handles the empty-haystack case (buf_len=0,
    // needle_len=0, i=0 → both true → vacuous check_body).
    // V7-CORR-006: the original guard `i + needle_len <= haystack_len`
    // wraps when i is near u32::MAX (Expr::add is Expr::BinOp { Add,
    // .. } which u32::wrapping_add). We rewrite as two separate
    // non-wrapping comparisons: (1) needle_len <= haystack_len ensures
    // the implicit subtraction in (2) is non-underflowing, and (2)
    // `i <= haystack_len - needle_len` keeps the rhs a constant-folded
    // expression from the builder so no wrap is possible. Since
    // needle_len is a compile-time u32 and haystack_len is a runtime
    // u32, the host-side `saturating_sub` pre-computes the cap value
    // safely and lets Expr::le do the comparison without Expr::add.
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::let_bind("haystack_len", Expr::buf_len(haystack)),
        Node::if_then(
            Expr::and(
                Expr::le(Expr::u32(needle_len), Expr::var("haystack_len")),
                Expr::le(
                    i.clone(),
                    // `haystack_len - needle_len` as a runtime Expr sub. If
                    // the compile-time needle_len exceeds the runtime
                    // haystack_len the first conjunct already short-
                    // circuits, so this sub-expression is evaluated only
                    // on the safe branch (eager vs lazy evaluation is
                    // the job of the optimizer's short-circuit pass  -
                    // here the conjunct ordering gives a safe guard).
                    Expr::sub(Expr::var("haystack_len"), Expr::u32(needle_len)),
                ),
            ),
            check_body,
        ),
    ];
    Program::wrapped(
        vec![
            counted_storage(haystack, 0, haystack_len),
            counted_storage(needle, 1, needle_len),
            output,
        ],
        [64, 1, 1],
        vec![wrap_anonymous(op_id, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SCAN_SUBSTRING_OP_ID,
        build: || substring_search("haystack", "needle", "matches", 8, 3),
        test_inputs: Some(|| {
            let to_u32_vec = |s: &str| s.bytes().map(u32::from).collect::<Vec<_>>();
            vec![
                vec![
                    crate::test_support::byte_pack::u32_bytes(&to_u32_vec("abcabc++")),
                    crate::test_support::byte_pack::u32_bytes(&to_u32_vec("abc")),
                ],
                vec![
                    crate::test_support::byte_pack::u32_bytes(&to_u32_vec("xyzxyzxy")),
                    crate::test_support::byte_pack::u32_bytes(&to_u32_vec("xyz")),
                ]
            ]
        }),
        expected_output: Some(|| {
            // Case 0: haystack="abcabc++", needle="abc". Matches at
            //   i ∈ {0, 3}. Positions i > haystack_len - needle_len
            //   (5) stay at their zero init because the guard never
            //   fires.
            // Case 1: haystack="xyzxyzxy", needle="xyz". Matches at
            //   i ∈ {0, 3}.
            let case0 = crate::test_support::byte_pack::u32_bytes(&[1u32, 0, 0, 1, 0, 0, 0, 0]);
            let case1 = crate::test_support::byte_pack::u32_bytes(&[1u32, 0, 0, 1, 0, 0, 0, 0]);
            vec![vec![case0], vec![case1]]
        }),
        category: Some("scan"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_scan_builder_uses_scan_op_id_not_matching_id() {
        let program = substring_search("haystack", "needle", "matches", 8, 3);
        let [Node::Region { generator, .. }] = program.entry() else {
            panic!("expected substring search to emit one scan region");
        };

        assert_eq!(generator.as_str(), SCAN_SUBSTRING_OP_ID);
        assert_ne!(generator.as_str(), LEGACY_MATCHING_SUBSTRING_OP_ID);
    }

    #[test]
    fn explicit_compatibility_builder_preserves_legacy_op_id() {
        let program = substring_search_with_op_id(
            LEGACY_MATCHING_SUBSTRING_OP_ID,
            "haystack",
            "needle",
            "matches",
            8,
            3,
        );
        let [Node::Region { generator, .. }] = program.entry() else {
            panic!("expected substring compatibility search to emit one region");
        };

        assert_eq!(generator.as_str(), LEGACY_MATCHING_SUBSTRING_OP_ID);
    }

    #[test]
    fn source_boundary_keeps_matching_identity_out_of_canonical_builder() {
        let source = include_str!("substring.rs");
        let canonical_builder = source
            .split("pub fn substring_search(")
            .nth(1)
            .expect("Fix: canonical substring builder must exist")
            .split("/// Build a substring Program with an explicit compatibility op id.")
            .next()
            .expect("Fix: compatibility builder must follow canonical substring builder");

        assert!(canonical_builder.contains("SCAN_SUBSTRING_OP_ID"));
        assert!(!canonical_builder.contains("LEGACY_MATCHING_SUBSTRING_OP_ID"));
        assert!(!canonical_builder.contains("vyre-libs::matching::substring_search"));
    }
}
