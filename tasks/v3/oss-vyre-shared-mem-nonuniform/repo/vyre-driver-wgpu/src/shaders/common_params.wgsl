// Shared uniform params struct for Vyre string-matching and string-similarity kernels.
// This layout is a FROZEN contract. New fields may only be appended at the end.
// Unused fields in a given kernel are padding for shared layout.
struct Params {
    len_a: u32,    // first operand length (haystack, pattern, input, a_len)
    len_b: u32,    // second operand length / count (needle, input, b_len, state_count, n)
    param_c: u32,  // algorithm-specific parameter 1 (max_matches, output_stride_words, max_records, reserved)
    param_d: u32,  // algorithm-specific parameter 2 (output_stride_words, reserved)
}

@group(0) @binding(0) var<uniform> params: Params;
