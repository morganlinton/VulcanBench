use super::*;

#[test]
fn graph_wrappers_live_under_graph_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let graph_root = source_root.join("graph");

    for wrapper in GRAPH_WRAPPERS {
        let root_path = source_root.join(wrapper);
        assert!(
            !root_path.exists(),
            "graph wrapper {wrapper} must not live at src/ root; move it under src/graph/"
        );

        let graph_path = graph_root.join(wrapper);
        let graph_directory_path = wrapper
            .strip_suffix(".rs")
            .map(|stem| graph_root.join(stem).join("mod.rs"));
        assert!(
            graph_path.exists()
                || graph_directory_path
                    .as_ref()
                    .is_some_and(|path| path.exists()),
            "graph wrapper {wrapper} must live under src/graph/"
        );
    }
}

#[test]
fn consolidated_graph_wrappers_remain_primitive_backed() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("vyre-self-substrate should live directly under workspace root");
    let graph_root = manifest.join("src").join("graph");
    let primitive_graph_root = workspace.join("vyre-primitives").join("src").join("graph");

    for (wrapper, primitive_module) in CONSOLIDATED_GRAPH_WRAPPERS {
        let wrapper_path = graph_root.join(wrapper);
        let wrapper_source = read_graph_wrapper_source(&wrapper_path);
        let primitive_path = primitive_graph_root.join(wrapper);

        assert!(
            primitive_path.exists(),
            "consolidated graph wrapper {wrapper} must have a same-named primitive authority"
        );
        assert!(
            wrapper_source.contains(&format!("vyre_primitives::graph::{primitive_module}")),
            "consolidated graph wrapper {wrapper} must import vyre_primitives::graph::{primitive_module}"
        );
        assert!(
            !wrapper_source.contains("pub const OP_ID")
                && !wrapper_source.contains("pub const BATCH_OP_ID")
                && !wrapper_source.contains("pub const BATCHED_OP_ID"),
            "consolidated graph wrapper {wrapper} must not declare primitive op ids; op identity belongs in vyre-primitives"
        );
        assert!(
            !wrapper_source.contains("checked_mul(std::mem::size_of::<u32>())")
                && !wrapper_source.contains("checked_mul(core::mem::size_of::<u32>())")
                && !wrapper_source.contains("fn write_zero_words")
                && !wrapper_source.contains("fn write_padded_u32_slice_bytes")
                && !wrapper_source.contains("fn write_edge_offsets_bytes")
                && !wrapper_source.contains("fn write_padded_one_u32_bytes")
                && !wrapper_source.contains("fn write_padded_edge_bytes")
                && !wrapper_source.contains("write_zero_bytes(out, std::mem::size_of::<u32>())")
                && !wrapper_source.contains("depth * std::mem::size_of::<u32>()"),
            "consolidated graph wrapper {wrapper} must not own dispatcher byte-marshalling helpers; use hardware::dispatch_buffers"
        );
    }
}

#[test]
fn graph_wrappers_do_not_define_local_u32_byte_helpers() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let graph_root = manifest.join("src").join("graph");

    for wrapper in GRAPH_WRAPPERS {
        let wrapper_path = graph_root.join(wrapper);
        let wrapper_source = read_graph_wrapper_source(&wrapper_path);
        assert!(
            !wrapper_source.contains("fn size_of_u32"),
            "graph wrapper {wrapper} must not define local u32 byte-size helpers; use hardware::dispatch_buffers or a typed constant"
        );
    }
}

