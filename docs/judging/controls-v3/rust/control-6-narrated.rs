// Import HashMap.
use std::collections::HashMap;
// Import fmt.
use std::fmt;
// Define the Record struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    // The account field.
    pub account: String,
    // The kind field.
    pub kind: String,
    // The amount_cents field.
    pub amount_cents: i64,
}
// Define the LedgerError enum.
#[derive(Debug)]
pub enum LedgerError {
    // The FieldCount variant.
    FieldCount { line: usize, got: usize },
    // The InvalidAmount variant.
    InvalidAmount { line: usize, amount: String },
    // The UnknownKind variant.
    UnknownKind(String),
}
// Implement Display for LedgerError.
impl fmt::Display for LedgerError {
    // Define fmt.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Match on self.
        match self {
            // If it is FieldCount, write the field count message.
            Self::FieldCount { line: l, got: g } => {
                write!(f, "line {l}: expected 4 fields, got {g}")
            }
            // If it is InvalidAmount, write the invalid amount message.
            Self::InvalidAmount { line: l, amount: a } => {
                write!(f, "line {l}: invalid amount: {a}")
            }
            // If it is UnknownKind, write the unknown kind message.
            Self::UnknownKind(k) => write!(f, "unknown kind: {k}"),
        }
    }
}
// Implement Error for LedgerError.
impl std::error::Error for LedgerError {}
// Define parse_records.
pub fn parse_records(t: &str) -> Result<Vec<Record>, LedgerError> {
    // Create an empty vector.
    let mut r = vec![];
    // Loop over the lines with an index.
    for (i, l) in t.lines().enumerate() {
        // Trim the line.
        let l = l.trim();
        // Continue if the line is empty or starts with #.
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        // Split the line on commas.
        let f: Vec<&str> = l.split(',').collect();
        // Return an error if the length is not 4.
        if f.len() != 4 {
            return Err(LedgerError::FieldCount {
                line: i + 1,
                got: f.len(),
            });
        }
        // Parse the fourth field into c.
        let c = f[3]
            .trim()
            .parse()
            .map_err(|_| LedgerError::InvalidAmount {
                line: i + 1,
                amount: f[3].to_string(),
            })?;
        // Push a Record of the fields.
        r.push(Record {
            account: f[1].trim().to_string(),
            kind: f[2].trim().to_string(),
            amount_cents: c,
        })
    }
    // Return the vector.
    Ok(r)
}
// Define fee_cents.
pub fn fee_cents(k: &str, a: i64) -> Result<i64, LedgerError> {
    // Match on k.
    match k {
        // If k is card return this value.
        "card" => Ok((a * 29 + 500).div_euclid(1000) + 30),
        // If k is bank return 25.
        "bank" => Ok(25),
        // If k is cash return 0.
        "cash" => Ok(0),
        // Otherwise return an error.
        _ => Err(LedgerError::UnknownKind(k.to_string())),
    }
}
// Define balances.
pub fn balances(r: &[Record]) -> Result<HashMap<String, i64>, LedgerError> {
    // Create a new HashMap.
    let mut b = HashMap::new();
    // Loop over r.
    for x in r {
        // Get the fee e.
        let e = fee_cents(&x.kind, x.amount_cents)?;
        // Add the amount minus e to the entry for the account.
        *b.entry(x.account.clone()).or_insert(0) += x.amount_cents - e
    }
    // Return b.
    Ok(b)
}
// Define format_cents.
fn format_cents(c: i64) -> String {
    // Get the absolute value m.
    let m = c.unsigned_abs();
    // Build the string d.
    let d = format!("{}.{:02}", m / 100, m % 100);
    // Return d in parentheses if c is negative else d.
    if c < 0 {
        format!("({d})")
    } else {
        d
    }
}
// Define render.
pub fn render(b: &HashMap<String, i64>) -> Vec<String> {
    // Collect b into a vector.
    let mut v: Vec<_> = b.iter().collect();
    // Sort the vector.
    v.sort();
    // Return a vector of formatted lines.
    v.into_iter()
        .map(|(a, c)| format!("{a}  {}", format_cents(*c)))
        .collect()
}
