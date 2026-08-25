//! Shared BLAKE3 compression helpers.
//!
//! `blake3_g` is the four-word mixing function reused eight times per round.
//! `blake3_round` remaps the message schedule and applies the eight `G`
//! quartets for one permutation round.

use std::sync::Arc;
use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable Tier 2.5 op id for the BLAKE3 `G` mixing function.
pub const BLAKE3_G_OP_ID: &str = "vyre-primitives::hash::blake3_g";
/// Stable Tier 2.5 op id for one BLAKE3 round.
pub const BLAKE3_ROUND_OP_ID: &str = "vyre-primitives::hash::blake3_round";

/// Message permutation applied between rounds.
pub const MSG_SCHEDULE: [[usize; 16]; 7] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8],
    [3, 4, 10, 12, 13, 2, 7, 14, 6, 5, 9, 0, 11, 15, 8, 1],
    [10, 7, 12, 9, 14, 3, 13, 15, 4, 0, 11, 2, 5, 8, 1, 6],
    [12, 13, 9, 11, 15, 10, 14, 8, 7, 2, 5, 3, 0, 1, 6, 4],
    [9, 14, 11, 5, 8, 12, 15, 1, 13, 3, 0, 10, 2, 6, 4, 7],
    [11, 15, 5, 0, 1, 9, 8, 6, 14, 10, 2, 12, 3, 4, 7, 13],
];

/// Emit the BLAKE3 `G` mixing quartet.
#[must_use]
pub fn blake3_g(a: usize, b: usize, c: usize, d: usize, mx: &str, my: &str) -> Vec<Node> {
    let sa = format!("s{a}");
    let sb = format!("s{b}");
    let sc = format!("s{c}");
    let sd = format!("s{d}");

    vec![
        Node::assign(
            sa.clone(),
            Expr::add(
                Expr::add(Expr::var(sa.clone()), Expr::var(sb.clone())),
                Expr::var(mx),
            ),
        ),
        Node::assign(
            sd.clone(),
            rotate_right(
                Expr::bitxor(Expr::var(sd.clone()), Expr::var(sa.clone())),
                16,
            ),
        ),
        Node::assign(
            sc.clone(),
            Expr::add(Expr::var(sc.clone()), Expr::var(sd.clone())),
        ),
        Node::assign(
            sb.clone(),
            rotate_right(
                Expr::bitxor(Expr::var(sb.clone()), Expr::var(sc.clone())),
                12,
            ),
        ),
        Node::assign(
            sa.clone(),
            Expr::add(
                Expr::add(Expr::var(sa.clone()), Expr::var(sb.clone())),
                Expr::var(my),
            ),
        ),
        Node::assign(
            sd.clone(),
            rotate_right(Expr::bitxor(Expr::var(sd.clone()), Expr::var(sa)), 8),
        ),
        Node::assign(sc.clone(), Expr::add(Expr::var(sc.clone()), Expr::var(sd))),
        Node::assign(
            sb.clone(),
            rotate_right(Expr::bitxor(Expr::var(sb), Expr::var(sc)), 7),
        ),
    ]
}

/// Emit one BLAKE3 round: remap message words, then apply 8 `G` quartets.
#[must_use]
pub fn blake3_round(round_idx: usize, perm: &[usize; 16]) -> Vec<Node> {
    let mut body = Vec::with_capacity(24);
    for (i, &src) in perm.iter().enumerate() {
        body.push(Node::let_bind(
            format!("r{round_idx}_m{i}"),
            Expr::var(format!("m{src}")),
        ));
    }

    let parent = GeneratorRef {
        name: BLAKE3_ROUND_OP_ID.to_string(),
    };
    let quartets: [(usize, usize, usize, usize, usize, usize); 8] = [
        (0, 4, 8, 12, 0, 1),
        (1, 5, 9, 13, 2, 3),
        (2, 6, 10, 14, 4, 5),
        (3, 7, 11, 15, 6, 7),
        (0, 5, 10, 15, 8, 9),
        (1, 6, 11, 12, 10, 11),
        (2, 7, 8, 13, 12, 13),
        (3, 4, 9, 14, 14, 15),
    ];
    for (a, b, c, d, mx, my) in quartets {
        body.push(Node::Region {
            generator: Ident::from(BLAKE3_G_OP_ID),
            source_region: Some(parent.clone()),
            body: Arc::new(blake3_g(
                a,
                b,
                c,
                d,
                &format!("r{round_idx}_m{mx}"),
                &format!("r{round_idx}_m{my}"),
            )),
        });
    }
    body
}

/// Standalone Program for one BLAKE3 `G` mixing quartet.
#[must_use]
pub fn blake3_g_program(state: &str, message: &str, out: &str) -> Program {
    let mut body = load_state_nodes(state);
    body.push(Node::let_bind("m0", Expr::load(message, Expr::u32(0))));
    body.push(Node::let_bind("m1", Expr::load(message, Expr::u32(1))));
    body.push(Node::Block(blake3_g(0, 4, 8, 12, "m0", "m1")));
    body.extend(store_state_nodes(out));

    Program::wrapped(
        vec![
            BufferDecl::storage(state, 0, BufferAccess::ReadOnly, DataType::U32).with_count(16),
            BufferDecl::storage(message, 1, BufferAccess::ReadOnly, DataType::U32).with_count(2),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32).with_count(16),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(BLAKE3_G_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Standalone Program for one BLAKE3 permutation round.
#[must_use]
pub fn blake3_round_program(state: &str, message: &str, out: &str) -> Program {
    let mut body = load_state_nodes(state);
    for i in 0..16 {
        body.push(Node::let_bind(
            format!("m{i}"),
            Expr::load(message, Expr::u32(i)),
        ));
    }
    body.push(Node::Block(blake3_round(0, &MSG_SCHEDULE[0])));
    body.extend(store_state_nodes(out));

    Program::wrapped(
        vec![
            BufferDecl::storage(state, 0, BufferAccess::ReadOnly, DataType::U32).with_count(16),
            BufferDecl::storage(message, 1, BufferAccess::ReadOnly, DataType::U32).with_count(16),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32).with_count(16),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(BLAKE3_ROUND_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

fn rotate_right(x: Expr, n: u32) -> Expr {
    // Delegate to the public first-class builder (single construction path for
    // BinOp::RotateRight) instead of hand-assembling the node — they share one
    // canonical fingerprint, proven by the operators.rs builder tests.
    Expr::rotate_right(x, Expr::u32(n))
}

fn load_state_nodes(state: &str) -> Vec<Node> {
    (0..16)
        .map(|i| Node::let_bind(format!("s{i}"), Expr::load(state, Expr::u32(i))))
        .collect()
}

fn store_state_nodes(out: &str) -> Vec<Node> {
    (0..16)
        .map(|i| Node::store(out, Expr::u32(i), Expr::var(format!("s{i}"))))
        .collect()
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        BLAKE3_G_OP_ID,
        || blake3_g_program("state", "message", "out"),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0; 16]),
                to_bytes(&[0; 2]),
                to_bytes(&[0; 16]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0; 16])]]
        }),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        BLAKE3_ROUND_OP_ID,
        || blake3_round_program("state", "message", "out"),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0; 16]),
                to_bytes(&[0; 16]),
                to_bytes(&[0; 16]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0; 16])]]
        }),
    )
}

// ---------------------------------------------------------------------------
// CPU reference implementations
// ---------------------------------------------------------------------------

/// CPU reference: BLAKE3 G mixing quartet on state words `(a, b, c, d)` with
/// message words `(mx, my)`. Mutates `state` in place.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_blake3_g(
    state: &mut [u32; 16],
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    mx: u32,
    my: u32,
) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

/// CPU reference: one full BLAKE3 permutation round with message schedule
/// permutation applied to `message`.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_blake3_round(state: &mut [u32; 16], message: &[u32; 16], perm: &[usize; 16]) {
    let mut m = [0u32; 16];
    for (i, &src) in perm.iter().enumerate() {
        m[i] = message[src];
    }
    // Column step
    cpu_blake3_g(state, 0, 4, 8, 12, m[0], m[1]);
    cpu_blake3_g(state, 1, 5, 9, 13, m[2], m[3]);
    cpu_blake3_g(state, 2, 6, 10, 14, m[4], m[5]);
    cpu_blake3_g(state, 3, 7, 11, 15, m[6], m[7]);
    // Diagonal step
    cpu_blake3_g(state, 0, 5, 10, 15, m[8], m[9]);
    cpu_blake3_g(state, 1, 6, 11, 12, m[10], m[11]);
    cpu_blake3_g(state, 2, 7, 8, 13, m[12], m[13]);
    cpu_blake3_g(state, 3, 4, 9, 14, m[14], m[15]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g_zero_state_zero_message_is_identity() {
        let mut state = [0u32; 16];
        cpu_blake3_g(&mut state, 0, 4, 8, 12, 0, 0);
        // With all zeros, the rotations and XORs produce zero (XOR 0 = 0,
        // rotate 0 = 0, wrapping_add 0 = 0).
        assert_eq!(state, [0u32; 16]);
    }

    #[test]
    fn g_nonzero_input_produces_avalanche() {
        let mut state = [0u32; 16];
        state[0] = 1;
        cpu_blake3_g(&mut state, 0, 4, 8, 12, 0, 0);
        // A single bit in s0 should propagate to s0, s4, s8, s12.
        assert_ne!(state[0], 0);
        assert_ne!(state[12], 0);
    }

    #[test]
    fn round_zero_input_is_identity() {
        let mut state = [0u32; 16];
        let message = [0u32; 16];
        cpu_blake3_round(&mut state, &message, &MSG_SCHEDULE[0]);
        assert_eq!(state, [0u32; 16]);
    }

    #[test]
    fn round_nonzero_produces_diffusion() {
        let mut state = [0u32; 16];
        state[0] = 0x6A09E667; // BLAKE3 IV[0]
        let message = [0u32; 16];
        cpu_blake3_round(&mut state, &message, &MSG_SCHEDULE[0]);
        // After one round, mixing should spread the IV word.
        let nonzero_count = state.iter().filter(|&&w| w != 0).count();
        assert!(
            nonzero_count >= 4,
            "expected at least 4 nonzero words after one round, got {nonzero_count}"
        );
    }
}
