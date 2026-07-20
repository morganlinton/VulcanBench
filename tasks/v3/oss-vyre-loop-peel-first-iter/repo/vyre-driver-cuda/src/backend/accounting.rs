//! CUDA-facing names for backend-neutral checked accounting primitives.

pub(crate) use vyre_driver::accounting::{
    checked_add_u64_count, checked_atomic_add_u64 as checked_add_u64,
    checked_atomic_sub_u64 as checked_sub_u64, checked_atomic_sub_usize as checked_sub_usize,
    checked_mul_u64_count, ArithmeticOverflow as CudaArithmeticOverflow,
};

#[cfg(test)]
pub(crate) use vyre_driver::accounting::{
    checked_add_u32_count, checked_add_u32_value, checked_add_u64_value, checked_add_usize_count,
    checked_add_usize_value, checked_mul_u64_value, checked_sub_u64_count, checked_sub_u64_value,
};

#[cfg(test)]
mod tests {
    use super::{
        checked_add_u32_count, checked_add_u32_value, checked_add_u64_count, checked_add_u64_value,
        checked_add_usize_count, checked_add_usize_value, checked_mul_u64_count,
        checked_mul_u64_value, checked_sub_u64_count, checked_sub_u64_value,
        CudaArithmeticOverflow,
    };

    #[derive(Debug, Eq, PartialEq)]
    enum ArithmeticError {
        Overflow(&'static str),
    }

    impl CudaArithmeticOverflow for ArithmeticError {
        fn arithmetic_overflow(field: &'static str) -> Self {
            Self::Overflow(field)
        }
    }

    #[test]
    fn checked_value_helpers_preserve_domain_errors() {
        assert_eq!(checked_add_u64_value(2, 3, "overflow"), Ok(5));
        assert_eq!(checked_mul_u64_value(2, 3, "overflow"), Ok(6));
        assert_eq!(checked_sub_u64_value(5, 3, "underflow"), Ok(2));
        assert_eq!(checked_add_usize_value(2, 3, "overflow"), Ok(5));
        assert_eq!(checked_add_u32_value(2, 3, "overflow"), Ok(5));

        assert_eq!(
            checked_add_u64_value(u64::MAX, 1, "overflow"),
            Err("overflow")
        );
        assert_eq!(
            checked_mul_u64_value(u64::MAX, 2, "overflow"),
            Err("overflow")
        );
        assert_eq!(checked_sub_u64_value(0, 1, "underflow"), Err("underflow"));
        assert_eq!(
            checked_add_usize_value(usize::MAX, 1, "overflow"),
            Err("overflow")
        );
        assert_eq!(
            checked_add_u32_value(u32::MAX, 1, "overflow"),
            Err("overflow")
        );
    }

    #[test]
    fn typed_checked_arithmetic_helpers_preserve_domain_error_fields() {
        assert_eq!(
            checked_add_u64_count::<ArithmeticError>(u64::MAX, 1, "u64 add"),
            Err(ArithmeticError::Overflow("u64 add"))
        );
        assert_eq!(
            checked_mul_u64_count::<ArithmeticError>(u64::MAX, 2, "u64 mul"),
            Err(ArithmeticError::Overflow("u64 mul"))
        );
        assert_eq!(
            checked_sub_u64_count::<ArithmeticError>(0, 1, "u64 sub"),
            Err(ArithmeticError::Overflow("u64 sub"))
        );
        assert_eq!(
            checked_add_usize_count::<ArithmeticError>(usize::MAX, 1, "usize add"),
            Err(ArithmeticError::Overflow("usize add"))
        );
        assert_eq!(
            checked_add_u32_count::<ArithmeticError>(u32::MAX, 1, "u32 add"),
            Err(ArithmeticError::Overflow("u32 add"))
        );
    }
}
