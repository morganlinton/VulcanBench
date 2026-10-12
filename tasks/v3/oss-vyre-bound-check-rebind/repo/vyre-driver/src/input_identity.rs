//! Exact-input identity keys shared by replay and materialized-output caches.
//!
//! Backend replay caches need the same collision-resistant,
//! tuple-boundary-preserving identity for borrowed input slice lists. The key is
//! only a hot-path filter: cache users must still retain collision-safe exact
//! byte checks before reusing bytes.

use crate::BackendError;

const DOMAIN_SEPARATED_INPUT_IDENTITY_PREFIX: &[u8] = b"vyre.input-identity.domain.v1";

/// Fixed-width exact-input identity key.
pub type ExactInputKey = [u8; 32];

fn input_identity_count(value: usize, field: &'static str) -> Result<u64, BackendError> {
    u64::try_from(value).map_err(|source| BackendError::InvalidProgram {
        fix: format!(
            "Fix: exact-input key {field} cannot fit u64 while hashing replay inputs: {source}."
        ),
    })
}

fn update_len_prefixed_bytes(
    hasher: &mut blake3::Hasher,
    bytes: &[u8],
    field: &'static str,
) -> Result<(), BackendError> {
    let byte_len = input_identity_count(bytes.len(), field)?;
    hasher.update(&byte_len.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

fn update_input_tuple(hasher: &mut blake3::Hasher, inputs: &[&[u8]]) -> Result<(), BackendError> {
    let input_count = input_identity_count(inputs.len(), "input count")?;
    hasher.update(&input_count.to_le_bytes());
    for input in inputs {
        update_len_prefixed_bytes(hasher, input, "input length")?;
    }
    Ok(())
}

/// Hash a borrowed input tuple with explicit arity and length prefixes.
///
/// # Errors
///
/// Returns [`BackendError`] when the input arity or one input length cannot fit
/// the stable `u64` hash envelope.
pub fn exact_input_key(inputs: &[&[u8]]) -> Result<ExactInputKey, BackendError> {
    let mut hasher = blake3::Hasher::new();
    update_input_tuple(&mut hasher, inputs)?;
    Ok(*hasher.finalize().as_bytes())
}

/// Hash a borrowed input tuple under an explicit cache domain and device salt.
///
/// Use this for resident/static caches that need the same tuple-boundary
/// protection as replay keys, but must not alias across different cache users,
/// logical domains, or backend feature sets.
///
/// # Errors
///
/// Returns [`BackendError`] when the domain tag is empty, the domain tag cannot
/// fit the stable `u64` envelope, or an input arity/length cannot fit.
pub fn domain_separated_exact_input_key(
    domain_tag: &[u8],
    domain_id: u64,
    feature_key: u64,
    inputs: &[&[u8]],
) -> Result<ExactInputKey, BackendError> {
    if domain_tag.is_empty() {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: exact-input domain-separated key requires a non-empty domain tag."
                .to_string(),
        });
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(DOMAIN_SEPARATED_INPUT_IDENTITY_PREFIX);
    update_len_prefixed_bytes(&mut hasher, domain_tag, "domain tag length")?;
    hasher.update(&domain_id.to_le_bytes());
    hasher.update(&feature_key.to_le_bytes());
    update_input_tuple(&mut hasher, inputs)?;
    Ok(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{domain_separated_exact_input_key, exact_input_key};

    #[test]
    fn exact_input_key_separates_tuple_boundaries_for_4096_generated_cases() {
        for seed in 0_u32..4096 {
            let left_len = ((seed.wrapping_mul(17) ^ seed.rotate_left(5)) % 31 + 1) as usize;
            let right_len = ((seed.wrapping_mul(29) ^ seed.rotate_left(9)) % 31 + 1) as usize;
            let mut state = seed ^ 0xC0DA_CAFE;
            let mut left = Vec::with_capacity(left_len);
            let mut right = Vec::with_capacity(right_len);
            for index in 0..left_len {
                state = state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223)
                    .rotate_left((index as u32) & 15);
                left.push((state ^ seed.rotate_left(index as u32 & 31)) as u8);
            }
            for index in 0..right_len {
                state = state
                    .wrapping_mul(22_695_477)
                    .wrapping_add(1)
                    .rotate_left((index as u32) & 7);
                right.push((state ^ seed.rotate_right(index as u32 & 31)) as u8);
            }
            let mut concatenated = Vec::with_capacity(left_len + right_len);
            concatenated.extend_from_slice(&left);
            concatenated.extend_from_slice(&right);

            let tuple_key = exact_input_key(&[left.as_slice(), right.as_slice()])
                .expect("Fix: generated tuple exact-input key must fit");
            let concatenated_key = exact_input_key(&[concatenated.as_slice()])
                .expect("Fix: generated concatenated exact-input key must fit");
            let empty_separated_key = exact_input_key(&[left.as_slice(), &[], right.as_slice()])
                .expect("Fix: generated empty-separated exact-input key must fit");

            assert_ne!(
                tuple_key, concatenated_key,
                "Fix: exact-input key must length-prefix slots so tuple boundaries cannot alias for generated case {seed}."
            );
            assert_ne!(
                tuple_key, empty_separated_key,
                "Fix: exact-input key must include empty input slots instead of collapsing them for generated case {seed}."
            );
        }
    }

    #[test]
    fn exact_input_key_changes_on_4096_generated_single_byte_mutations() {
        for seed in 0_u32..4096 {
            let len = ((seed.wrapping_mul(37) ^ seed.rotate_left(11)) % 96 + 1) as usize;
            let mut bytes = Vec::with_capacity(len);
            let mut state = seed ^ 0xA5A5_5A5A;
            for index in 0..len {
                state = state
                    .wrapping_mul(1_103_515_245)
                    .wrapping_add(12_345)
                    .rotate_left((index as u32) & 15);
                bytes.push((state >> ((index & 3) * 8)) as u8);
            }
            let mut mutated = bytes.clone();
            let mutation_index = (seed as usize) % len;
            mutated[mutation_index] ^= 0x80 | ((seed as u8) & 0x7f);

            let base_key = exact_input_key(&[bytes.as_slice()])
                .expect("Fix: base generated exact-input key must fit");
            let mutated_key = exact_input_key(&[mutated.as_slice()])
                .expect("Fix: mutated generated exact-input key must fit");

            assert_ne!(
                base_key, mutated_key,
                "Fix: exact-input key must change when one byte changes for generated case {seed}."
            );
        }
    }

    #[test]
    fn domain_separated_exact_input_key_preserves_domain_and_tuple_boundaries() {
        for seed in 0_u32..2048 {
            let left_len = ((seed.wrapping_mul(19) ^ seed.rotate_left(3)) % 48 + 1) as usize;
            let right_len = ((seed.wrapping_mul(41) ^ seed.rotate_left(9)) % 48 + 1) as usize;
            let mut state = seed ^ 0x1D_EA_7E5D;
            let mut left = Vec::with_capacity(left_len);
            let mut right = Vec::with_capacity(right_len);
            for index in 0..left_len {
                state = state
                    .wrapping_mul(747_796_405)
                    .wrapping_add(2_891_336_453)
                    .rotate_left((index as u32) & 15);
                left.push((state >> ((index & 3) * 8)) as u8);
            }
            for index in 0..right_len {
                state = state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223)
                    .rotate_left((index as u32) & 7);
                right.push((state ^ seed.rotate_right(index as u32 & 31)) as u8);
            }
            let mut concatenated = Vec::with_capacity(left_len + right_len);
            concatenated.extend_from_slice(&left);
            concatenated.extend_from_slice(&right);
            let domain_id = u64::from(seed) << 1;
            let feature_key = u64::from(seed.rotate_left(11)) | 1;

            let key = domain_separated_exact_input_key(
                b"generated.cache.domain",
                domain_id,
                feature_key,
                &[left.as_slice(), right.as_slice()],
            )
            .expect("Fix: generated domain-separated exact-input key must fit");
            let different_tag = domain_separated_exact_input_key(
                b"generated.cache.other",
                domain_id,
                feature_key,
                &[left.as_slice(), right.as_slice()],
            )
            .expect("Fix: generated domain tag variation must fit");
            let different_domain = domain_separated_exact_input_key(
                b"generated.cache.domain",
                domain_id ^ 0x55AA,
                feature_key,
                &[left.as_slice(), right.as_slice()],
            )
            .expect("Fix: generated domain id variation must fit");
            let different_feature = domain_separated_exact_input_key(
                b"generated.cache.domain",
                domain_id,
                feature_key.rotate_left(17),
                &[left.as_slice(), right.as_slice()],
            )
            .expect("Fix: generated feature key variation must fit");
            let concatenated_key = domain_separated_exact_input_key(
                b"generated.cache.domain",
                domain_id,
                feature_key,
                &[concatenated.as_slice()],
            )
            .expect("Fix: generated concatenated domain key must fit");

            assert_ne!(key, different_tag);
            assert_ne!(key, different_domain);
            assert_ne!(key, different_feature);
            assert_ne!(key, concatenated_key);
            assert_ne!(
                key,
                exact_input_key(&[left.as_slice(), right.as_slice()])
                    .expect("Fix: generated plain exact-input key must fit"),
                "Fix: domain-separated exact-input keys must not alias plain replay keys."
            );
        }
    }

    #[test]
    fn domain_separated_exact_input_key_rejects_empty_domain_tag() {
        let error = domain_separated_exact_input_key(&[], 0, 0, &[b"payload".as_slice()])
            .expect_err("Fix: empty cache domains must be rejected");
        assert!(
            error.to_string().contains("non-empty domain tag"),
            "Fix: empty domain tag diagnostics must explain the rejected cache-domain contract."
        );
    }
}
