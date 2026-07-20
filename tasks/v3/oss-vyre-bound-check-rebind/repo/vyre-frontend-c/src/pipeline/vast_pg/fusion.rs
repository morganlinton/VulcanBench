const TYPEDEF_CLASSIFY_FUSION_MAX_NODES: u32 = 4096;

pub(super) fn light_runtime_fusion_enabled(
    readback_terminal_outputs: bool,
    vast_count: u32,
) -> bool {
    !readback_terminal_outputs && vast_count.max(1) <= TYPEDEF_CLASSIFY_FUSION_MAX_NODES
}
