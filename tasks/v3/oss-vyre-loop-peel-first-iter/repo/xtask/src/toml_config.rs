use serde::de::DeserializeOwned;

pub(crate) fn parse_embedded_toml<T>(path: &str, text: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    toml::from_str::<T>(text)
        .map_err(|error| format!("Fix: {path} is invalid TOML: {error}"))
}

pub(crate) fn data_or_exit<T>(result: &'static Result<T, String>) -> &'static T {
    match result {
        Ok(data) => data,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
