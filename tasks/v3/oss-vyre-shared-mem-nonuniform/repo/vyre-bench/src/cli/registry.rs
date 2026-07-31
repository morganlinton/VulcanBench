use crate::api::case::BenchId;
use std::collections::BTreeMap;

pub(super) fn list_cases(format: &str) -> anyhow::Result<()> {
    let registry = crate::registry::collect_all();
    let metadata: Vec<_> = registry.iter().map(|case| case.metadata()).collect();
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&metadata)?);
        return Ok(());
    }
    for meta in metadata {
        println!("{} ({}) {}", meta.id.0, meta.name, meta.description);
    }
    Ok(())
}

pub(super) fn explain_case(id: &str) -> anyhow::Result<()> {
    let registry = crate::registry::collect_all();
    let case = registry
        .get(&BenchId(id.to_string()))
        .ok_or_else(|| anyhow::anyhow!("unknown benchmark `{id}`"))?;
    let mut details = BTreeMap::new();
    details.insert("metadata", serde_json::to_value(case.metadata())?);
    details.insert("requirements", serde_json::to_value(case.requirements())?);
    details.insert(
        "performance_contract",
        serde_json::to_value(case.performance_contract())?,
    );
    println!("{}", serde_json::to_string_pretty(&details)?);
    Ok(())
}
