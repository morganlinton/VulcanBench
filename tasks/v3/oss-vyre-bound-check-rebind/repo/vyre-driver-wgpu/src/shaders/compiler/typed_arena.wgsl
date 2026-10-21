struct Params {
    capacity_words: u32,
    op_count: u32,
};

@group(0) @binding(0) var<storage, read> sizes: array<u32>;
@group(0) @binding(1) var<storage, read_write> offsets: array<u32>;
@group(0) @binding(2) var<storage, read_write> status: array<atomic<u32>>;
@group(0) @binding(3) var<uniform> params: Params;

var<workgroup> bump_words: atomic<u32>;

fn align_words(size_bytes: u32) -> u32 {
    return (size_bytes + 3u) / 4u;
}

@compute @workgroup_size(64)
fn compiler_primitives_typed_arena(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(global_invocation_id) gid: vec3<u32>
) {
    if (lid.x == 0u) {
        atomicStore(&bump_words, 0u);
        atomicStore(&status[0], 0u);
    }
    workgroupBarrier();

    let op_index = gid.x;
    if (op_index >= params.op_count) {
        return;
    }

    let size = sizes[op_index];
    if (size == 0xffffffffu) {
        if (lid.x == 0u) {
            atomicStore(&bump_words, 0u);
        }
        workgroupBarrier();
        offsets[op_index] = 0xffffffffu;
        return;
    }

    if (size > 0xfffffffcu) {
        offsets[op_index] = 0xffffffffu;
        atomicStore(&status[0], 1u);
        return;
    }

    let words = align_words(size);
    let start = atomicAdd(&bump_words, words);
    let end = start + words;
    if (end > params.capacity_words || end < start) {
        offsets[op_index] = 0xffffffffu;
        atomicStore(&status[0], 2u);
        return;
    }
    offsets[op_index] = start * 4u;
}
