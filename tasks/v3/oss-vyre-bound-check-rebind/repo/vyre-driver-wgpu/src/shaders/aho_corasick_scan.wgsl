// Portable GPU kernel for string_matching.aho_corasick_scan.
//
// Bindings:
// 0 params: len_a, len_b, param_c, param_d
// 1 haystack_words: little-endian packed bytes
// 2 transitions: dense state_count * 256 table of next-state u32 values
// 3 output_offsets: state_count + 1 prefix offsets into output_records
// 4 output_records: pattern_id, pattern_len pairs
// 5 match_count: atomic u32 append counter
// 6 matches: pattern_id, start, end triples

@group(0) @binding(1) var<storage, read> haystack_words: array<u32>;
@group(0) @binding(2) var<storage, read> transitions: array<u32>;
@group(0) @binding(3) var<storage, read> output_offsets: array<u32>;
@group(0) @binding(4) var<storage, read> output_records: array<u32>;
@group(0) @binding(5) var<storage, read_write> match_count: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> matches: array<u32>;

var<workgroup> active_transition_row: array<u32, 256>;

fn byte_at(index: u32) -> u32 {
    let word = haystack_words[index >> 2u];
    let shift = (index & 3u) << 3u;
    return (word >> shift) & 0xffu;
}

fn emit_match(pattern_id: u32, start: u32, end: u32) {
    let slot = atomicAdd(&match_count, 1u);
    if (slot >= params.param_c) {
        return;
    }
    let base = slot * 3u;
    matches[base] = pattern_id;
    matches[base + 1u] = start;
    matches[base + 2u] = end;
}

@compute @workgroup_size(1, 1, 1)
fn string_matching_aho_corasick_scan(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || params.len_b == 0u) {
        return;
    }

    var state = 0u;
    var index = 0u;
    loop {
        if (index >= params.len_a) {
            break;
        }
        var row_byte = 0u;
        loop {
            if (row_byte >= 256u) {
                break;
            }
            active_transition_row[row_byte] = transitions[state * 256u + row_byte];
            row_byte = row_byte + 1u;
        }
        workgroupBarrier();

        let byte = byte_at(index);
        state = active_transition_row[byte];
        if (state >= params.len_b) {
            return;
        }

        let out_begin = output_offsets[state];
        let out_end = output_offsets[state + 1u];
        var cursor = out_begin;
        loop {
            if (cursor >= out_end) {
                break;
            }
            let rec = cursor * 2u;
            let pattern_id = output_records[rec];
            let pattern_len = output_records[rec + 1u];
            let end = index + 1u;
            if (pattern_len <= end) {
                emit_match(pattern_id, end - pattern_len, end);
            }
            cursor = cursor + 1u;
        }
        index = index + 1u;
    }
}
