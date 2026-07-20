use vyre_macros::AlgebraicLaws;

#[derive(AlgebraicLaws)]
#[vyre(rulebook = [Commutative])]
pub struct BadLawAttribute;

fn main() {}
