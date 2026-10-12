use crate::api::case::BenchCase;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct BenchRegistry {
    cases: BTreeMap<crate::api::case::BenchId, &'static dyn BenchCase>,
}

impl BenchRegistry {
    pub fn new() -> Self {
        Self {
            cases: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, case: &'static dyn BenchCase) -> Result<(), String> {
        let id = case.id();
        if self.cases.insert(id.clone(), case).is_some() {
            return Err(format!("duplicate benchmark ID: {:?}", id));
        }
        Ok(())
    }

    pub fn get(&self, id: &crate::api::case::BenchId) -> Option<&'static dyn BenchCase> {
        self.cases.get(id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static dyn BenchCase> + '_ {
        self.cases.values().copied()
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.cases.len()
    }
}

// using inventory crate for decentralized registration
inventory::collect!(&'static dyn BenchCase);

pub fn collect_all() -> BenchRegistry {
    let mut registry = BenchRegistry::new();
    for case in inventory::iter::<&'static dyn BenchCase> {
        if let Err(error) = registry.register(*case) {
            eprintln!("Fix: benchmark registry must contain unique ids: {error}");
            std::process::exit(1);
        }
    }
    registry
}
