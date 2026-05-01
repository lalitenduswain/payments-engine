use rust_decimal::Decimal;

use crate::account::AccountStore;
use crate::error::EngineError;
use crate::reader::{TransactionRecord, TransactionType};
use crate::transaction::{DisputeState, LedgerEntry, TransactionLedger};

pub struct TransactionEngine {
    accounts: AccountStore,
    ledger: TransactionLedger,
}

impl TransactionEngine {
    pub fn new() -> Self {
        TransactionEngine {
            accounts: AccountStore::default(),
            ledger: TransactionLedger::default(),
        }
    }

    /// Routes each record to the correct handler.
    /// Returns `EngineError` for ignorable business-rule violations (caller logs and continues).
    pub fn process(&mut self, record: TransactionRecord) -> Result<(), EngineError> {
        match record.tx_type {
            TransactionType::Deposit => self.apply_deposit(record),
            TransactionType::Withdrawal => self.apply_withdrawal(record),
            TransactionType::Dispute => self.apply_dispute(record),
            TransactionType::Resolve => self.apply_resolve(record),
            TransactionType::Chargeback => self.apply_chargeback(record),
        }
    }

    pub fn accounts(&self) -> &AccountStore {
        &self.accounts
    }

    fn apply_deposit(&mut self, record: TransactionRecord) -> Result<(), EngineError> {
        // A missing amount (None) defaults to zero and is caught by the guard below.
        let amount = record.amount.unwrap_or_default();
        if amount <= Decimal::ZERO {
            return Err(EngineError::NonPositiveAmount { tx_id: record.tx_id });
        }
        // Insert into ledger first — if the tx_id is a duplicate, reject the entire deposit.
        self.ledger.insert(
            record.tx_id,
            LedgerEntry {
                client_id: record.client_id,
                amount,
                dispute_state: DisputeState::Clean,
            },
        )?;
        self.accounts.get_or_create(record.client_id).credit(amount)
    }

    fn apply_withdrawal(&mut self, record: TransactionRecord) -> Result<(), EngineError> {
        // A missing amount (None) defaults to zero and is caught by the guard below.
        let amount = record.amount.unwrap_or_default();
        if amount <= Decimal::ZERO {
            return Err(EngineError::NonPositiveAmount { tx_id: record.tx_id });
        }
        // get_or_create so a first-time client who immediately withdraws gets a zero-balance account.
        self.accounts.get_or_create(record.client_id).debit(amount)
    }

    fn apply_dispute(&mut self, record: TransactionRecord) -> Result<(), EngineError> {
        let amount = self.ledger.begin_dispute(record.tx_id, record.client_id)?;
        // The account is guaranteed to exist: begin_dispute only succeeds for a deposit
        // that was previously inserted into the ledger, which always creates the account.
        // The if-let guards against ledger/account state diverging (defensive, not expected).
        if let Some(account) = self.accounts.get_mut(record.client_id) {
            account.hold(amount)?;
        }
        Ok(())
    }

    fn apply_resolve(&mut self, record: TransactionRecord) -> Result<(), EngineError> {
        let amount = self.ledger.resolve_dispute(record.tx_id, record.client_id)?;
        if let Some(account) = self.accounts.get_mut(record.client_id) {
            account.release(amount)?;
        }
        Ok(())
    }

    fn apply_chargeback(&mut self, record: TransactionRecord) -> Result<(), EngineError> {
        let amount = self.ledger.chargeback_dispute(record.tx_id, record.client_id)?;
        if let Some(account) = self.accounts.get_mut(record.client_id) {
            account.chargeback(amount);
        }
        Ok(())
    }
}

impl Default for TransactionEngine {
    fn default() -> Self {
        Self::new()
    }
}
