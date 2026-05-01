use std::collections::HashMap;

use crate::error::EngineError;
use crate::types::{Amount, ClientId, TxId};

#[derive(Debug, Clone, PartialEq)]
pub enum DisputeState {
    Clean,
    Disputed,
    Resolved,
    Chargedback,
}

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub client_id: ClientId,
    pub amount: Amount,
    pub dispute_state: DisputeState,
}

#[derive(Debug, Default)]
pub struct TransactionLedger {
    entries: HashMap<TxId, LedgerEntry>,
}

impl TransactionLedger {
    pub fn insert(&mut self, tx_id: TxId, entry: LedgerEntry) -> Result<(), EngineError> {
        if self.entries.contains_key(&tx_id) {
            return Err(EngineError::DuplicateTransactionId { tx_id });
        }
        self.entries.insert(tx_id, entry);
        Ok(())
    }

    /// Transitions: Clean | Resolved → Disputed
    pub fn begin_dispute(
        &mut self,
        tx_id: TxId,
        client_id: ClientId,
    ) -> Result<Amount, EngineError> {
        let entry = self
            .entries
            .get_mut(&tx_id)
            .ok_or(EngineError::TransactionNotFound { tx_id })?;

        if entry.client_id != client_id {
            return Err(EngineError::ClientMismatch { tx_id });
        }

        match entry.dispute_state {
            DisputeState::Clean | DisputeState::Resolved => {
                entry.dispute_state = DisputeState::Disputed;
                Ok(entry.amount)
            }
            DisputeState::Disputed | DisputeState::Chargedback => {
                Err(EngineError::TransactionAlreadyDisputed { tx_id })
            }
        }
    }

    /// Transitions: Disputed → Resolved
    pub fn resolve_dispute(
        &mut self,
        tx_id: TxId,
        client_id: ClientId,
    ) -> Result<Amount, EngineError> {
        let entry = self
            .entries
            .get_mut(&tx_id)
            .ok_or(EngineError::TransactionNotFound { tx_id })?;

        if entry.client_id != client_id {
            return Err(EngineError::ClientMismatch { tx_id });
        }

        match entry.dispute_state {
            DisputeState::Disputed => {
                entry.dispute_state = DisputeState::Resolved;
                Ok(entry.amount)
            }
            _ => Err(EngineError::TransactionNotDisputed { tx_id }),
        }
    }

    /// Transitions: Disputed → Chargedback (terminal)
    pub fn chargeback_dispute(
        &mut self,
        tx_id: TxId,
        client_id: ClientId,
    ) -> Result<Amount, EngineError> {
        let entry = self
            .entries
            .get_mut(&tx_id)
            .ok_or(EngineError::TransactionNotFound { tx_id })?;

        if entry.client_id != client_id {
            return Err(EngineError::ClientMismatch { tx_id });
        }

        match entry.dispute_state {
            DisputeState::Disputed => {
                entry.dispute_state = DisputeState::Chargedback;
                Ok(entry.amount)
            }
            _ => Err(EngineError::TransactionNotDisputed { tx_id }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn deposit(client_id: ClientId, amount: Amount) -> LedgerEntry {
        LedgerEntry {
            client_id,
            amount,
            dispute_state: DisputeState::Clean,
        }
    }

    #[test]
    fn insert_and_retrieve_entry() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        let entry = ledger.entries.get(&1).unwrap();
        assert_eq!(entry.amount, dec!(100.0));
        assert_eq!(entry.dispute_state, DisputeState::Clean);
    }

    #[test]
    fn duplicate_insert_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        let err = ledger.insert(1, deposit(1, dec!(50.0))).unwrap_err();
        assert_eq!(err, EngineError::DuplicateTransactionId { tx_id: 1 });
    }

    #[test]
    fn begin_dispute_transitions_clean_to_disputed() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        let amount = ledger.begin_dispute(1, 1).unwrap();
        assert_eq!(amount, dec!(100.0));
        assert_eq!(ledger.entries[&1].dispute_state, DisputeState::Disputed);
    }

    #[test]
    fn dispute_wrong_client_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        let err = ledger.begin_dispute(1, 2).unwrap_err();
        assert_eq!(err, EngineError::ClientMismatch { tx_id: 1 });
    }

    #[test]
    fn double_dispute_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        ledger.begin_dispute(1, 1).unwrap();
        let err = ledger.begin_dispute(1, 1).unwrap_err();
        assert_eq!(err, EngineError::TransactionAlreadyDisputed { tx_id: 1 });
    }

    #[test]
    fn resolve_transitions_disputed_to_resolved() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        ledger.begin_dispute(1, 1).unwrap();
        let amount = ledger.resolve_dispute(1, 1).unwrap();
        assert_eq!(amount, dec!(100.0));
        assert_eq!(ledger.entries[&1].dispute_state, DisputeState::Resolved);
    }

    #[test]
    fn resolve_wrong_client_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        ledger.begin_dispute(1, 1).unwrap();
        let err = ledger.resolve_dispute(1, 2).unwrap_err();
        assert_eq!(err, EngineError::ClientMismatch { tx_id: 1 });
        // ledger entry must still be Disputed after the rejected resolve
        assert_eq!(ledger.entries[&1].dispute_state, DisputeState::Disputed);
    }

    #[test]
    fn resolve_clean_tx_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        let err = ledger.resolve_dispute(1, 1).unwrap_err();
        assert_eq!(err, EngineError::TransactionNotDisputed { tx_id: 1 });
    }

    #[test]
    fn chargeback_transitions_disputed_to_chargedback() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        ledger.begin_dispute(1, 1).unwrap();
        let amount = ledger.chargeback_dispute(1, 1).unwrap();
        assert_eq!(amount, dec!(100.0));
        assert_eq!(ledger.entries[&1].dispute_state, DisputeState::Chargedback);
    }

    #[test]
    fn chargeback_wrong_client_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        ledger.begin_dispute(1, 1).unwrap();
        let err = ledger.chargeback_dispute(1, 2).unwrap_err();
        assert_eq!(err, EngineError::ClientMismatch { tx_id: 1 });
        // ledger entry must still be Disputed after the rejected chargeback
        assert_eq!(ledger.entries[&1].dispute_state, DisputeState::Disputed);
    }

    #[test]
    fn chargeback_clean_tx_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        let err = ledger.chargeback_dispute(1, 1).unwrap_err();
        assert_eq!(err, EngineError::TransactionNotDisputed { tx_id: 1 });
    }

    #[test]
    fn re_dispute_after_resolve_is_allowed() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        ledger.begin_dispute(1, 1).unwrap();
        ledger.resolve_dispute(1, 1).unwrap();
        // re-dispute after resolution is valid
        let amount = ledger.begin_dispute(1, 1).unwrap();
        assert_eq!(amount, dec!(100.0));
        assert_eq!(ledger.entries[&1].dispute_state, DisputeState::Disputed);
    }

    #[test]
    fn re_dispute_after_chargeback_rejected() {
        let mut ledger = TransactionLedger::default();
        ledger.insert(1, deposit(1, dec!(100.0))).unwrap();
        ledger.begin_dispute(1, 1).unwrap();
        ledger.chargeback_dispute(1, 1).unwrap();
        let err = ledger.begin_dispute(1, 1).unwrap_err();
        assert_eq!(err, EngineError::TransactionAlreadyDisputed { tx_id: 1 });
    }

    #[test]
    fn dispute_nonexistent_tx_returns_not_found() {
        let mut ledger = TransactionLedger::default();
        let err = ledger.begin_dispute(999, 1).unwrap_err();
        assert_eq!(err, EngineError::TransactionNotFound { tx_id: 999 });
    }
}
