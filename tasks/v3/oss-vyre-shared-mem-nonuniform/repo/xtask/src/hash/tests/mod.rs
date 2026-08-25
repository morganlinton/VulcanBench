//! Deep-sweep fix: the file was `hash/tests.rs` but wrapped its one
//! test in a redundant inner `mod tests { ... }`, which shifted
//! `super` from the parent `hash` module to the file module, so
//! `super::sha256_hex` resolved to nothing. `hash::tests::tests`
//! was dead. The inner wrapper is gone; the file is cfg-gated at
//! the parent declaration (`mod tests;` is already inside
//! `#[cfg(test)]` scope via the xtask test harness), so a bare
//! `#[test]` is enough.

use super::sha256_hex;

#[test]
fn known_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
