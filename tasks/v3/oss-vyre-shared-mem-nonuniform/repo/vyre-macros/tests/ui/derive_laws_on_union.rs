use vyre_macros::AlgebraicLaws;

#[derive(AlgebraicLaws)]
#[vyre(laws = [Commutative])]
union BadLawTarget {
    bits: u32,
}

fn main() {}
