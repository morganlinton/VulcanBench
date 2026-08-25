struct Params {
    string_count: u32,
    table_slots: u32,
};

@group(0) @binding(0) var<storage, read> bytes: array<u32>;
@group(0) @binding(1) var<storage, read> spans: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read_write> table_hashes: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> table_spans: array<vec2<u32>>;
@group(0) @binding(4) var<storage, read_write> intern_ids: array<u32>;
@group(0) @binding(5) var<storage, read_write> status: array<atomic<u32>>;
@group(0) @binding(6) var<uniform> params: Params;

fn fnv1a32(offset: u32, len: u32) -> u32 {
    var h = 0x811c9dc5u;
    var i = 0u;
    loop {
        if (i >= len) { break; }
        h = h ^ bytes[offset + i];
        h = h * 0x01000193u;
        i = i + 1u;
    }
    if (h == 0u) { return 1u; }
    return h;
}

fn equal_bytes(a: vec2<u32>, b: vec2<u32>) -> bool {
    if (a.y != b.y) { return false; }
    var i = 0u;
    loop {
        if (i >= a.y) { break; }
        if (bytes[a.x + i] != bytes[b.x + i]) { return false; }
        i = i + 1u;
    }
    return true;
}

@compute @workgroup_size(64)
fn compiler_primitives_string_interner(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.string_count) {
        return;
    }
    let span = spans[idx];
    let hash = fnv1a32(span.x, span.y);
    let start = hash % params.table_slots;
    var probe = 0u;
    loop {
        if (probe >= params.table_slots) {
            intern_ids[idx] = 0u;
            atomicStore(&status[0], 1u);
            return;
        }
        let slot = (start + probe) % params.table_slots;
        let prior = atomicCompareExchangeWeak(&table_hashes[slot], 0u, hash);
        if (prior.exchanged) {
            table_spans[slot] = span;
            // Ensure the non-atomic span write is visible to other invocations
            // in this workgroup before they observe the hash match.
            workgroupBarrier();
            intern_ids[idx] = slot + 1u;
            return;
        }
        if (prior.old_value == hash && equal_bytes(table_spans[slot], span)) {
            intern_ids[idx] = slot + 1u;
            return;
        }
        probe = probe + 1u;
    }
}
