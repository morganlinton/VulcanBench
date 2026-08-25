use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

pub(crate) fn serialize_usize<S>(
    map: &BTreeMap<Vec<usize>, usize>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let entries: Vec<(&Vec<usize>, &usize)> = map.iter().collect();
    entries.serialize(serializer)
}

pub(crate) fn deserialize_usize<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<Vec<usize>, usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Vec::<(Vec<usize>, usize)>::deserialize(deserializer)?;
    Ok(entries.into_iter().collect())
}

pub(crate) fn serialize_i64<S>(
    map: &BTreeMap<Vec<usize>, i64>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let entries: Vec<(&Vec<usize>, &i64)> = map.iter().collect();
    entries.serialize(serializer)
}

pub(crate) fn deserialize_i64<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<Vec<usize>, i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Vec::<(Vec<usize>, i64)>::deserialize(deserializer)?;
    Ok(entries.into_iter().collect())
}
