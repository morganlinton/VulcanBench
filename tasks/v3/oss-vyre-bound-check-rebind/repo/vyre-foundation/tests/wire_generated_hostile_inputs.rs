//! Deterministic hostile corpus for `Program::from_wire`.
//!
//! This is not a replacement for cargo-fuzz. It is an always-available,
//! generated regression corpus that feeds thousands of malformed byte streams
//! through the public decoder and asserts the decoder never panics or silently
//! produces an un-reencodable program.

mod wire_decode_support;

use wire_decode_support::{
    assert_decode_is_safe, hostile_bytes, minimal_program_bytes, mutation_dictionary, next_u64,
};

#[test]
fn generated_hostile_wire_inputs_never_panic_or_decode_to_unencodable_programs() {
    for seed in 0..8192_u64 {
        let bytes = hostile_bytes(seed);
        assert_decode_is_safe(&bytes, &format!("hostile wire seed {seed}"));
    }
}

#[test]
fn generated_single_byte_mutations_of_valid_wire_never_panic() {
    let valid = minimal_program_bytes();
    for index in 0..valid.len().min(4096) {
        for delta in [1_u8, 0x7f, 0x80, 0xff] {
            let mut mutated = valid.clone();
            mutated[index] = mutated[index].wrapping_add(delta);
            assert_decode_is_safe(
                &mutated,
                &format!("valid-wire mutation byte {index} delta {delta}"),
            );
        }
    }
}

#[test]
fn generated_all_valid_wire_prefixes_never_panic() {
    let valid = minimal_program_bytes();
    for keep in 0..=valid.len() {
        assert_decode_is_safe(&valid[..keep], &format!("valid-wire prefix length {keep}"));
    }
}

#[test]
fn generated_every_bit_flip_of_valid_wire_never_panics() {
    let valid = minimal_program_bytes();
    for index in 0..valid.len().min(4096) {
        for bit in 0..8 {
            let mut mutated = valid.clone();
            mutated[index] ^= 1 << bit;
            assert_decode_is_safe(
                &mutated,
                &format!("valid-wire bit flip byte {index} bit {bit}"),
            );
        }
    }
}

#[test]
fn generated_two_site_wire_mutations_never_panic() {
    let valid = minimal_program_bytes();
    let valid_len = valid.len().max(1);

    for seed in 0..16384_u64 {
        let mut mutated = valid.clone();
        let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
        let first = (next_u64(&mut state) as usize) % valid_len;
        let second = (next_u64(&mut state) as usize) % valid_len;
        let first_delta = next_u64(&mut state) as u8;
        let second_mask = (next_u64(&mut state) as u8).max(1);

        mutated[first] = mutated[first].wrapping_add(first_delta);
        mutated[second] ^= second_mask;
        assert_decode_is_safe(&mutated, &format!("two-site wire mutation seed {seed}"));
    }
}

#[test]
fn generated_structured_splice_delete_dictionary_mutations_never_panic() {
    let valid = minimal_program_bytes();
    let dictionary = mutation_dictionary(&valid);
    let valid_len = valid.len().max(1);

    for seed in 0..8192_u64 {
        let mut mutated = valid.clone();
        let mut state = seed ^ 0x510e_527f_ade6_82d1;
        let position = (next_u64(&mut state) as usize) % valid_len;
        let span =
            ((next_u64(&mut state) as usize) % 32).min(mutated.len().saturating_sub(position));
        let token = dictionary[(next_u64(&mut state) as usize) % dictionary.len()].as_slice();

        match seed % 6 {
            0 => {
                mutated.drain(position..position + span);
            }
            1 => {
                mutated.splice(position..position + span, token.iter().copied());
            }
            2 => {
                mutated.splice(position..position, token.iter().copied());
            }
            3 => {
                let mut hostile = hostile_bytes(seed ^ 0xA5A5_5A5A);
                let hostile_keep = (next_u64(&mut state) as usize) % (hostile.len().max(1));
                hostile.truncate(hostile_keep);
                mutated.splice(position..position + span, hostile);
            }
            4 => {
                mutated.extend_from_slice(token);
            }
            _ => {
                mutated.clear();
                mutated.extend_from_slice(token);
                mutated.extend_from_slice(&valid[position..]);
            }
        }

        assert_decode_is_safe(&mutated, &format!("structured wire mutation seed {seed}"));
    }
}
