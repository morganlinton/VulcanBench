use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuiteKind {
    Smoke,
    Release,
    Deep,
    Gpu,
    Sweep,
    CrossBackend,
    Evolve,
    Adversarial,
    Competition,
    Honest,
    Custom(&'static str),
}

impl std::str::FromStr for SuiteKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "smoke" => Ok(SuiteKind::Smoke),
            "release" => Ok(SuiteKind::Release),
            "deep" => Ok(SuiteKind::Deep),
            "gpu" => Ok(SuiteKind::Gpu),
            "sweep" => Ok(SuiteKind::Sweep),
            "cross-backend" | "cross_backend" => Ok(SuiteKind::CrossBackend),
            "evolve" => Ok(SuiteKind::Evolve),
            "adversarial" => Ok(SuiteKind::Adversarial),
            "competition" => Ok(SuiteKind::Competition),
            "honest" => Ok(SuiteKind::Honest),
            other => Ok(SuiteKind::Custom(Box::leak(
                other.to_string().into_boxed_str(),
            ))),
        }
    }
}

impl SuiteKind {
    pub fn as_str(&self) -> &str {
        match self {
            SuiteKind::Smoke => "smoke",
            SuiteKind::Release => "release",
            SuiteKind::Deep => "deep",
            SuiteKind::Gpu => "gpu",
            SuiteKind::Sweep => "sweep",
            SuiteKind::CrossBackend => "cross-backend",
            SuiteKind::Evolve => "evolve",
            SuiteKind::Adversarial => "adversarial",
            SuiteKind::Competition => "competition",
            SuiteKind::Honest => "honest",
            SuiteKind::Custom(value) => value,
        }
    }
}
