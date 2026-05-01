use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::error::EngineError;
use crate::types::{Amount, ClientId};

#[derive(Debug, Clone)]
pub struct Account {
    pub client_id: ClientId,
    pub available: Amount,
    pub held: Amount,
    pub locked: bool,
}

impl Account {
    pub fn new(client_id: ClientId) -> Self {
        Account {
            client_id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn total(&self) -> Amount {
        self.available + self.held
    }

    pub fn credit(&mut self, amount: Amount) -> Result<(), EngineError> {
        self.check_unlocked()?;
        self.available += amount;
        Ok(())
    }

    pub fn debit(&mut self, amount: Amount) -> Result<(), EngineError> {
        self.check_unlocked()?;
        if self.available < amount {
            return Err(EngineError::InsufficientFunds {
                client_id: self.client_id,
            });
        }
        self.available -= amount;
        Ok(())
    }

    pub fn hold(&mut self, amount: Amount) -> Result<(), EngineError> {
        self.check_unlocked()?;
        // No lower-bound check: a dispute for a deposit already partially withdrawn
        // is still valid. Available can go negative, reflecting a bank deficit.
        self.available -= amount;
        self.held += amount;
        Ok(())
    }

    pub fn release(&mut self, amount: Amount) -> Result<(), EngineError> {
        self.check_unlocked()?;
        self.held -= amount;
        self.available += amount;
        Ok(())
    }

    pub fn chargeback(&mut self, amount: Amount) {
        // No locked check: once an account is locked, hold() rejects all future disputes,
        // so a second Disputed tx can never exist. This method is therefore unreachable
        // on a locked account — the invariant is enforced at the engine level.
        self.held -= amount;
        self.locked = true;
    }

    fn check_unlocked(&self) -> Result<(), EngineError> {
        if self.locked {
            Err(EngineError::AccountLocked {
                client_id: self.client_id,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Default)]
pub struct AccountStore {
    accounts: HashMap<ClientId, Account>,
}

impl AccountStore {
    pub fn get_or_create(&mut self, client_id: ClientId) -> &mut Account {
        self.accounts
            .entry(client_id)
            .or_insert_with(|| Account::new(client_id))
    }

    pub fn get_mut(&mut self, client_id: ClientId) -> Option<&mut Account> {
        self.accounts.get_mut(&client_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn account(id: u16) -> Account {
        Account::new(id)
    }

    #[test]
    fn credit_increases_available_and_total() {
        let mut a = account(1);
        a.credit(dec!(50.0)).unwrap();
        assert_eq!(a.available, dec!(50.0));
        assert_eq!(a.held, dec!(0));
        assert_eq!(a.total(), dec!(50.0));
    }

    #[test]
    fn debit_decreases_available_and_total() {
        let mut a = account(1);
        a.credit(dec!(50.0)).unwrap();
        a.debit(dec!(30.0)).unwrap();
        assert_eq!(a.available, dec!(20.0));
        assert_eq!(a.total(), dec!(20.0));
    }

    #[test]
    fn debit_exact_balance_leaves_zero() {
        let mut a = account(1);
        a.credit(dec!(50.0)).unwrap();
        a.debit(dec!(50.0)).unwrap();
        assert_eq!(a.available, dec!(0));
        assert_eq!(a.total(), dec!(0));
    }

    #[test]
    fn debit_insufficient_funds_rejected() {
        let mut a = account(1);
        a.credit(dec!(50.0)).unwrap();
        let err = a.debit(dec!(60.0)).unwrap_err();
        assert_eq!(err, EngineError::InsufficientFunds { client_id: 1 });
        assert_eq!(a.available, dec!(50.0));
    }

    #[test]
    fn hold_moves_available_to_held() {
        let mut a = account(1);
        a.credit(dec!(100.0)).unwrap();
        a.hold(dec!(40.0)).unwrap();
        assert_eq!(a.available, dec!(60.0));
        assert_eq!(a.held, dec!(40.0));
        assert_eq!(a.total(), dec!(100.0));
    }

    #[test]
    fn release_moves_held_to_available() {
        let mut a = account(1);
        a.credit(dec!(100.0)).unwrap();
        a.hold(dec!(40.0)).unwrap();
        a.release(dec!(40.0)).unwrap();
        assert_eq!(a.available, dec!(100.0));
        assert_eq!(a.held, dec!(0));
        assert_eq!(a.total(), dec!(100.0));
    }

    #[test]
    fn chargeback_removes_held_and_locks_account() {
        let mut a = account(1);
        a.credit(dec!(100.0)).unwrap();
        a.hold(dec!(100.0)).unwrap();
        a.chargeback(dec!(100.0));
        assert_eq!(a.held, dec!(0));
        assert_eq!(a.total(), dec!(0));
        assert!(a.locked);
    }

    #[test]
    fn locked_account_rejects_credit() {
        let mut a = account(1);
        a.credit(dec!(100.0)).unwrap();
        a.hold(dec!(100.0)).unwrap();
        a.chargeback(dec!(100.0));
        let err = a.credit(dec!(50.0)).unwrap_err();
        assert_eq!(err, EngineError::AccountLocked { client_id: 1 });
    }

    #[test]
    fn locked_account_rejects_debit() {
        let mut a = account(1);
        a.credit(dec!(100.0)).unwrap();
        a.hold(dec!(50.0)).unwrap();
        a.chargeback(dec!(50.0));
        let err = a.debit(dec!(10.0)).unwrap_err();
        assert_eq!(err, EngineError::AccountLocked { client_id: 1 });
    }

    #[test]
    fn total_invariant_holds_after_all_ops() {
        let mut a = account(1);
        a.credit(dec!(100.0)).unwrap();
        a.debit(dec!(20.0)).unwrap();
        a.hold(dec!(30.0)).unwrap();
        assert_eq!(a.total(), a.available + a.held);
        a.release(dec!(10.0)).unwrap();
        assert_eq!(a.total(), a.available + a.held);
    }
}
