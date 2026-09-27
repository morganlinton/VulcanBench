use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub account: String,
    pub kind: String,
    pub amount_cents: i64,
}

#[derive(Debug)]
pub enum LedgerError {
    FieldCount { line: usize, got: usize },
    InvalidAmount { line: usize, amount: String },
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

#[derive(Debug, Clone, Copy)]
struct Money {
    cents: i64,
}

impl Money {
    fn new(cents: i64) -> Self {
        Self { cents }
    }

    fn minus(self, other: Money) -> Money {
        Money::new(self.cents - other.cents)
    }

    fn plus(self, other: Money) -> Money {
        Money::new(self.cents + other.cents)
    }
}

trait FeePolicy: Send + Sync {
    fn fee(&self, amount: Money) -> Money;
}

struct CardFeePolicy;

impl FeePolicy for CardFeePolicy {
    fn fee(&self, amount: Money) -> Money {
        Money::new((amount.cents * 29 + 500).div_euclid(1000) + 30)
    }
}

struct BankFeePolicy;

impl FeePolicy for BankFeePolicy {
    fn fee(&self, _amount: Money) -> Money {
        Money::new(25)
    }
}

struct CashFeePolicy;

impl FeePolicy for CashFeePolicy {
    fn fee(&self, _amount: Money) -> Money {
        Money::new(0)
    }
}

struct FeePolicyRegistry {
    policies: HashMap<String, Box<dyn FeePolicy>>,
}

impl FeePolicyRegistry {
    fn new() -> Self {
        Self {
            policies: HashMap::new(),
        }
    }

    fn register(mut self, kind: &str, policy: Box<dyn FeePolicy>) -> Self {
        self.policies.insert(kind.to_string(), policy);
        self
    }

    fn resolve(&self, kind: &str) -> Result<&dyn FeePolicy, LedgerError> {
        self.policies
            .get(kind)
            .map(|policy| policy.as_ref())
            .ok_or_else(|| LedgerError::UnknownKind(kind.to_string()))
    }
}

static REGISTRY: OnceLock<FeePolicyRegistry> = OnceLock::new();

fn registry() -> &'static FeePolicyRegistry {
    REGISTRY.get_or_init(|| {
        FeePolicyRegistry::new()
            .register("card", Box::new(CardFeePolicy))
            .register("bank", Box::new(BankFeePolicy))
            .register("cash", Box::new(CashFeePolicy))
    })
}

struct Transaction {
    account: String,
    kind: String,
    amount: Money,
}

impl From<&Record> for Transaction {
    fn from(record: &Record) -> Self {
        Self {
            account: record.account.clone(),
            kind: record.kind.clone(),
            amount: Money::new(record.amount_cents),
        }
    }
}

impl From<Transaction> for Record {
    fn from(transaction: Transaction) -> Self {
        Self {
            account: transaction.account,
            kind: transaction.kind,
            amount_cents: transaction.amount.cents,
        }
    }
}

struct RecordParser<'a> {
    text: &'a str,
}

impl<'a> RecordParser<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
    }

    fn parse(&self) -> Result<Vec<Transaction>, LedgerError> {
        self.text
            .lines()
            .enumerate()
            .filter(|(_, line)| Self::keep(line))
            .map(|(index, line)| Self::parse_line(index + 1, line))
            .collect()
    }

    fn keep(line: &str) -> bool {
        !line.trim().is_empty() && !line.trim().starts_with('#')
    }

    fn parse_line(number: usize, line: &str) -> Result<Transaction, LedgerError> {
        let fields: Vec<&str> = line.trim().split(',').collect();
        if fields.len() != 4 {
            return Err(LedgerError::FieldCount {
                line: number,
                got: fields.len(),
            });
        }
        let cents = fields[3]
            .trim()
            .parse()
            .map_err(|_| LedgerError::InvalidAmount {
                line: number,
                amount: fields[3].to_string(),
            })?;
        Ok(Transaction {
            account: fields[1].trim().to_string(),
            kind: fields[2].trim().to_string(),
            amount: Money::new(cents),
        })
    }
}

struct BalanceAggregator<'r> {
    registry: &'r FeePolicyRegistry,
    totals: HashMap<String, Money>,
}

impl<'r> BalanceAggregator<'r> {
    fn new(registry: &'r FeePolicyRegistry) -> Self {
        Self {
            registry,
            totals: HashMap::new(),
        }
    }

    fn accept(&mut self, transaction: Transaction) -> Result<&mut Self, LedgerError> {
        let fee = self
            .registry
            .resolve(&transaction.kind)?
            .fee(transaction.amount);
        let total = self
            .totals
            .entry(transaction.account)
            .or_insert(Money::new(0));
        *total = total.plus(transaction.amount.minus(fee));
        Ok(self)
    }

    fn result(&self) -> HashMap<String, i64> {
        self.totals
            .iter()
            .map(|(account, money)| (account.clone(), money.cents))
            .collect()
    }
}

struct Renderer;

impl Renderer {
    fn render(&self, totals: &HashMap<String, i64>) -> Vec<String> {
        let mut rows: Vec<_> = totals.iter().collect();
        rows.sort();
        rows.into_iter()
            .map(|(account, cents)| format!("{account}  {}", Self::money(*cents)))
            .collect()
    }

    fn money(cents: i64) -> String {
        let magnitude = cents.unsigned_abs();
        let dollars = format!("{}.{:02}", magnitude / 100, magnitude % 100);
        if cents < 0 {
            format!("({dollars})")
        } else {
            dollars
        }
    }
}

pub fn parse_records(text: &str) -> Result<Vec<Record>, LedgerError> {
    Ok(RecordParser::new(text)
        .parse()?
        .into_iter()
        .map(Record::from)
        .collect())
}

pub fn fee_cents(kind: &str, amount_cents: i64) -> Result<i64, LedgerError> {
    Ok(registry()
        .resolve(kind)?
        .fee(Money::new(amount_cents))
        .cents)
}

pub fn balances(records: &[Record]) -> Result<HashMap<String, i64>, LedgerError> {
    let mut aggregator = BalanceAggregator::new(registry());
    for record in records {
        aggregator.accept(Transaction::from(record))?;
    }
    Ok(aggregator.result())
}

pub fn render(totals: &HashMap<String, i64>) -> Vec<String> {
    Renderer.render(totals)
}
