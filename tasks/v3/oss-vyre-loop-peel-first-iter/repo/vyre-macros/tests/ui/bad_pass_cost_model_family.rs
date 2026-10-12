use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad_cost",
    requires = [],
    invalidates = [],
    cost_model_family = "guesswork"
)]
pub struct BadCost;

fn main() {}
