//! Pattern sharding for subgroup-sized NFA plans.

use vyre_primitives::nfa::subgroup_nfa::MAX_STATES_PER_SUBGROUP;

/// Shard a pattern set across multiple NFA plans so each shard fits
/// in [`MAX_STATES_PER_SUBGROUP`]. Greedy first-fit.
#[must_use]
pub fn plan_shards<'a>(patterns: &'a [&'a str]) -> Vec<Vec<&'a str>> {
    let mut shards: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut current_states: usize = 1;
    for p in patterns {
        let extra = p.len();
        if extra >= MAX_STATES_PER_SUBGROUP {
            if !current.is_empty() {
                shards.push(std::mem::take(&mut current));
                current_states = 1;
            }
            shards.push(vec![*p]);
            continue;
        }
        if current_states + extra > MAX_STATES_PER_SUBGROUP {
            shards.push(std::mem::take(&mut current));
            current_states = 1;
        }
        current.push(*p);
        current_states += extra;
    }
    if !current.is_empty() {
        shards.push(current);
    }
    shards
}
