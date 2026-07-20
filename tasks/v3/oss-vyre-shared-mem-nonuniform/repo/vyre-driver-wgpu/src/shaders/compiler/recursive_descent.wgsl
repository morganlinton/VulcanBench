struct Params {
    token_count: u32,
    transition_count: u32,
    start_state: u32,
    accept_state: u32,
    max_stack: u32,
    max_callbacks: u32,
};

@group(0) @binding(0) var<storage, read> tokens: array<u32>;
@group(0) @binding(1) var<storage, read> transitions: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> transition_push: array<u32>;
@group(0) @binding(3) var<storage, read_write> callbacks: array<u32>;
@group(0) @binding(4) var<storage, read_write> result: array<u32>;
@group(0) @binding(5) var<uniform> params: Params;

var<workgroup> stack: array<u32, 256>;

@compute @workgroup_size(1)
fn compiler_primitives_recursive_descent() {
    var state = params.start_state;
    var sp = 0u;
    var callback_count = 0u;
    var pos = 0u;
    loop {
        if (pos >= params.token_count) { break; }
        let token = tokens[pos];
        var found = false;
        var chosen = vec4<u32>(0u);
        var chosen_push = 0xffffffffu;
        var i = 0u;
        loop {
            if (i >= params.transition_count) { break; }
            let t = transitions[i];
            if (t.x == state && t.y == token) {
                found = true;
                chosen = t;
                chosen_push = transition_push[i];
                break;
            }
            i = i + 1u;
        }
        if (!found) {
            result[0] = 1u;
            result[1] = pos;
            return;
        }
        if (chosen_push != 0xffffffffu) {
            if (sp >= params.max_stack || sp >= 256u) {
                result[0] = 2u;
                result[1] = pos;
                return;
            }
            stack[sp] = chosen_push;
            sp = sp + 1u;
        }
        if (chosen.w != 0u) {
            if (callback_count >= params.max_callbacks) {
                result[0] = 3u;
                result[1] = pos;
                return;
            }
            callbacks[callback_count] = chosen.w;
            callback_count = callback_count + 1u;
        }
        if (chosen.z == 0xffffffffu) {
            if (sp == 0u) {
                result[0] = 4u;
                result[1] = pos;
                return;
            }
            sp = sp - 1u;
            state = stack[sp];
        } else {
            state = chosen.z;
        }
        pos = pos + 1u;
    }
    if (state != params.accept_state) {
        result[0] = 5u;
        result[1] = state;
        return;
    }
    result[0] = 0u;
    result[1] = callback_count;
    result[2] = pos;
}
