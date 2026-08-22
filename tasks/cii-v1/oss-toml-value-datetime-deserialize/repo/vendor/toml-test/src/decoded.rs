use std::io::Read;
use std::io::Write;

/// Logical representation of any TOML value
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum DecodedValue {
    Scalar(DecodedScalar),
    Table(std::collections::HashMap<String, DecodedValue>),
    Array(Vec<DecodedValue>),
}

impl DecodedValue {
    pub fn from_slice(v: &[u8]) -> Result<Self, crate::Error> {
        serde_json::from_slice(v).map_err(|e| {
            crate::Error::new(format!(
                "failed decoding: {}\n```json\n{}\n```",
                e,
                String::from_utf8_lossy(v)
            ))
        })
    }

    pub fn to_string_pretty(&self) -> Result<String, crate::Error> {
        serde_json::to_string_pretty(self).map_err(crate::Error::new)
    }

    /// See [`Command`][crate::Command]
    pub fn from_stdin() -> Result<Self, crate::Error> {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(crate::Error::new)?;
        Self::from_slice(&buf)
    }

    /// See [`Command`][crate::Command]
    pub fn into_stdout(&self) -> Result<(), crate::Error> {
        let s = self.to_string_pretty()?;
        std::io::stdout()
            .write_all(s.as_bytes())
            .map_err(crate::Error::new)
    }
}

/// A part of [`DecodedValue`]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(tag = "type", content = "value")]
pub enum DecodedScalar {
    String(String),
    Integer(String),
    Float(String),
    Bool(String),
    Datetime(String),
    DatetimeLocal(String),
    DateLocal(String),
    TimeLocal(String),
}

impl DecodedScalar {
    pub fn as_str(&self) -> &str {
        match self {
            DecodedScalar::String(v)
            | DecodedScalar::Integer(v)
            | DecodedScalar::Float(v)
            | DecodedScalar::Bool(v)
            | DecodedScalar::Datetime(v)
            | DecodedScalar::DatetimeLocal(v)
            | DecodedScalar::DateLocal(v)
            | DecodedScalar::TimeLocal(v) => v.as_str(),
        }
    }
}

impl<'a> From<&'a str> for DecodedScalar {
    fn from(other: &'a str) -> Self {
        DecodedScalar::String(other.to_owned())
    }
}

impl<'a> From<&'a String> for DecodedScalar {
    fn from(other: &'a String) -> Self {
        DecodedScalar::String(other.clone())
    }
}

impl From<String> for DecodedScalar {
    fn from(other: String) -> Self {
        DecodedScalar::String(other)
    }
}

impl From<i64> for DecodedScalar {
    fn from(other: i64) -> Self {
        DecodedScalar::Integer(other.to_string())
    }
}

impl From<f64> for DecodedScalar {
    fn from(other: f64) -> Self {
        let s = if other.is_nan() {
            "nan".to_owned()
        } else if other.is_infinite() && other.is_sign_negative() {
            "-inf".to_owned()
        } else if other.is_infinite() && other.is_sign_positive() {
            "inf".to_owned()
        } else {
            let mut buffer = ryu::Buffer::new();
            let printed = buffer.format(other);
            printed.to_owned()
        };
        DecodedScalar::Float(s)
    }
}

impl From<bool> for DecodedScalar {
    fn from(other: bool) -> Self {
        DecodedScalar::Bool(other.to_string())
    }
}

impl PartialEq for DecodedScalar {
    fn eq(&self, other: &Self) -> bool {
        #[allow(clippy::if_same_then_else)]
        match (self, other) {
            (DecodedScalar::String(s), DecodedScalar::String(o)) => s == o,
            (DecodedScalar::Integer(s), DecodedScalar::Integer(o)) => s == o,
            (DecodedScalar::Float(s), DecodedScalar::Float(o)) => {
                if s == "inf" && o == "+inf" {
                    true
                } else if s == "+inf" && o == "inf" {
                    true
                } else if s == "nan" && o == "nan" {
                    true
                } else {
                    let s = s.parse::<f64>().unwrap();
                    let o = o.parse::<f64>().unwrap();
                    s == o
                }
            }
            (DecodedScalar::Bool(s), DecodedScalar::Bool(o)) => s == o,
            (DecodedScalar::Datetime(s), DecodedScalar::Datetime(o)) => {
                parse_date_time(s) == parse_date_time(o)
            }
            (DecodedScalar::DatetimeLocal(s), DecodedScalar::DatetimeLocal(o)) => {
                parse_date_time_local(s) == parse_date_time_local(o)
            }
            (DecodedScalar::DateLocal(s), DecodedScalar::DateLocal(o)) => {
                parse_date_local(s) == parse_date_local(o)
            }
            (DecodedScalar::TimeLocal(s), DecodedScalar::TimeLocal(o)) => {
                parse_time_local(s) == parse_time_local(o)
            }
            (_, _) => false,
        }
    }
}

fn parse_date_time(s: &str) -> chrono::DateTime<chrono::FixedOffset> {
    match normalize_datetime(s).parse() {
        Ok(d) => d,
        Err(err) => panic!("Failed to parse {s:?}: {err}"),
    }
}

fn parse_date_time_local(s: &str) -> chrono::NaiveDateTime {
    match normalize_datetime(s).parse() {
        Ok(d) => d,
        Err(err) => panic!("Failed to parse {s:?}: {err}"),
    }
}

fn parse_date_local(s: &str) -> chrono::NaiveDate {
    match s.parse() {
        Ok(d) => d,
        Err(err) => panic!("Failed to parse {s:?}: {err}"),
    }
}

fn parse_time_local(s: &str) -> chrono::NaiveTime {
    match s.parse() {
        Ok(d) => d,
        Err(err) => panic!("Failed to parse {s:?}: {err}"),
    }
}

fn normalize_datetime(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => 'T',
            't' => 'T',
            'z' => 'Z',
            _ => c,
        })
        .collect()
}

impl Eq for DecodedScalar {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn string_equality() {
        assert_eq!(DecodedScalar::from("foo"), DecodedScalar::from("foo"));
        assert_ne!(DecodedScalar::from("foo"), DecodedScalar::from("bar"));
        assert_ne!(DecodedScalar::from("42"), DecodedScalar::from(42));
        assert_ne!(DecodedScalar::from("true"), DecodedScalar::from(true));
    }

    #[test]
    fn integer_equality() {
        assert_eq!(DecodedScalar::from(42), DecodedScalar::from(42));
        assert_ne!(DecodedScalar::from(42), DecodedScalar::from(21));
        assert_ne!(DecodedScalar::from(42), DecodedScalar::from("42"));
    }

    #[test]
    fn float_equality() {
        assert_eq!(DecodedScalar::from(42.0), DecodedScalar::from(42.0));
        assert_ne!(DecodedScalar::from(42.0), DecodedScalar::from(21.0));
        assert_ne!(DecodedScalar::from(42.0), DecodedScalar::from("42.0"));
    }

    #[test]
    fn nan_equality() {
        assert_eq!(DecodedScalar::from(f64::NAN), DecodedScalar::from(f64::NAN));
        assert_eq!(
            DecodedScalar::from(f64::NAN),
            DecodedScalar::Float("nan".to_owned())
        );
        assert_ne!(DecodedScalar::from(f64::NAN), DecodedScalar::from("nan"));
    }

    #[test]
    fn inf_equality() {
        assert_eq!(
            DecodedScalar::from(f64::INFINITY),
            DecodedScalar::from(f64::INFINITY)
        );
        assert_ne!(
            DecodedScalar::from(f64::INFINITY),
            DecodedScalar::from(f64::NEG_INFINITY)
        );
        assert_eq!(
            DecodedScalar::from(f64::INFINITY),
            DecodedScalar::Float("inf".to_owned())
        );
        assert_eq!(
            DecodedScalar::from(f64::INFINITY),
            DecodedScalar::Float("+inf".to_owned())
        );
        assert_ne!(
            DecodedScalar::from(f64::INFINITY),
            DecodedScalar::from("inf")
        );
    }

    #[test]
    fn float_exp_equality() {
        assert_eq!(DecodedScalar::from(3.0e14), DecodedScalar::from(3.0e14));
        assert_eq!(
            DecodedScalar::from(3.0e14),
            DecodedScalar::Float("3.0e14".to_owned())
        );
    }

    #[test]
    fn float_binary_equality() {
        #![allow(clippy::excessive_precision)]

        // These cases are equivalent, just wanting to call out how Rust, at times, encodes the
        // number in a string.
        assert_eq!(
            DecodedScalar::from(3141.5927),
            DecodedScalar::Float("3141.5927".to_owned())
        );
        assert_eq!(
            DecodedScalar::from(3141.59270000000015),
            DecodedScalar::Float("3141.5927".to_owned())
        );
    }

    #[test]
    fn neg_inf_equality() {
        assert_eq!(
            DecodedScalar::from(f64::NEG_INFINITY),
            DecodedScalar::from(f64::NEG_INFINITY)
        );
        assert_ne!(
            DecodedScalar::from(f64::NEG_INFINITY),
            DecodedScalar::from(f64::INFINITY)
        );
        assert_eq!(
            DecodedScalar::from(f64::NEG_INFINITY),
            DecodedScalar::Float("-inf".to_owned())
        );
        assert_ne!(
            DecodedScalar::from(f64::NEG_INFINITY),
            DecodedScalar::from("-inf")
        );
    }

    #[test]
    fn bool_equality() {
        assert_eq!(DecodedScalar::from(true), DecodedScalar::from(true));
        assert_ne!(DecodedScalar::from(true), DecodedScalar::from(false));
        assert_ne!(DecodedScalar::from(true), DecodedScalar::from("true"));
    }

    #[test]
    fn datetime_equality() {
        assert_eq!(
            DecodedScalar::Datetime("1987-07-05 17:45:00Z".to_owned()),
            DecodedScalar::Datetime("1987-07-05 17:45:00Z".to_owned())
        );
        assert_eq!(
            DecodedScalar::Datetime("1987-07-05T17:45:56.123456Z".to_owned()),
            DecodedScalar::Datetime("1987-07-05T17:45:56.123456Z".to_owned()),
        );
        assert_ne!(
            DecodedScalar::Datetime("1987-07-05 17:45:00Z".to_owned()),
            DecodedScalar::Datetime("2000-07-05 17:45:00Z".to_owned())
        );
        assert_eq!(
            DecodedScalar::Datetime("1987-07-05t17:45:00z".to_owned()),
            DecodedScalar::Datetime("1987-07-05 17:45:00Z".to_owned())
        );
        assert_ne!(
            DecodedScalar::Datetime("1987-07-05 17:45:00Z".to_owned()),
            DecodedScalar::from("1987-07-05 17:45:00Z")
        );
    }

    #[test]
    fn datetime_local_equality() {
        assert_eq!(
            DecodedScalar::DatetimeLocal("1987-07-05 17:45:00".to_owned()),
            DecodedScalar::DatetimeLocal("1987-07-05 17:45:00".to_owned())
        );
        assert_eq!(
            DecodedScalar::DatetimeLocal("1987-07-05 17:45:00.444".to_owned()),
            DecodedScalar::DatetimeLocal("1987-07-05 17:45:00.444".to_owned())
        );
        assert_ne!(
            DecodedScalar::DatetimeLocal("1987-07-05 17:45:00".to_owned()),
            DecodedScalar::DatetimeLocal("2000-07-05 17:45:00".to_owned())
        );
        assert_eq!(
            DecodedScalar::DatetimeLocal("1987-07-05t17:45:00".to_owned()),
            DecodedScalar::DatetimeLocal("1987-07-05 17:45:00".to_owned())
        );
        assert_ne!(
            DecodedScalar::DatetimeLocal("1987-07-05 17:45:00".to_owned()),
            DecodedScalar::from("1987-07-05 17:45:00")
        );
    }

    #[test]
    fn date_local_equality() {
        assert_eq!(
            DecodedScalar::DateLocal("1987-07-05".to_owned()),
            DecodedScalar::DateLocal("1987-07-05".to_owned())
        );
        assert_ne!(
            DecodedScalar::DateLocal("1987-07-05".to_owned()),
            DecodedScalar::DateLocal("2000-07-05".to_owned())
        );
        assert_ne!(
            DecodedScalar::DateLocal("1987-07-05".to_owned()),
            DecodedScalar::from("1987-07-05")
        );
    }

    #[test]
    fn time_local_equality() {
        assert_eq!(
            DecodedScalar::TimeLocal("17:45:00".to_owned()),
            DecodedScalar::TimeLocal("17:45:00".to_owned())
        );
        assert_eq!(
            DecodedScalar::TimeLocal("17:45:00.444".to_owned()),
            DecodedScalar::TimeLocal("17:45:00.444".to_owned())
        );
        assert_ne!(
            DecodedScalar::TimeLocal("17:45:00".to_owned()),
            DecodedScalar::TimeLocal("19:45:00".to_owned())
        );
        assert_ne!(
            DecodedScalar::TimeLocal("17:45:00".to_owned()),
            DecodedScalar::from("17:45:00")
        );
    }
}
