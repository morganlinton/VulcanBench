extern crate self as vyre;

use vyre_macros::AlgebraicLaws;

pub mod ops {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum AlgebraicLaw {
        Commutative,
    }

    pub trait AlgebraicLawProvider {
        fn laws() -> &'static [AlgebraicLaw];
    }
}

#[derive(AlgebraicLaws)]
#[vyre(laws = [DefinitelyNotALaw])]
pub struct BadUnknownLaw;

fn main() {}
