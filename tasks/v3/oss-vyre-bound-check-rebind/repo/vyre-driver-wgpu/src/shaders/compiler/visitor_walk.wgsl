struct Params {
    node_count: u32,
    root: u32,
    max_stack: u32,
};

@group(0) @binding(0) var<storage, read> child_offsets: array<u32>;
@group(0) @binding(1) var<storage, read> children: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<storage, read_write> output_count: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> status: array<atomic<u32>>;
@group(0) @binding(5) var<uniform> params: Params;

var<workgroup> stack_node: array<u32, 256>;
var<workgroup> stack_expanded: array<u32, 256>;

@compute @workgroup_size(1)
fn compiler_primitives_visitor_walk() {
    var sp = 1u;
    stack_node[0] = params.root;
    stack_expanded[0] = 0u;
    atomicStore(&output_count[0], 0u);
    loop {
        if (sp == 0u) { break; }
        sp = sp - 1u;
        let node = stack_node[sp];
        let expanded = stack_expanded[sp];
        if (node >= params.node_count) {
            atomicStore(&status[0], 1u);
            return;
        }
        if (expanded == 1u) {
            let out = atomicAdd(&output_count[0], 1u);
            output[out] = node;
        } else {
            if (sp + 1u >= params.max_stack || sp + 1u >= 256u) {
                atomicStore(&status[0], 2u);
                return;
            }
            stack_node[sp] = node;
            stack_expanded[sp] = 1u;
            sp = sp + 1u;
            var child = child_offsets[node + 1u];
            let start = child_offsets[node];
            loop {
                if (child <= start) { break; }
                child = child - 1u;
                if (sp + 1u >= params.max_stack || sp + 1u >= 256u) {
                    atomicStore(&status[0], 2u);
                    return;
                }
                stack_node[sp] = children[child];
                stack_expanded[sp] = 0u;
                sp = sp + 1u;
            }
        }
    }
    atomicStore(&status[0], 0u);
}
