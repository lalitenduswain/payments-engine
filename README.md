# Payments Engine

A streaming toy payments engine written in Rust. Reads a series of transactions from a CSV file, updates client accounts, handles disputes and chargebacks, and outputs the final account state as CSV.

## Prerequisites

- Rust toolchain installed via `rustup`
- Install command:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

- Verify installation:

```bash
cargo --version
rustc --version
```

## Build & Run

```bash
cargo build --release
cargo run --release -- transactions.csv
```

Output is written to stdout:

```bash
cargo run --release -- transactions.csv > output.csv
```

## Usage

```
payments-engine <transactions.csv>
```

### Input format

```
type, client, tx, amount
deposit, 1, 1, 100.0
withdrawal, 1, 2, 40.0
dispute, 1, 1,
resolve, 1, 1,
chargeback, 1, 1,
```

- `type`: one of `deposit`, `withdrawal`, `dispute`, `resolve`, `chargeback`
- `client`: u16 client ID
- `tx`: u32 transaction ID (globally unique)
- `amount`: decimal with up to 4 places past the decimal (absent for dispute/resolve/chargeback)

### Output format

```
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

## Testing

```bash
cargo test
```

44 tests: 22 unit tests (account operations and ledger state machine) + 22 integration tests (full CSV → account state scenarios).

## Design

### Architecture

```
CSV File → Streaming Reader → TransactionEngine → AccountStore + TransactionLedger → CSV Writer → stdout
```

The engine is split into four layers with strict separation:

- **`reader.rs`** — streams CSV rows one at a time via the `csv` crate; never loads the full file into memory
- **`engine.rs`** — routes each record to a handler; returns ignorable `EngineError` for business-rule violations
- **`account.rs`** + **`transaction.rs`** — stateful stores; `AccountStore` holds `HashMap<u16, Account>`, `TransactionLedger` holds `HashMap<u32, LedgerEntry>` for deposits only
- **`writer.rs`** — serialises final account state to stdout

### Key decisions

**`rust_decimal` not `f64`:** Financial code must not use floating-point arithmetic. `0.1 + 0.2 != 0.3` in IEEE 754. `rust_decimal` provides exact fixed-point math.

**Only deposits stored in the ledger:** Withdrawals are processed and discarded. Only deposits can be disputed per the fraud scenario described in the spec. This keeps memory at O(clients + deposits), not O(total transactions).

**Disputes can create a negative available balance:** If a client deposits $1000, withdraws $900, then disputes the original $1000 deposit, `available` becomes `-$900`. The bank absorbs the deficit. This is the correct model — the full disputed amount must be held and potentially charged back regardless of the current balance.

**`EngineError` is non-fatal:** Bad references (dispute on unknown tx, resolve on non-disputed tx, etc.) are logged to stderr as warnings and processing continues. Only I/O errors and malformed CSV headers terminate the process.

**Dispute state machine:** Each ledger entry moves through `Clean → Disputed → Resolved | Chargedback`. `Chargedback` is terminal. A `Resolved` tx can be re-disputed. A `Chargedback` tx cannot.

**Account locking:** A chargeback immediately and permanently locks the account. All subsequent transactions against a locked account are rejected.

### Memory model

| Structure            | Type                        | Worst-case size                     |
|----------------------|-----------------------------|-------------------------------------|
| Account store        | `HashMap<u16, Account>`     | 65,535 entries × ~56 bytes ≈ 3.5 MB |
| Transaction ledger   | `HashMap<u32, LedgerEntry>` | n deposits × ~48 bytes              |
| CSV row buffer       | Single row at a time        | O(1)                                |

### Assumptions

| # | Assumption |
|---|------------|
| 1 | Only deposits can be disputed. Withdrawals cannot be disputed. |
| 2 | A locked account rejects all further transactions (deposit, withdrawal, dispute, resolve, chargeback). |
| 3 | A dispute must reference a transaction owned by the same client filing the dispute. |
| 4 | Transaction IDs are globally unique. A duplicate deposit tx ID is ignored. |
| 5 | Disputes can make `available` go negative (bank deficit). |
| 6 | A `Resolved` transaction can be re-disputed. A `Chargedback` transaction cannot. |
| 7 | Missing `amount` on a deposit or withdrawal row is treated as 0.0. |

## AI Tool Usage

This solution was built using Claude Code (claude-sonnet-4-6) as an AI assistant.

### Design phase prompts

1. *"first think like an enterprise architect and create all designs. we will follow a proper software development lifecycle"*
   - Output: 8 design documents (BRD, SAD, Domain Model, LLD, State Machines, Error Handling, Test Strategy, Implementation Plan) saved in `design/`

2. *"yes please"* (start coding)
   - Output: full implementation following the phased plan

### Technical decisions made with AI assistance

- Identified that `hold()` must not check `available >= amount` because disputes should go through even when the disputed deposit has already been partially withdrawn (fraud scenario requires this)
- Chose `rust_decimal` over `f64` for financial precision
- Chose to store only deposits in the ledger (not withdrawals) as a memory optimisation

All prompts and technical decisions are documented above. The implementation was reviewed and understood before submission.
