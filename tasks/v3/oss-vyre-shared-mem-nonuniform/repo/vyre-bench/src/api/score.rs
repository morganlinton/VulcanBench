use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Penalty {
    pub reason: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub valid: bool,
    pub total: f64,
    pub runtime_score: f64,
    pub compile_score: f64,
    pub allocation_score: f64,
    pub memory_score: f64,
    pub cache_score: f64,
    pub stability_score: f64,
    pub correctness_score: f64,
    pub penalties: Vec<Penalty>,
}
