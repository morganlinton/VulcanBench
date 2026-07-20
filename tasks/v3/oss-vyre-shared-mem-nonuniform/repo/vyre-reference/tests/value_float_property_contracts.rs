//! Property contracts for bit-exact floating value behavior.

use proptest::prelude::*;
use vyre_reference::value::Value;

proptest! {
    #[test]
    fn generated_float_values_are_bit_exact_for_equality(left_bits in any::<u64>(), right_bits in any::<u64>()) {
        let left = Value::Float(f64::from_bits(left_bits));
        let right = Value::Float(f64::from_bits(right_bits));

        prop_assert_eq!(left == right, left_bits == right_bits);
    }

    #[test]
    fn generated_float_bytes_preserve_all_host_bits(bits in any::<u64>()) {
        let value = Value::Float(f64::from_bits(bits));
        let bytes = value.to_bytes();
        let mut recovered = [0_u8; 8];
        recovered.copy_from_slice(&bytes);

        prop_assert_eq!(u64::from_le_bytes(recovered), bits);
    }

    #[test]
    fn generated_u32_to_f32_uses_bitcast_not_numeric_cast(bits in any::<u32>()) {
        let value = Value::U32(bits);
        let recovered = value
            .try_as_f32()
            .expect("u32 values must be interpretable as f32 bit patterns");

        prop_assert_eq!(recovered.to_bits(), bits);
    }

    #[test]
    fn generated_float_width_encoding_is_prefix_then_zero_pad(bits in any::<u64>(), width in 0usize..16) {
        let value = Value::Float(f64::from_bits(bits));
        let mut expected = f64::from_bits(bits).to_le_bytes().to_vec();

        if width != 0 {
            expected.resize(width, 0);
            expected.truncate(width);
        }

        prop_assert_eq!(value.to_bytes_width(width), expected);
    }
}
