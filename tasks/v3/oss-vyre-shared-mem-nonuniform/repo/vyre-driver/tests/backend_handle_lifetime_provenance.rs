//! Backend handle lifetime provenance test suite.

const HANDLES: &str =
    include_str!("../../docs/optimization/BACKEND_HANDLE_LIFETIME_PROVENANCE.toml");

#[test]
fn backend_handle_lifetime_provenance_records_origin_owner_scope_sync_release_and_guards() {
    for required in [
        "handle_id",
        "backend",
        "origin",
        "owner",
        "lifetime_scope",
        "synchronization_policy",
        "release_policy",
        "use_after_release_guard",
        "diagnostic",
        "cuda-device-buffer",
        "wgpu-buffer",
        "metal-buffer",
    ] {
        assert!(
            HANDLES.contains(required),
            "backend handle lifetime provenance must include {required}"
        );
    }
}
