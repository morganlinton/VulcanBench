struct Params {
    node_count: u32,
    entry: u32,
    max_iterations: u32,
};

@group(0) @binding(0) var<storage, read> successor_offsets: array<u32>;
@group(0) @binding(1) var<storage, read> successors: array<u32>;
@group(0) @binding(2) var<storage, read> predecessor_offsets: array<u32>;
@group(0) @binding(3) var<storage, read> predecessors: array<u32>;
@group(0) @binding(4) var<storage, read_write> idom: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> status: array<atomic<u32>>;
@group(0) @binding(6) var<uniform> params: Params;

var<workgroup> changed: atomic<u32>;

fn intersect(mut_a: u32, mut_b: u32) -> u32 {
    var a = mut_a;
    var b = mut_b;
    loop {
        if (a == b) { return a; }
        if (a > b) {
            a = atomicLoad(&idom[a]);
        } else {
            b = atomicLoad(&idom[b]);
        }
    }
}

@compute @workgroup_size(64)
fn compiler_primitives_dominator_tree(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(global_invocation_id) gid: vec3<u32>
) {
    let node = gid.x;
    if (node < params.node_count) {
        if (node == params.entry) {
            atomicStore(&idom[node], node);
        } else {
            atomicStore(&idom[node], 0xffffffffu);
        }
    }
    workgroupBarrier();

    var iteration = 0u;
    loop {
        if (iteration >= params.max_iterations) {
            if (lid.x == 0u) { atomicStore(&status[0], 1u); }
            return;
        }
        if (lid.x == 0u) { atomicStore(&changed, 0u); }
        workgroupBarrier();

        if (node < params.node_count && node != params.entry) {
            var new_idom = 0xffffffffu;
            var p = predecessor_offsets[node];
            let pend = predecessor_offsets[node + 1u];
            loop {
                if (p >= pend) { break; }
                let pred = predecessors[p];
                if (atomicLoad(&idom[pred]) != 0xffffffffu) {
                    if (new_idom == 0xffffffffu) {
                        new_idom = pred;
                    } else {
                        new_idom = intersect(pred, new_idom);
                    }
                }
                p = p + 1u;
            }
            if (new_idom != atomicLoad(&idom[node])) {
                atomicStore(&idom[node], new_idom);
                atomicStore(&changed, 1u);
            }
        }
        workgroupBarrier();
        iteration = iteration + 1u;
        if (atomicLoad(&changed) == 0u) {
            if (lid.x == 0u) { atomicStore(&status[0], iteration); }
            return;
        }
    }
}
