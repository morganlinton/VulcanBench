//! Shared decode-to-DFA scan bodies.

use vyre::ir::{Expr, Node};

const ALPHABET_SIZE: u32 = 256;

fn transition_expr(transitions: &str, state: Expr, byte: Expr) -> Expr {
    Expr::load(
        transitions,
        Expr::add(Expr::mul(state, Expr::u32(ALPHABET_SIZE)), byte),
    )
}

/// Build a bounded Aho-Corasick scan body for fused decoders.
///
/// The scanner walks the decoded stream once and writes every accepting state
/// in order. This preserves the existing Aho-Corasick output contract without
/// replaying the prefix independently for every output position.
#[must_use]
pub(crate) fn linear_aho_scan_body(
    input: &str,
    transitions: &str,
    accept: &str,
    matches: &str,
    valid_len: Expr,
) -> Vec<Node> {
    vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("state", Expr::u32(0)),
            Node::loop_for(
                "decode_scan_step",
                Expr::u32(0),
                valid_len,
                vec![
                    Node::let_bind("byte", Expr::load(input, Expr::var("decode_scan_step"))),
                    Node::assign(
                        "state",
                        transition_expr(transitions, Expr::var("state"), Expr::var("byte")),
                    ),
                    Node::store(
                        matches,
                        Expr::var("decode_scan_step"),
                        Expr::load(accept, Expr::var("state")),
                    ),
                ],
            ),
        ],
    )]
}

/// Build a single-invocation tiled Aho-Corasick body over a caller-supplied
/// byte expression.
///
/// The body keeps DFA state in registers and advances over bounded tiles,
/// alternating the decoded byte through two scalar slots. For decoders that can
/// expose `byte_at(index)` cheaply, this avoids the old decode-buffer readback
/// pass: decode for the next slot and scan for the current slot are fused in one
/// loop nest. The optional `store_decoded` hook preserves the public decoded
/// buffer contract for existing builders.
#[must_use]
pub(crate) fn tiled_decode_aho_scan_body<ByteAt, StoreDecoded>(
    transitions: &str,
    accept: &str,
    matches: &str,
    valid_len: Expr,
    tile_width: u32,
    mut byte_at: ByteAt,
    mut store_decoded: StoreDecoded,
) -> Vec<Node>
where
    ByteAt: FnMut(Expr) -> Expr,
    StoreDecoded: FnMut(Expr, Expr) -> Option<Node>,
{
    let tile_width = tile_width.max(1).next_power_of_two();
    let tile_count = tiled_scan_tile_count_expr(valid_len.clone(), tile_width);
    vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("state", Expr::u32(0)),
            Node::let_bind("decode_scan_ping", Expr::u32(0)),
            Node::let_bind("decode_scan_pong", Expr::u32(0)),
            Node::loop_for(
                "decode_scan_tile_index",
                Expr::u32(0),
                tile_count,
                vec![
                    Node::let_bind(
                        "decode_scan_tile_base",
                        Expr::mul(Expr::var("decode_scan_tile_index"), Expr::u32(tile_width)),
                    ),
                    Node::loop_for(
                        "decode_scan_tile_lane",
                        Expr::u32(0),
                        Expr::u32(tile_width),
                        tiled_lane_body(
                            transitions,
                            accept,
                            matches,
                            valid_len.clone(),
                            &mut byte_at,
                            &mut store_decoded,
                        ),
                    ),
                ],
            ),
        ],
    )]
}

fn tiled_scan_tile_count_expr(valid_len: Expr, tile_width: u32) -> Expr {
    let tile_width = tile_width.max(1).next_power_of_two();
    Expr::select(
        Expr::eq(valid_len.clone(), Expr::u32(0)),
        Expr::u32(0),
        Expr::add(
            Expr::div(Expr::sub(valid_len, Expr::u32(1)), Expr::u32(tile_width)),
            Expr::u32(1),
        ),
    )
}

fn tiled_lane_body<ByteAt, StoreDecoded>(
    transitions: &str,
    accept: &str,
    matches: &str,
    valid_len: Expr,
    byte_at: &mut ByteAt,
    store_decoded: &mut StoreDecoded,
) -> Vec<Node>
where
    ByteAt: FnMut(Expr) -> Expr,
    StoreDecoded: FnMut(Expr, Expr) -> Option<Node>,
{
    let index = Expr::add(
        Expr::var("decode_scan_tile_base"),
        Expr::var("decode_scan_tile_lane"),
    );
    let slot_is_ping = Expr::eq(
        Expr::bitand(Expr::var("decode_scan_tile_lane"), Expr::u32(1)),
        Expr::u32(0),
    );
    let decoded = byte_at(index.clone());
    let mut body = vec![Node::let_bind("decode_scan_byte", decoded)];
    if let Some(store) = store_decoded(index.clone(), Expr::var("decode_scan_byte")) {
        body.push(store);
    }
    body.extend([
        Node::if_then_else(
            slot_is_ping,
            vec![Node::assign(
                "decode_scan_ping",
                Expr::var("decode_scan_byte"),
            )],
            vec![Node::assign(
                "decode_scan_pong",
                Expr::var("decode_scan_byte"),
            )],
        ),
        Node::assign(
            "state",
            transition_expr(
                transitions,
                Expr::var("state"),
                Expr::var("decode_scan_byte"),
            ),
        ),
        Node::store(
            matches,
            index.clone(),
            Expr::load(accept, Expr::var("state")),
        ),
    ]);
    vec![Node::if_then(Expr::lt(index, valid_len), body)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiled_decode_scan_uses_tile_count_loop_not_byte_count_gate() {
        let body = tiled_decode_aho_scan_body(
            "transitions",
            "accept",
            "matches",
            Expr::u32(1024),
            8,
            |index| Expr::load("decoded", index),
            |_index, _byte| None,
        );
        let rendered = format!("{body:?}");
        assert!(
            rendered.contains("decode_scan_tile_index"),
            "fused decode-scan must loop over tile indices, not every byte offset"
        );
        assert!(
            rendered.contains("decode_scan_tile_base"),
            "fused decode-scan must derive a tile base from the tile index"
        );
    }
}
