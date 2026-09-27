use std::collections::HashMap;use std::fmt;
#[derive(Debug,Clone,PartialEq,Eq)]pub struct Record{pub account:String,pub kind:String,pub amount_cents:i64}
#[derive(Debug)]pub enum LedgerError{FieldCount{line:usize,got:usize},InvalidAmount{line:usize,amount:String},UnknownKind(String)}
impl fmt::Display for LedgerError{fn fmt(&self,f:&mut fmt::Formatter)->fmt::Result{match self{
Self::FieldCount{line:l,got:g}=>write!(f,"line {l}: expected 4 fields, got {g}"),Self::InvalidAmount{line:l,amount:a}=>write!(f,"line {l}: invalid amount: {a}"),Self::UnknownKind(k)=>write!(f,"unknown kind: {k}")}}}
impl std::error::Error for LedgerError{}
pub fn parse_records(t:&str)->Result<Vec<Record>,LedgerError>{let mut r=vec![];
for(i,l)in t.lines().enumerate(){let l=l.trim();if l.is_empty()||l.starts_with('#'){continue}
let f:Vec<&str>=l.split(',').collect();if f.len()!=4{return Err(LedgerError::FieldCount{line:i+1,got:f.len()})}
let c=f[3].trim().parse().map_err(|_|LedgerError::InvalidAmount{line:i+1,amount:f[3].to_string()})?;r.push(Record{account:f[1].trim().to_string(),kind:f[2].trim().to_string(),amount_cents:c})}
Ok(r)}
pub fn fee_cents(k:&str,a:i64)->Result<i64,LedgerError>{match k{"card"=>Ok((a*29+500).div_euclid(1000)+30),"bank"=>Ok(25),"cash"=>Ok(0),_=>Err(LedgerError::UnknownKind(k.to_string()))}}
pub fn balances(r:&[Record])->Result<HashMap<String,i64>,LedgerError>{let mut b=HashMap::new();
for x in r{let e=fee_cents(&x.kind,x.amount_cents)?;*b.entry(x.account.clone()).or_insert(0)+=x.amount_cents-e}Ok(b)}
fn format_cents(c:i64)->String{let m=c.unsigned_abs();let d=format!("{}.{:02}",m/100,m%100);if c<0{format!("({d})")}else{d}}
pub fn render(b:&HashMap<String,i64>)->Vec<String>{let mut v:Vec<_>=b.iter().collect();v.sort();v.into_iter().map(|(a,c)|format!("{a}  {}",format_cents(*c))).collect()}
