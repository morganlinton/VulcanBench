use vyre_macros::AlgebraicLaws;

#[derive(AlgebraicLaws)]
union BadLawCarrier {
    value: u32,
}

fn main() {}
