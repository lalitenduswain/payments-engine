use crate::types::{ClientId, TxId};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum EngineError {
    #[error("insufficient funds for client {client_id}")]
    InsufficientFunds { client_id: ClientId },

    #[error("account {client_id} is locked")]
    AccountLocked { client_id: ClientId },

    #[error("transaction {tx_id} not found in ledger")]
    TransactionNotFound { tx_id: TxId },

    #[error("transaction {tx_id} is already disputed or chargedback")]
    TransactionAlreadyDisputed { tx_id: TxId },

    #[error("transaction {tx_id} is not under dispute")]
    TransactionNotDisputed { tx_id: TxId },

    #[error("transaction {tx_id} already exists (duplicate deposit)")]
    DuplicateTransactionId { tx_id: TxId },

    #[error("dispute for transaction {tx_id} belongs to a different client")]
    ClientMismatch { tx_id: TxId },

    #[error("transaction {tx_id} has a non-positive amount")]
    NonPositiveAmount { tx_id: TxId },
}
