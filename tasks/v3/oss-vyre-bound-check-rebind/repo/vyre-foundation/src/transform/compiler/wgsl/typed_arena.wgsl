// Typed arena  -  bounded workgroup-local bump allocator.
//
// Op id: vyre_foundation::transform::compiler::typed_arena.
// Soundness: Exact  -  atomic_fetch_add returns the pre-increment
// cursor; the bounds check uses the pre-increment cursor + size so
// the overflow sentinel fires byte-identically to the CPU
// reference.
//
// Wire layout (one u32 array `arena`):
//   word 0          capacity_words
//   word 1          bump_cursor_words (atomic)
//   word 2..        payload (caller-managed)
//
// Output buffer `out_offset[0]` carries the byte offset on success
// or u32::MAX (ALLOC_OVERFLOW_SENTINEL) on overflow.

@group(0) @binding(0) var<storage, read_write> arena: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read_write> out_offset: array<u32>;

const ALLOC_OVERFLOW_SENTINEL: u32 = 0xFFFFFFFFu;

struct AllocParams {
    size_words: u32,
};

@group(0) @binding(2) var<uniform> params: AllocParams;

@compute @workgroup_size(1)
fn main() {
    let cap_words = atomicLoad(&arena[0u]);
    let prev_cursor = atomicAdd(&arena[1u], params.size_words);
    let new_cursor = prev_cursor + params.size_words;
    if (new_cursor <= cap_words) {
        out_offset[0] = prev_cursor * 4u;
    } else {
        out_offset[0] = ALLOC_OVERFLOW_SENTINEL;
    }
}
