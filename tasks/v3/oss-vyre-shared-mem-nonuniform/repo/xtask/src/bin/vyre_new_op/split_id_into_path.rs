#![allow(missing_docs)]
pub(crate) fn split_id_into_path(id: &str) -> Result<(&str, &str, &str), String> {
    let mut parts = id.split('.');
    let family = parts
        .next()
        .ok_or_else(|| format!("Fix: id `{id}` is empty"))?;
    let subfamily = parts
        .next()
        .ok_or_else(|| format!("Fix: id `{id}` must be <family>.<subfamily>.<name>"))?;
    let name = parts
        .next()
        .ok_or_else(|| format!("Fix: id `{id}` must be <family>.<subfamily>.<name>"))?;
    if parts.next().is_some() {
        return Err(format!(
            "Fix: id `{id}` must be <family>.<subfamily>.<name> for generator output."
        ));
    }
    Ok((family, subfamily, name))
}
