//! Hidden fail-to-pass tests: Flags::iter_equal_names — every defined name
//! whose flags value equals this value, aliases included. Does not compile at
//! the base commit (the method does not exist).

use bitflags::{bitflags, Flags};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct MyFlags: u8 {
        const A = 1;
        const B = 2;
        const AB = Self::A.bits() | Self::B.bits();
        const ALIAS_A = 1;
    }
}

#[test]
fn vb_single_flag_with_alias_yields_both_names() {
    let names: Vec<_> = MyFlags::A.iter_equal_names().collect();
    assert_eq!(names, vec!["A", "ALIAS_A"]);
}

#[test]
fn vb_convenience_flag_name_is_yielded() {
    let names: Vec<_> = (MyFlags::A | MyFlags::B).iter_equal_names().collect();
    assert_eq!(names, vec!["AB"]);
}

#[test]
fn vb_unnamed_combination_yields_nothing() {
    bitflags! {
        #[derive(Clone, Copy)]
        struct Other: u8 { const X = 1; const Y = 2; }
    }
    let names: Vec<_> = (Other::X | Other::Y).iter_equal_names().collect();
    assert!(names.is_empty());
}

#[test]
fn vb_empty_value_yields_nothing_unless_defined() {
    let names: Vec<_> = MyFlags::empty().iter_equal_names().collect();
    assert!(names.is_empty());
}
