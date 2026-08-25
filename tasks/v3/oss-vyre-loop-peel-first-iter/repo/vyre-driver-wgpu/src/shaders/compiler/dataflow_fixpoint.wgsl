struct Params {
    node_count: u32,
    max_iterations: u32,
};

@group(0) @binding(0) var<storage, read_write> state: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read> transfer: array<u32>;
@group(0) @binding(2) var<storage, read> successor_offsets: array<u32>;
@group(0) @binding(3) var<storage, read> successors: array<u32>;
@group(0) @binding(4) var<storage, read_write> out_iterations: array<atomic<u32>>;
@group(0) @binding(5) var<uniform> params: Params;

var<workgroup> changed: atomic<u32>;

@compute @workgroup_size(64)
fn compiler_primitives_dataflow_fixpoint(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(global_invocation_id) gid: vec3<u32>
) {
    var iteration = 0u;
    loop {
        if (iteration >= params.max_iterations) {
            if (lid.x == 0u) {
                atomicStore(&out_iterations[0], 0xffffffffu);
            }
            return;
        }
        if (lid.x == 0u) {
            atomicStore(&changed, 0u);
        }
        workgroupBarrier();

        let node = gid.x;
        if (node < params.node_count) {
            let propagated = atomicLoad(&state[node]) | transfer[node];
            var edge = successor_offsets[node];
            let end = successor_offsets[node + 1u];
            loop {
                if (edge >= end) { break; }
                let succ = successors[edge];
                let old = atomicOr(&state[succ], propagated);
                if ((old | propagated) != old) {
                    atomicStore(&changed, 1u);
                }
                edge = edge + 1u;
            }
        }
        workgroupBarrier();
        iteration = iteration + 1u;
        if (atomicLoad(&changed) == 0u) {
            if (lid.x == 0u) {
                atomicStore(&out_iterations[0], iteration);
            }
            return;
        }
    }
}
