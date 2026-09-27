//! Ledger: parse transaction lines, apply per-kind fees, and render balances.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

const CARD_FEE_PERMILLE: i64 = 29; // 2.9 percent of the amount
const CARD_FEE_FIXED_CENTS: i64 = 30;
const BANK_FEE_CENTS: i64 = 25;

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

/// Return the records in `text`, skipping blank lines and `#` comments.
pub fn parse_records(text: &str) -> Result<Vec<Record>, LedgerError> {
    let mut records = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        let [_date, account, kind, amount] = fields[..] else {
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
        records.push(Record {
            account: account.trim().to_string(),
            kind: kind.trim().to_string(),
            amount_cents,
        });
    }
    Ok(records)
}

/// Fee for one transaction. The percentage part rounds half up to whole cents.
pub fn fee_cents(kind: &str, amount_cents: i64) -> Result<i64, LedgerError> {
    match kind {
        "card" => {
            let percentage = (amount_cents * CARD_FEE_PERMILLE + 500).div_euclid(1000);
            Ok(percentage + CARD_FEE_FIXED_CENTS)
        }
        "bank" => Ok(BANK_FEE_CENTS),
        "cash" => Ok(0),
        _ => Err(LedgerError::UnknownKind(kind.to_string())),
    }
}

/// Sum amount minus fee for each account.
pub fn balances(records: &[Record]) -> Result<HashMap<String, i64>, LedgerError> {
    let mut totals = HashMap::new();
    for record in records {
        let fee = fee_cents(&record.kind, record.amount_cents)?;
        *totals.entry(record.account.clone()).or_insert(0) += record.amount_cents - fee;
    }
    Ok(totals)
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

pub fn render(totals: &HashMap<String, i64>) -> Vec<String> {
    let mut rows: Vec<_> = totals.iter().collect();
    rows.sort();
    rows.into_iter()
        .map(|(account, cents)| format!("{account}  {}", format_cents(*cents)))
        .collect()
}
