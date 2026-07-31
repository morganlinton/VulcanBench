//! Regression for F-IR-21 (CRITICAL): when multiple Cat-A Programs are
//! fused into one megakernel, their buffer declarations share a namespace.
//! Two programs using the same generic names ("input", "out",
//! "decoded") would alias in the fused kernel  -  the write-after-read / RW
//! collision produces silent data corruption.
//!
//! vyre-libs defends against this by rewriting generic placeholders to
//! family-scoped names via `buffer_names::scoped_generic_name`. This
//! suite asserts the rewrite is effective across every shipped hash +
//! decode family: calling each builder with the canonical generic names
//! and asserting the flattened buffer-name set is unique.
//!
//! If this test ever fails, the fix is to add the conflicting family's
//! builder to `scoped_generic_name` with a family-specific prefix, not
//! to delete the test.

#[cfg(all(feature = "hash", feature = "decode"))]
use std::collections::HashSet;

#[cfg(all(feature = "hash", feature = "decode"))]
use vyre_libs::decode::{base64_decode, hex_decode, inflate};
#[cfg(feature = "hash")]
use vyre_libs::hash::{adler32, blake3_compress, crc32, fnv1a32, fnv1a64};

#[cfg(any(feature = "hash", feature = "decode"))]
fn names_for(program: &vyre::ir::Program) -> Vec<String> {
    program
        .buffers()
        .iter()
        .map(|buf| buf.name().to_string())
        .collect()
}

#[cfg(all(feature = "hash", feature = "decode"))]
#[test]
fn cat_a_hash_and_decode_family_buffer_names_are_globally_unique() {
    let programs = vec![
        ("adler32", adler32("input", "out", 64)),
        ("crc32", crc32("input", "out", 64)),
        ("fnv1a32", fnv1a32("input", "out")),
        ("fnv1a64", fnv1a64("input", "out")),
        (
            "blake3_compress",
            blake3_compress("cv_in", "msg", "params", "cv_out"),
        ),
        ("base64_decode", base64_decode("input", "output", 64)),
        ("hex_decode", hex_decode("input", "output", 64)),
        ("inflate", inflate("input", "output", 64)),
    ];

    let mut seen: HashSet<String> = HashSet::new();
    let mut collisions: Vec<(String, String)> = Vec::new();
    let mut source: std::collections::HashMap<String, &'static str> =
        std::collections::HashMap::new();

    for (family, program) in &programs {
        let static_family: &'static str = Box::leak((*family).to_string().into_boxed_str());
        for name in names_for(program) {
            if let Some(prior) = source.get(&name) {
                collisions.push((name.clone(), format!("{prior} + {static_family}")));
            } else {
                source.insert(name.clone(), static_family);
                seen.insert(name);
            }
        }
    }

    assert!(
        collisions.is_empty(),
        "cross-family buffer-name collision in fused megakernel namespace: {collisions:?}. \
         Fix: add the offending family to buffer_names::scoped_generic_name with a unique \
         FAMILY_PREFIX so its generic aliases rewrite to `__vyre_<prefix>_<role>`.",
    );
    assert!(
        !seen.is_empty(),
        "sanity: at least one buffer must be declared"
    );
}

// ------------------------------------------------------------------
// Adversarial extensions for F-IR-21.
// ------------------------------------------------------------------

#[cfg(feature = "hash")]
#[test]
fn explicit_scoped_name_is_preserved_not_rewritten() {
    // Adversarial: a malicious or advanced consumer passes the raw
    // already-scoped name of one family as an EXPLICIT buffer name to
    // another family. The scoped_generic_name helper must recognise it
    // as an explicit (non-generic) name and preserve it unchanged.
    // If it were rewritten, the caller could not intentionally route
    // buffers across families.
    let explicit = "__vyre_hash_fnv1a64_input";
    let program = crc32(explicit, "out", 64);

    let names = names_for(&program);
    assert!(
        names.contains(&explicit.to_string()),
        "Fix: explicit scoped name `{explicit}` must be preserved, not rewritten. Got {names:?}"
    );
}

#[test]
fn empty_string_buffer_name_is_preserved_or_scoped() {
    // Adversarial: an empty-string buffer name must not cause a panic
    // or silently disappear. It is not a generic alias, so it must be
    // passed through unchanged (preserved as empty string) OR explicitly
    // rejected. We pin the "preserved" behaviour because scoped_generic_name
    // only rewrites names that match the generic alias list.
    use vyre_libs::buffer_names::scoped_generic_name;

    let result = scoped_generic_name("test_family", "input", "", &["input"]);
    assert_eq!(
        result, "",
        "Fix: empty-string explicit buffer name must be preserved, not prefixed"
    );
}
