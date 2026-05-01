use std::io::Read;

use csv::ReaderBuilder;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};

use crate::types::{ClientId, TxId};

#[derive(Debug, PartialEq, Clone)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

impl<'de> Deserialize<'de> for TransactionType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.trim().to_lowercase().as_str() {
            "deposit"    => Ok(TransactionType::Deposit),
            "withdrawal" => Ok(TransactionType::Withdrawal),
            "dispute"    => Ok(TransactionType::Dispute),
            "resolve"    => Ok(TransactionType::Resolve),
            "chargeback" => Ok(TransactionType::Chargeback),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["deposit", "withdrawal", "dispute", "resolve", "chargeback"],
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TransactionRecord {
    #[serde(rename = "type")]
    pub tx_type: TransactionType,
    #[serde(rename = "client")]
    pub client_id: ClientId,
    #[serde(rename = "tx")]
    pub tx_id: TxId,
    pub amount: Option<Decimal>,
}

pub fn stream_transactions<R: Read>(
    reader: R,
) -> impl Iterator<Item = Result<TransactionRecord, csv::Error>> {
    ReaderBuilder::new()
        .trim(csv::Trim::All)   // spec allows whitespace around field values
        .flexible(true)         // dispute/resolve/chargeback rows have no amount column
        .from_reader(reader)
        .into_deserialize()
}
