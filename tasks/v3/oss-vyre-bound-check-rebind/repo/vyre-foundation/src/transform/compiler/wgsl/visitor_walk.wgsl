// Visitor walk  -  bounded post-order tree traversal primitive.
//
// Op id: vyre_foundation::transform::compiler::visitor_walk.
// Soundness: Exact  -  explicit-stack iterative post-order walk over
// a CSR child table; the GPU pop order matches the CPU reference
// byte-for-byte so conform can prove identical post_order output.
//
// Buffers:
//   stack          [0] = top-of-stack index, [1..] = contents
//   child_offsets  CSR offsets into `children`
//   children       flat child list
//   post_order     popped nodes, in pop order
//   post_count     [0] = number of entries written to post_order

@group(0) @binding(0) var<storage, read>       child_offsets: array<u32>;
@group(0) @binding(1) var<storage, read>       children:      array<u32>;
@group(0) @binding(2) var<storage, read_write> stack:         array<u32>;
@group(0) @binding(3) var<storage, read_write> post_order:    array<u32>;
@group(0) @binding(4) var<storage, read_write> post_count:    array<u32>;

const VISIT_STACK_EMPTY: u32 = 0xFFFFFFFFu;

@compute @workgroup_size(1)
fn main() {
    let top = stack[0u];
    if (top == 0u) {
        return;
    }
    let node = stack[top];
    let new_top = top - 1u;
    stack[0u] = new_top;

    let count = post_count[0u];
    post_order[count] = node;
    post_count[0u] = count + 1u;

    let begin = child_offsets[node];
    let end = child_offsets[node + 1u];
    var cursor = end;
    var write_top = new_top;
    loop {
        if (cursor <= begin) {
            break;
        }
        cursor = cursor - 1u;
        write_top = write_top + 1u;
        stack[write_top] = children[cursor];
    }
    stack[0u] = write_top;
}
