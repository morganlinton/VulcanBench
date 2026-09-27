// This module is the ledger module. It handles the ledger.

use std::collections::HashMap;
use std::fmt;

// This is the record struct. It holds one record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub account: String,
    pub kind: String,
    pub amount_cents: i64,
}

// This is the error enum. It holds the errors.
#[derive(Debug)]
pub enum LedgerError {
    FieldCount { line: usize, got: usize },
    InvalidAmount { line: usize, amount: String },
    UnknownKind(String),
}

// This turns an error into text.
impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Check which error it is and write the message for it.
        match self {
            LedgerError::FieldCount { line, got } => {
                write!(f, "line {}: expected 4 fields, got {}", line, got)
            }
            LedgerError::InvalidAmount { line, amount } => {
                write!(f, "line {}: invalid amount: {}", line, amount)
            }
            LedgerError::UnknownKind(kind) => write!(f, "unknown kind: {}", kind),
        }
    }
}

// This makes the error an error.
impl std::error::Error for LedgerError {}

pub fn parse_records(text: &str) -> Result<Vec<Record>, LedgerError> {
    // First we make an empty list to hold the records.
    let mut records: Vec<Record> = Vec::new();
    // Now we go through every line.
    let mut line_number = 0;
    for raw in text.lines() {
        line_number = line_number + 1;
        let line = raw.trim();
        // Skip the line if it is blank.
        if line == "" {
            continue;
        }
        // Skip the line if it is a comment.
        if line.starts_with("#") {
            continue;
        }
        let fields: Vec<&str> = line.split(",").collect();
        // Check the number of fields.
        if fields.len() != 4 {
            return Err(LedgerError::FieldCount {
                line: line_number,
                got: fields.len(),
            });
        }
        let account = fields[1].trim().to_string();
        let kind = fields[2].trim().to_string();
        let amount: i64;
        // Turn the amount into a number.
        match fields[3].trim().parse::<i64>() {
            Ok(value) => {
                amount = value;
            }
            Err(_) => {
                return Err(LedgerError::InvalidAmount {
                    line: line_number,
                    amount: fields[3].to_string(),
                });
            }
        }
        // Validate the kind here so bad kinds are caught early.
        if kind == "card" {
        } else if kind == "bank" {
        } else if kind == "cash" {
        } else {
            return Err(LedgerError::UnknownKind(kind));
        }
        records.push(Record {
            account: account,
            kind: kind,
            amount_cents: amount,
        });
    }
    return Ok(records);
}

pub fn fee_cents(kind: &str, amount_cents: i64) -> Result<i64, LedgerError> {
    // Work out the fee for a card.
    if kind == "card" {
        let mut fee = (amount_cents * 29 + 500).div_euclid(1000);
        fee = fee + 30;
        return Ok(fee);
    }
    // Work out the fee for a bank transfer.
    if kind == "bank" {
        return Ok(25);
    }
    // Work out the fee for cash.
    if kind == "cash" {
        return Ok(0);
    }
    // The kind is not a kind we know.
    return Err(LedgerError::UnknownKind(kind.to_string()));
}

pub fn balances(records: &[Record]) -> Result<HashMap<String, i64>, LedgerError> {
    // Start with an empty map of totals.
    let mut totals: HashMap<String, i64> = HashMap::new();
    for record in records {
        let account = record.account.clone();
        let kind = record.kind.clone();
        let amount = record.amount_cents;
        let net: i64;
        if kind == "card" {
            // Work out the fee for a card.
            let mut fee = (amount * 29 + 500).div_euclid(1000);
            fee = fee + 30;
            net = amount - fee;
        } else if kind == "bank" {
            // Work out the fee for a bank transfer.
            let fee = 25;
            net = amount - fee;
        } else if kind == "cash" {
            // Work out the fee for cash.
            let fee = 0;
            net = amount - fee;
        } else {
            return Err(LedgerError::UnknownKind(kind));
        }
        // Add the net amount to the account.
        if totals.contains_key(&account) {
            let current = totals[&account];
            totals.insert(account, current + net);
        } else {
            totals.insert(account, net);
        }
    }
    return Ok(totals);
}

pub fn total_fees(records: &[Record]) -> Result<i64, LedgerError> {
    // Sum all the fees. Uses the same rules as balances.
    let mut fees = 0;
    for record in records {
        let kind = record.kind.clone();
        let amount = record.amount_cents;
        if kind == "card" {
            fees = fees + (amount * 29 + 500).div_euclid(1000) + 30;
        } else if kind == "bank" {
            fees = fees + 25;
        } else if kind == "cash" {
            fees = fees + 0;
        } else {
            return Err(LedgerError::UnknownKind(kind));
        }
    }
    return Ok(fees);
}

pub fn render(totals: &HashMap<String, i64>) -> Vec<String> {
    // Build the output lines.
    let mut lines: Vec<String> = Vec::new();
    let mut accounts: Vec<String> = totals.keys().cloned().collect();
    accounts.sort();
    for account in accounts {
        let mut cents = totals[&account];
        // Format negative numbers with parentheses.
        if cents < 0 {
            cents = -cents;
            let dollars = (cents / 100).to_string() + "." + &format!("{:02}", cents % 100);
            lines.push(account + "  (" + &dollars + ")");
        } else {
            let dollars = (cents / 100).to_string() + "." + &format!("{:02}", cents % 100);
            lines.push(account + "  " + &dollars);
        }
    }
    return lines;
}
