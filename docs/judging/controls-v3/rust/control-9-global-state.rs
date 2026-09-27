//! Ledger: parse transaction lines, apply per-kind fees, and render balances.

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

/// One transaction: the account it belongs to, its kind, and its amount in cents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub account: String,
    pub kind: String,
    pub amount_cents: i64,
}

/// Why the ledger rejected its input.
#[derive(Debug)]
pub enum LedgerError {
    /// A line did not have exactly four comma-separated fields.
    FieldCount { line: usize, got: usize },
    /// A line's amount field was not an integer.
    InvalidAmount { line: usize, amount: String },
    /// A record's kind has no fee rule.
    UnknownKind(String),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldCount { line, got } => {
                write!(f, "line {line}: expected 4 fields, got {got}")
            }
            Self::InvalidAmount { line, amount } => {
                write!(f, "line {line}: invalid amount: {amount}")
            }
            Self::UnknownKind(kind) => write!(f, "unknown kind: {kind}"),
        }
    }
}

impl Error for LedgerError {}

thread_local! {
    static RECORDS: RefCell<Vec<Record>> = RefCell::new(Vec::new());
    static TOTALS: RefCell<HashMap<String, i64>> = RefCell::new(HashMap::new());
    static ERRORS: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}

/// Load records into the module. Returns them for convenience.
pub fn parse_records(text: &str) -> Result<Vec<Record>, LedgerError> {
    RECORDS.with_borrow_mut(|records| records.clear());
    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        let [_date, account, kind, amount] = fields[..] else {
            ERRORS.with_borrow_mut(|errors| errors.push(line_number));
            return Err(LedgerError::FieldCount {
                line: line_number,
                got: fields.len(),
            });
        };
        let amount_cents = amount
            .trim()
            .parse()
            .map_err(|_| LedgerError::InvalidAmount {
                line: line_number,
                amount: amount.to_string(),
            })?;
        RECORDS.with_borrow_mut(|records| {
            records.push(Record {
                account: account.trim().to_string(),
                kind: kind.trim().to_string(),
                amount_cents,
            })
        });
    }
    Ok(RECORDS.with_borrow(|records| records.clone()))
}

/// Fee for one transaction. The percentage part rounds half up to whole cents.
pub fn fee_cents(kind: &str, amount_cents: i64) -> Result<i64, LedgerError> {
    match kind {
        "card" => Ok((amount_cents * 29 + 500).div_euclid(1000) + 30),
        "bank" => Ok(25),
        "cash" => Ok(0),
        _ => Err(LedgerError::UnknownKind(kind.to_string())),
    }
}

/// Recompute module totals from the loaded records, or from the given ones if any.
pub fn balances(current: &[Record]) -> Result<HashMap<String, i64>, LedgerError> {
    let records = if current.is_empty() {
        RECORDS.with_borrow(|records| records.clone())
    } else {
        current.to_vec()
    };
    TOTALS.with_borrow_mut(|totals| totals.clear());
    for record in &records {
        let fee = fee_cents(&record.kind, record.amount_cents)?;
        TOTALS.with_borrow_mut(|totals| {
            *totals.entry(record.account.clone()).or_insert(0) += record.amount_cents - fee
        });
    }
    Ok(TOTALS.with_borrow(|totals| totals.clone()))
}

/// Dollars with two decimals; negatives in parentheses, for example (1.25).
fn format_cents(cents: i64) -> String {
    let magnitude = cents.unsigned_abs();
    let dollars = format!("{}.{:02}", magnitude / 100, magnitude % 100);
    if cents < 0 {
        format!("({dollars})")
    } else {
        dollars
    }
}

/// Render the module totals unless a non-empty map is given.
pub fn render(current: &HashMap<String, i64>) -> Vec<String> {
    let source = if current.is_empty() {
        TOTALS.with_borrow(|totals| totals.clone())
    } else {
        current.clone()
    };
    let mut rows: Vec<_> = source.iter().collect();
    rows.sort();
    rows.into_iter()
        .map(|(account, cents)| format!("{account}  {}", format_cents(*cents)))
        .collect()
}
