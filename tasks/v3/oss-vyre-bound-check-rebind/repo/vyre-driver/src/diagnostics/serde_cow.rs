use std::borrow::Cow;

use serde::{Deserialize, Deserializer};

/// Deserialize helper that forces an owned `Cow<'static, str>`.
pub(crate) fn de_cow_static<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Cow<'static, str>, D::Error> {
    String::deserialize(d).map(Cow::Owned)
}

/// Deserialize helper for `Option<Cow<'static, str>>`.
pub(crate) fn de_opt_cow_static<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<Cow<'static, str>>, D::Error> {
    Option::<String>::deserialize(d).map(|opt| opt.map(Cow::Owned))
}
