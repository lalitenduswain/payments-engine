# AI Prompts Transcript

This file documents every prompt submitted to Claude (claude-sonnet-4-6 via Claude Code) during
the development of this project, along with a summary of what was produced and what decisions were
made or validated as a result.

---

## Prompt 1 — Understand the problem

**Prompt:**
> First explain me in simple language.

The full challenge specification was pasted alongside this prompt.

**What the AI produced:**
A plain-English breakdown of the five transaction types (deposit, withdrawal, dispute, resolve,
chargeback), the account fields (available, held, total, locked), the fraud scenario motivating
the engine, and the key implementation constraints (streaming, 4dp precision, Rust type hints).

**Decisions made:**
- Confirmed understanding of the dispute lifecycle before any design work began.

---

## Prompt 2 — Full system design

**Prompt:**
> First think like an enterprise architect and create all designs. We will follow a proper
> software development lifecycle.

**What the AI produced:**
Eight design documents written to `design/`:

| File | Contents |
|---|---|
| `01_BRD.md` | 14 functional requirements, 9 non-functional requirements, assumptions, constraints |
| `02_SAD.md` | Layered architecture diagram, data-flow, technology decisions, memory model |
| `03_Domain_Model.md` | Entities, value objects, enums, ER diagram, Cargo.toml dependency rationale |
| `04_LLD.md` | Full pseudo-Rust signatures for every module, struct, and method |
| `05_State_Machines.md` | Account (Active/Locked) + DisputeState machine + step-by-step operation traces |
| `06_Error_Handling.md` | Fatal vs ignorable errors, 40+ edge cases catalogued |
| `07_Test_Strategy.md` | Unit tests per module, integration fixtures, E2E plan |
| `08_Implementation_Plan.md` | 5 phased tasks, file creation order, delivery checklist |

**Key design decisions established at this stage:**
- Use `rust_decimal` not `f64` — financial code must never use floating-point arithmetic.
- Store only deposits in the ledger (not withdrawals) — only deposits can be disputed, so
  withdrawals can be discarded after processing. Memory: O(clients + deposits) not O(all tx).
- `EngineError` is non-fatal — bad rows log a warning and processing continues; only I/O errors
  kill the process.
- `total` is a computed property (`available + held`), never stored — prevents the invariant
  `total = available + held` from ever being violated.
- Streaming CSV via `csv::ReaderBuilder` — never `collect()` the full input.

---

## Prompt 3 — Implementation

**Prompt:**
> Yes please. (in response to: "Ready to start Phase 1 — shall I begin coding?")

**What the AI produced:**
The full implementation across all 5 phases:

- `Cargo.toml` with all dependencies
- `src/types.rs` — `ClientId`, `TxId`, `Amount` type aliases
- `src/error.rs` — `EngineError` enum with `thiserror`
- `src/account.rs` — `Account` struct + `AccountStore` + 10 unit tests
- `src/transaction.rs` — `DisputeState` enum + `LedgerEntry` + `TransactionLedger` + 12 unit tests
- `src/reader.rs` — streaming CSV deserializer
- `src/writer.rs` — CSV output with 4dp formatting
- `src/engine.rs` — `TransactionEngine` with all 5 transaction handlers
- `src/main.rs` — CLI entry point
- `src/lib.rs` — public re-exports for integration tests
- `tests/engine_tests.rs` — 22 integration tests
- `tests/fixtures/` — 9 CSV fixture files

**One bug caught and corrected during this phase:**
The fraud scenario test exposed that `hold()` must not check `available >= amount`. When a client
disputes a deposit they have already partially withdrawn, the available balance is less than the
disputed amount. The dispute must still go through — the account goes into deficit, the bank
absorbs the loss. This is the correct model per the spec's fraud scenario. The check was removed
and the test expected values corrected.

**Result:** 44/44 tests passing.

---

## Prompt 4 — Edge case review

**Prompt:**
> How many edge cases we handled?

**What the AI produced:**
A full table of all 44 test cases organised by layer (account unit tests, ledger state machine
unit tests, engine integration tests), with a description of what each one covers.

---

## Prompt 5 — Production readiness audit

**Prompt:**
> Is it prod ready code?

**What the AI produced:**
An honest audit identifying three bugs and several production maturity gaps:

**Bugs found:**
1. `serde-float` feature on `rust_decimal` routes decimal parsing through `f64`, defeating the
   purpose of using `rust_decimal`. Should be `serde-str`.
2. `resolve` and `chargeback` handlers did not validate that the client in the CSV row owns the
   referenced transaction. Client 2 could resolve client 1's dispute.
3. Negative and zero amounts on deposits and withdrawals were not rejected.

**Production maturity gaps noted (not bugs):**
- No structured logging
- No audit trail
- Non-deterministic output order
- Unbounded ledger memory for long-running processes
- No `Cargo.lock` committed
- No CI/CD

---

## Prompt 6 — Fix the bugs

**Prompt:**
> Yes please. (in response to: "Want me to fix all three bugs now?")

**What the AI produced:**

**Fix 1 — `serde-float` → `serde-str`** (`Cargo.toml`)
Changed `rust_decimal` feature from `serde-float` to `serde-str` so amounts are parsed directly
from the CSV string, not routed through `f64`.

**Fix 2 — Client ownership validation on resolve/chargeback** (`transaction.rs`, `engine.rs`)
Added `client_id` parameter to `resolve_dispute()` and `chargeback_dispute()` in
`TransactionLedger`. Both methods now reject with `ClientMismatch` if the requesting client does
not own the referenced transaction — the same guard `begin_dispute()` already had.

**Fix 3 — Non-positive amount guard** (`error.rs`, `engine.rs`)
Added `EngineError::NonPositiveAmount` variant. `apply_deposit()` and `apply_withdrawal()` now
reject `amount <= 0` before touching any account state.

New tests added for all three fixes. Result: 53/53 tests passing.

---

## Prompt 7 — Second production readiness check

**Prompt:**
> Is it now prod ready?

**What the AI produced:**
A revised assessment confirming the three bugs were fixed and identifying the one remaining
correctness issue: non-deterministic output order due to `HashMap` iteration.

---

## Prompt 8 — Fix output ordering

**Prompt:**
> Yes please. (in response to: "Want me to fix the output ordering?")

**What the AI produced:**
A 3-line change in `writer.rs` — collect accounts into a `Vec`, sort by `client_id` ascending,
then iterate. Output is now deterministic across all runs.

---

## Prompt 9 — Third production readiness check

**Prompt:**
> Is the code now prod ready?

**What the AI produced:**
A final honest assessment:
- For this coding challenge: **yes**.
- Remaining gaps (structured logging, audit trail, CI/CD, `Cargo.lock`) are operational maturity
  concerns expected to be absent in a take-home challenge, not correctness issues.

---

## Prompt 10 — Human readability review

**Prompt:**
> Are all code human readable?

**What the AI produced:**
A line-by-line review of every source file identifying 5 specific spots where a future maintainer
would have to pause and reason through the code:

1. `reader.rs` — `flexible(true)` and `trim(All)` looked like arbitrary config flags.
2. `engine.rs` — `unwrap_or_default()` silently converting `None → 0.0 → rejected` was indirect.
3. `account.rs` — `chargeback()` skips the locked check with no explanation.
4. `engine.rs` — `if let Some(account)` in dispute handlers looked like a null-guard but is
   actually defensive.
5. `engine_tests.rs` — `negative_deposit_rejected` test had a stray garbage CSV row acting as a
   comment, and a vague `map_or` assertion.

Each was fixed with a single, purposeful comment. No logic was changed.

---

## Prompt 11 — Requirements alignment check

**Prompt:**
> This is the refined requirement I got from ChatGPT. Can you make sure it aligns with ours?
> Don't change any code, just check and let me know.

**What the AI produced:**
A gap analysis comparing the refined requirements document against the actual implementation:

**Fully aligned:** CLI interface, input/output format, account model, all 5 transaction types,
all edge case rules, streaming processing, safety guarantees.

**Misalignments found:**
- `money.rs` in the doc does not exist — we have `types.rs` with `rust_decimal` type aliases.
  The doc's description "stored as integers scaled to 4 decimal places" is ChatGPT's
  simplification and does not accurately describe `rust_decimal::Decimal`'s internals.
- The doc implies all transactions are stored in `HashMap<TxId, Transaction>`; we store only
  deposits (`LedgerEntry`) — a deliberate memory optimisation.
- Our project has 4 extra modules not mentioned in the doc (`reader.rs`, `writer.rs`, `error.rs`,
  `lib.rs`) — this is strictly better separation of concerns.
- `ai-prompts.md` referenced in the doc did not yet exist.

**Where our code exceeds the requirements:**
- Negative/zero amount validation (not in the doc).
- Cross-client ownership check on resolve/chargeback (not in the doc).
- Stable deterministic output ordering (not in the doc).

---

## Prompt 12 — Create this file

**Prompt:**
> Create ai-prompts.md.

**What the AI produced:**
This file.

---

## Summary of AI's role

| Area | AI role |
|---|---|
| Problem understanding | Explained the spec in plain English |
| Architecture & design | Produced all 8 design documents |
| Implementation | Wrote all source files and tests |
| Bug discovery | Identified 3 correctness bugs during audit |
| Bug fixes | Implemented all fixes |
| Readability | Identified 5 unclear spots, added targeted comments |
| Requirements alignment | Cross-checked refined doc against actual code |

All code, logic, and design decisions were reviewed, understood, and validated before acceptance.
The final implementation reflects deliberate engineering choices — not blind AI output.
