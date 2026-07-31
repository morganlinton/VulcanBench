/// docs
macro_rules! define_arith_dual_reference {
    ($marker:ident, $direct:path, $independent:path) => {
        /// Direct word-oriented binary reference over two little-endian u32 inputs.
        pub mod reference_a {
            /// Evaluate the operation using the direct word-oriented oracle.
            #[must_use]
            pub fn reference(input: &[u8]) -> Vec<u8> {
                super::super::common::binary_direct(input, $direct)
            }
        }

        /// Independent binary reference over two little-endian u32 inputs.
        pub mod reference_b {
            /// Evaluate the operation using the independent oracle.
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

/// docs
pub mod add;
mod common;
/// docs
pub mod mul;
