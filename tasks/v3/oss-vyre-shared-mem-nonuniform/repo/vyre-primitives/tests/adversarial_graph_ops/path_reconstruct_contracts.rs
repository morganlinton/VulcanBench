use super::*;

#[test]
fn path_parent_self_loops() {
    let mut scratch = Vec::with_capacity(4);
    let len = path_cpu_ref(&[0, 1], 1, 4, &mut scratch);
    assert_eq!(len, 1);
    assert_eq!(scratch[0], 1);
    assert_eq!(&scratch[1..], &[0, 0, 0]);
}

#[test]
fn path_deep_chain() {
    let parent = &[0, 0, 1, 2, 3];
    let mut scratch = Vec::with_capacity(8);
    let len = path_cpu_ref(parent, 4, 8, &mut scratch);
    assert_eq!(len, 5);
    assert_eq!(&scratch[..5], &[4, 3, 2, 1, 0]);
}

#[test]
fn path_target_not_in_parent() {
    let mut scratch = Vec::with_capacity(4);
    let len = path_cpu_ref(&[0, 0, 1], 5, 4, &mut scratch);
    assert_eq!(len, 1);
    assert_eq!(scratch[0], 5);
}

#[test]
fn path_max_depth_zero() {
    let mut scratch = Vec::with_capacity(4);
    let len = path_cpu_ref(&[0, 0, 1, 2], 3, 0, &mut scratch);
    assert_eq!(len, 0);
    assert!(scratch.is_empty());
}

#[test]
fn path_max_depth_one() {
    let mut scratch = Vec::with_capacity(4);
    let len = path_cpu_ref(&[0, 0, 1, 2], 3, 1, &mut scratch);
    assert_eq!(len, 1);
    assert_eq!(scratch[0], 3);
}
