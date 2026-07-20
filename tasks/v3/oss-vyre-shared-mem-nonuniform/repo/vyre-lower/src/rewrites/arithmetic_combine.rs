use crate::rewrites::commutative_lit_chain::{
    combine_commutative_lit_chain, CommutativeLitChainRule,
};
use crate::KernelDescriptor;
use vyre_foundation::ir::BinOp;

macro_rules! define_commutative_arithmetic_combine {
    (
        module = $module:ident,
        function = $function:ident,
        op = $op:expr,
        combine = $combine:path,
        first = $first:expr,
        second = $second:expr,
        combined = $combined:expr,
        overflow_first = $overflow_first:expr,
        overflow_second = $overflow_second:expr
    ) => {
        pub mod $module {
            use super::*;

            #[must_use]
            pub fn $function(desc: &KernelDescriptor) -> KernelDescriptor {
                combine_commutative_lit_chain(
                    desc,
                    CommutativeLitChainRule {
                        op: $op,
                        combine_literals: $combine,
                    },
                )
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use crate::rewrites::commutative_lit_chain::test_support::{
                    assert_commutative_lit_chain_contract, CommutativeLitChainContract,
                };

                #[test]
                fn satisfies_commutative_literal_chain_contract() {
                    assert_commutative_lit_chain_contract(CommutativeLitChainContract {
                        rewrite: $function,
                        op: $op,
                        combine_literals: $combine,
                        first: $first,
                        second: $second,
                        combined: $combined,
                        overflow_first: $overflow_first,
                        overflow_second: $overflow_second,
                    });
                }
            }
        }
    };
}

define_commutative_arithmetic_combine!(
    module = add_combine,
    function = add_combine,
    op = BinOp::Add,
    combine = u32::checked_add,
    first = 4,
    second = 6,
    combined = 10,
    overflow_first = 0xFFFF_FFFE,
    overflow_second = 5
);

define_commutative_arithmetic_combine!(
    module = mul_combine,
    function = mul_combine,
    op = BinOp::Mul,
    combine = u32::checked_mul,
    first = 4,
    second = 6,
    combined = 24,
    overflow_first = 0x1_0000,
    overflow_second = 0x1_0000
);
