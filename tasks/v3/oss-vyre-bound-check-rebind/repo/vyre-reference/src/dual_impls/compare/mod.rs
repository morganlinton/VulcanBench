macro_rules! define_compare_dual_reference {
    ($marker:ident, $direct:expr, $independent:path) => {
        /// Direct word-oriented comparison reference over two little-endian u32 inputs.
        pub mod reference_a {
            /// Evaluate the comparison using the direct word-oriented oracle.
            #[must_use]
            pub fn reference(input: &[u8]) -> Vec<u8> {
                super::super::common::binary_direct_predicate(input, $direct)
            }
        }

        /// Independent comparison reference over two little-endian u32 inputs.
        pub mod reference_b {
            /// Evaluate the comparison using the independent oracle.
            #[must_use]
            pub fn reference(input: &[u8]) -> Vec<u8> {
                $independent(input)
            }
        }

        impl crate::dual::DualReference for $marker {
            fn reference_a(input: &[u8]) -> Vec<u8> {
                reference_a::reference(input)
            }

            fn reference_b(input: &[u8]) -> Vec<u8> {
                reference_b::reference(input)
            }
        }
    };
}

mod common;
/// docs
pub mod eq;
/// docs
pub mod lt;
