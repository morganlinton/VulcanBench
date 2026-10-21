use super::*;

#[test]
fn toposort_single_node() {
    assert_eq!(toposort(1, &[]), Ok(vec![0]));
}

#[test]
fn toposort_self_loops_rejected() {
    let err = toposort(3, &[(0, 0), (1, 1), (2, 2)]).expect_err("self-loops are cycles");
    assert!(matches!(err, ToposortError::Cycle { .. }));
}

#[test]
fn toposort_disconnected_components() {
    let got = toposort(4, &[(0, 1), (2, 3)]).unwrap();
    assert_eq!(got.len(), 4);
    let pos = |v: u32| got.iter().position(|&x| x == v).unwrap();
    assert!(pos(1) < pos(0));
    assert!(pos(3) < pos(2));
}

#[test]
fn toposort_large_graph_cycle_diagnostic() {
    let mut edges: Vec<(u32, u32)> = (0..99).map(|i| (i, i + 1)).collect();
    edges.push((99, 50));
    let err = toposort(100, &edges).expect_err("cycle must be detected");
    match err {
        ToposortError::Cycle { node } => {
            assert!((50..=99).contains(&node));
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
}
