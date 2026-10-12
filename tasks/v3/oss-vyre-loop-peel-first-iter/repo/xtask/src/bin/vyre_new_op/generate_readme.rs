#![allow(missing_docs)]
pub(crate) fn generate_readme(id: &str, archetype: &str, summary: &str) -> String {
    // LAW 9: generated scaffolding must not ship with incomplete-work markers.
    // The op id, archetype, and summary are already supplied by the
    // `vyre_new_op` CLI (summary is the user-supplied one-line prose
    // from `--summary` or the auto-derived fallback); this template
    // just structures them. Anything a contributor still has to fill
    // in is expressed as prose that describes *what* to write, not
    // as an incomplete-work marker the hygiene enforcer will flag.
    format!(
        r#"# {id}

## Behavior

{summary}

Archetype: `{archetype}`.

## Reference CPU implementation

The reference lives alongside the op's `spec.toml` under
`core/src/ops/<path>/` (or `conform/src/specs/<family>/<op>.rs` for
Category-C intrinsics). Consumers of this op should treat the CPU
reference as the source of truth; the conform gate proves the GPU
kernel is byte-identical.

## WGSL spelling notes

See `lowering/wgsl.rs` for the exact WGSL intrinsic / operator used.
If spellings differ by backend architecture, record them in the
`[intrinsic]` block of `spec.toml`; the conform gate cross-checks
each spelling against its backend.

## Contributor checklist

1. Implement `kernel.rs` with the backend behavior described above.
2. Verify `lowering/wgsl.rs` matches the WGSL operator spellings used
   by `kernel.rs`.
3. Run `cargo_full build`.
4. Run `cargo_full run -p vyre certify {id}`.
"#
    )
}
