use std::collections::HashMap;

use pretty_assertions::assert_eq;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Runs the engine over a CSV string and returns a map of client_id → (available, held, locked).
fn run(csv: &str) -> HashMap<u16, (Decimal, Decimal, bool)> {
    use payments_engine::{engine::TransactionEngine, reader::stream_transactions};
    let mut engine = TransactionEngine::new();
    for result in stream_transactions(csv.as_bytes()) {
        if let Ok(record) = result {
            let _ = engine.process(record);
        }
    }
    engine
        .accounts()
        .iter()
        .map(|a| (a.client_id, (a.available, a.held, a.locked)))
        .collect()
}

fn total(available: Decimal, held: Decimal) -> Decimal {
    available + held
}

// ── spec sample ───────────────────────────────────────────────────────────────

#[test]
fn spec_sample_input() {
    let csv = "\
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0";

    let state = run(csv);
    let (avail1, held1, locked1) = state[&1];
    let (avail2, held2, locked2) = state[&2];

    assert_eq!(avail1, dec!(1.5));
    assert_eq!(held1, dec!(0));
    assert!(!locked1);
    assert_eq!(total(avail1, held1), dec!(1.5));

    assert_eq!(avail2, dec!(2.0));
    assert_eq!(held2, dec!(0));
    assert!(!locked2);
}

// ── deposits ─────────────────────────────────────────────────────────────────

#[test]
fn deposit_increases_available_and_total() {
    let state = run("type,client,tx,amount\ndeposit,1,1,100.0");
    let (avail, held, locked) = state[&1];
    assert_eq!(avail, dec!(100.0));
    assert_eq!(held, dec!(0));
    assert!(!locked);
}

#[test]
fn multiple_deposits_accumulate() {
    let state = run("type,client,tx,amount\ndeposit,1,1,50.0\ndeposit,1,2,30.0");
    let (avail, _, _) = state[&1];
    assert_eq!(avail, dec!(80.0));
}

// ── withdrawals ───────────────────────────────────────────────────────────────

#[test]
fn withdrawal_decreases_available() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nwithdrawal,1,2,40.0";
    let (avail, held, _) = run(csv)[&1];
    assert_eq!(avail, dec!(60.0));
    assert_eq!(held, dec!(0));
    assert_eq!(total(avail, held), dec!(60.0));
}

#[test]
fn withdrawal_exact_balance_leaves_zero() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nwithdrawal,1,2,100.0";
    let (avail, _, _) = run(csv)[&1];
    assert_eq!(avail, dec!(0));
}

#[test]
fn withdrawal_rejected_when_insufficient_funds() {
    let csv = "type,client,tx,amount\ndeposit,1,1,50.0\nwithdrawal,1,2,60.0";
    let (avail, _, _) = run(csv)[&1];
    assert_eq!(avail, dec!(50.0)); // unchanged
}

// ── disputes ──────────────────────────────────────────────────────────────────

#[test]
fn dispute_moves_funds_to_held() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,";
    let (avail, held, locked) = run(csv)[&1];
    assert_eq!(avail, dec!(0));
    assert_eq!(held, dec!(100.0));
    assert_eq!(total(avail, held), dec!(100.0)); // total unchanged
    assert!(!locked);
}

#[test]
fn dispute_nonexistent_tx_ignored() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,999,";
    let (avail, held, _) = run(csv)[&1];
    assert_eq!(avail, dec!(100.0));
    assert_eq!(held, dec!(0));
}

#[test]
fn double_dispute_second_ignored() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\ndispute,1,1,";
    let (avail, held, _) = run(csv)[&1];
    assert_eq!(avail, dec!(0));
    assert_eq!(held, dec!(100.0));
}

// ── resolves ──────────────────────────────────────────────────────────────────

#[test]
fn resolve_releases_held_funds() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nresolve,1,1,";
    let (avail, held, locked) = run(csv)[&1];
    assert_eq!(avail, dec!(100.0));
    assert_eq!(held, dec!(0));
    assert_eq!(total(avail, held), dec!(100.0));
    assert!(!locked);
}

#[test]
fn resolve_without_dispute_ignored() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nresolve,1,1,";
    let (avail, held, _) = run(csv)[&1];
    assert_eq!(avail, dec!(100.0));
    assert_eq!(held, dec!(0));
}

#[test]
fn resolve_nonexistent_tx_ignored() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nresolve,1,999,";
    let (avail, _, _) = run(csv)[&1];
    assert_eq!(avail, dec!(100.0));
}

// ── chargebacks ───────────────────────────────────────────────────────────────

#[test]
fn chargeback_deducts_held_and_locks_account() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nchargeback,1,1,";
    let (avail, held, locked) = run(csv)[&1];
    assert_eq!(avail, dec!(0));
    assert_eq!(held, dec!(0));
    assert_eq!(total(avail, held), dec!(0));
    assert!(locked);
}

#[test]
fn chargeback_without_dispute_ignored() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nchargeback,1,1,";
    let (avail, held, locked) = run(csv)[&1];
    assert_eq!(avail, dec!(100.0));
    assert_eq!(held, dec!(0));
    assert!(!locked);
}

#[test]
fn chargeback_nonexistent_tx_ignored() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nchargeback,1,999,";
    let (avail, _, locked) = run(csv)[&1];
    assert_eq!(avail, dec!(100.0));
    assert!(!locked);
}

// ── locked account ────────────────────────────────────────────────────────────

#[test]
fn locked_account_rejects_further_deposits() {
    let csv =
        "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nchargeback,1,1,\ndeposit,1,2,50.0";
    let (avail, _, locked) = run(csv)[&1];
    assert_eq!(avail, dec!(0)); // second deposit rejected
    assert!(locked);
}

#[test]
fn locked_account_rejects_withdrawals() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndeposit,1,2,50.0\ndispute,1,1,\nchargeback,1,1,\nwithdrawal,1,3,50.0";
    let (avail, held, locked) = run(csv)[&1];
    // After chargeback: available=50, held=0. Then withdrawal rejected.
    assert_eq!(avail, dec!(50.0));
    assert_eq!(held, dec!(0));
    assert!(locked);
}

// ── precision ─────────────────────────────────────────────────────────────────

#[test]
fn four_decimal_place_precision() {
    let csv =
        "type,client,tx,amount\ndeposit,1,1,1.1234\ndeposit,1,2,0.0001\nwithdrawal,1,3,0.1111";
    let (avail, _, _) = run(csv)[&1];
    assert_eq!(avail, dec!(1.0124));
}

// ── multi-client isolation ────────────────────────────────────────────────────

#[test]
fn multiple_clients_are_independent() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndeposit,2,2,200.0\ndeposit,3,3,300.0\nwithdrawal,1,4,50.0\nwithdrawal,2,5,100.0\ndispute,3,3,\nchargeback,3,3,";
    let state = run(csv);

    let (avail1, held1, locked1) = state[&1];
    assert_eq!(avail1, dec!(50.0));
    assert_eq!(held1, dec!(0));
    assert!(!locked1);

    let (avail2, held2, locked2) = state[&2];
    assert_eq!(avail2, dec!(100.0));
    assert_eq!(held2, dec!(0));
    assert!(!locked2);

    let (avail3, held3, locked3) = state[&3];
    assert_eq!(avail3, dec!(0));
    assert_eq!(held3, dec!(0));
    assert_eq!(total(avail3, held3), dec!(0));
    assert!(locked3);
}

// ── dispute lifecycle edge cases ──────────────────────────────────────────────

#[test]
fn re_dispute_after_resolve_holds_funds_again() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nresolve,1,1,\ndispute,1,1,";
    let (avail, held, _) = run(csv)[&1];
    assert_eq!(avail, dec!(0));
    assert_eq!(held, dec!(100.0));
}

#[test]
fn dispute_different_client_tx_ignored() {
    // client 2 tries to dispute a tx belonging to client 1
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,2,1,";
    let (avail1, held1, _) = run(csv)[&1];
    assert_eq!(avail1, dec!(100.0)); // client 1 unaffected
    assert_eq!(held1, dec!(0));
}

// ── fix: cross-client resolve / chargeback ───────────────────────────────────

#[test]
fn resolve_by_wrong_client_leaves_funds_held() {
    // client 1 has a disputed deposit; client 2 tries to resolve it
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nresolve,2,1,";
    let state = run(csv);
    let (avail1, held1, _) = state[&1];
    // resolve must be ignored — funds must remain held for client 1
    assert_eq!(avail1, dec!(0));
    assert_eq!(held1, dec!(100.0));
}

#[test]
fn chargeback_by_wrong_client_does_not_lock_or_deduct() {
    // client 1 has a disputed deposit; client 2 tries to chargeback it
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nchargeback,2,1,";
    let state = run(csv);
    let (avail1, held1, locked1) = state[&1];
    // chargeback must be ignored — client 1 still holds the disputed funds, not locked
    assert_eq!(avail1, dec!(0));
    assert_eq!(held1, dec!(100.0));
    assert!(!locked1);
}

// ── fix: negative and zero amounts ───────────────────────────────────────────

#[test]
fn negative_deposit_rejected() {
    // The negative deposit must be fully rejected: no account created, no balance change.
    let csv = "type,client,tx,amount\ndeposit,1,1,-100.0";
    let state = run(csv);
    assert!(!state.contains_key(&1));
}

#[test]
fn zero_deposit_rejected() {
    let csv = "type,client,tx,amount\ndeposit,1,1,0.0\ndeposit,1,2,50.0";
    let (avail, _, _) = run(csv)[&1];
    // only the 50.0 deposit should count
    assert_eq!(avail, dec!(50.0));
}

#[test]
fn negative_withdrawal_rejected() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nwithdrawal,1,2,-50.0";
    let (avail, _, _) = run(csv)[&1];
    // negative withdrawal must not increase available balance
    assert_eq!(avail, dec!(100.0));
}

#[test]
fn zero_withdrawal_rejected() {
    let csv = "type,client,tx,amount\ndeposit,1,1,100.0\nwithdrawal,1,2,0.0";
    let (avail, _, _) = run(csv)[&1];
    assert_eq!(avail, dec!(100.0));
}

// ── fix: serde-str precision (was serde-float) ────────────────────────────────

#[test]
fn decimal_precision_not_lost_through_f64() {
    // 0.0001 cannot be represented exactly in f64 — this would fail with serde-float
    let csv = "type,client,tx,amount\ndeposit,1,1,0.0001\ndeposit,1,2,0.0002\ndeposit,1,3,0.0003";
    let (avail, _, _) = run(csv)[&1];
    assert_eq!(avail, dec!(0.0006));
}

// ── fraud scenario (the motivating use case) ──────────────────────────────────

#[test]
fn fraud_scenario_deposit_withdraw_chargeback() {
    // Bad actor: deposits 1000, withdraws 900, then chargebacks the full 1000 deposit.
    //
    // Trace:
    //   deposit 1000  → available=1000
    //   withdraw 900  → available=100
    //   dispute tx=1  → available=100-1000=-900, held=1000, total=100 (unchanged)
    //   chargeback    → held=0, total=-900, locked=true
    //
    // The account goes into deficit (-900). The bank absorbs the loss.
    // This is the correct behavior per the spec — the dispute/chargeback goes through
    // regardless of whether the funds are still available.
    let csv = "type,client,tx,amount\ndeposit,1,1,1000.0\nwithdrawal,1,2,900.0\ndispute,1,1,\nchargeback,1,1,";
    let (avail, held, locked) = run(csv)[&1];
    assert_eq!(avail, dec!(-900.0));
    assert_eq!(held, dec!(0));
    assert_eq!(total(avail, held), dec!(-900.0));
    assert!(locked);
}
