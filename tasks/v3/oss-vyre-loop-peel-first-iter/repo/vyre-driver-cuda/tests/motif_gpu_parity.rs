//! Parity test: GPU motif matching matches the reference oracle.

#![cfg(test)]

mod common;

use common::with_cuda_optimizer_dispatcher;
use vyre_primitives::graph::motif::MotifEdge;
use vyre_self_substrate::motif::{match_motif as reference_match_motif, match_motif_via};

#[test]
fn cuda_match_motif_via_triangle_full_match() {
    // 0 -> 1 -> 2 -> 0, all kind 1.
    let edge_offsets = vec![0u32, 1, 2, 3];
    let edge_targets = vec![1u32, 2, 0];
    let edge_kind_mask = vec![1u32, 1, 1];
    let motif = vec![
        MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 1,
        },
        MotifEdge {
            from: 1,
            kind_mask: 1,
            to: 2,
        },
        MotifEdge {
            from: 2,
            kind_mask: 1,
            to: 0,
        },
    ];
    with_cuda_optimizer_dispatcher("triangle motif", |dispatcher| {
        let gpu = match_motif_via(
            dispatcher,
            3,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &motif,
        )
        .expect("dispatch");
        let reference =
            reference_match_motif(3, &edge_offsets, &edge_targets, &edge_kind_mask, &motif);
        assert_eq!(gpu, reference);
    });
}

#[test]
fn cuda_match_motif_via_partial_match_returns_zero() {
    let edge_offsets = vec![0u32, 1, 2, 3];
    let edge_targets = vec![1u32, 2, 0];
    let edge_kind_mask = vec![1u32, 1, 1];
    let motif = vec![
        MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 1,
        }, // exists
        MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 2,
        }, // missing
    ];
    with_cuda_optimizer_dispatcher("partial motif", |dispatcher| {
        let gpu = match_motif_via(
            dispatcher,
            3,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &motif,
        )
        .expect("dispatch");
        let reference =
            reference_match_motif(3, &edge_offsets, &edge_targets, &edge_kind_mask, &motif);
        assert_eq!(gpu, reference);
        assert_eq!(gpu, vec![0, 0, 0]);
    });
}

#[test]
fn cuda_match_motif_via_kind_mask_filter() {
    let edge_offsets = vec![0u32, 1, 1];
    let edge_targets = vec![1u32];
    let edge_kind_mask = vec![0b0010u32];
    // Demand kind bit 0  -  graph has only kind bit 1  -  no match.
    let motif = vec![MotifEdge {
        from: 0,
        kind_mask: 0b0001,
        to: 1,
    }];
    with_cuda_optimizer_dispatcher("kind-mask motif", |dispatcher| {
        let gpu = match_motif_via(
            dispatcher,
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &motif,
        )
        .expect("dispatch");
        let reference =
            reference_match_motif(2, &edge_offsets, &edge_targets, &edge_kind_mask, &motif);
        assert_eq!(gpu, reference);
        assert_eq!(gpu, vec![0, 0]);
    });
}
