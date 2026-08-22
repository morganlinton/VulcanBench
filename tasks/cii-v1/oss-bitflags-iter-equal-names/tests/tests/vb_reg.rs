//! Hidden pass-to-pass guards: existing iteration APIs unchanged. Compiles
//! and passes at the base commit.

use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct MyFlags: u8 {
        const A = 1;
        const B = 2;
        const AB = Self::A.bits() | Self::B.bits();
    }
}

#[test]
fn vb_iter_names_still_decomposes() {
    let names: Vec<_> = (MyFlags::A | MyFlags::B).iter_names().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["A", "B"]);
}

#[test]
fn vb_contains_and_bits_unchanged() {
    let v = MyFlags::A | MyFlags::B;
    assert!(v.contains(MyFlags::A));
    assert_eq!(v.bits(), 3);
    assert_eq!(v, MyFlags::AB);
}
