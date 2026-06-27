# Implement an event-sourced bank ledger

The `ledger` package models a set of bank accounts whose balances are derived from
an append-only event log. The work is split across two files and both must be
implemented and kept consistent:

- `ledger/events.py` — the immutable event schema and the pure reducer
  (`apply_event` and `replay`).
- `ledger/bank.py` — the command side: `Bank` validates commands, emits events,
  and maintains the live balances.

## Event schema (given)

`events.py` already defines three frozen event types. Do not change them:

- `AccountOpened(account_id)`
- `Deposited(account_id, amount)`
- `Withdrawn(account_id, amount)`

## Reducer (`ledger/events.py`)

- `apply_event(state, event)`: return a **new** balances dict (mapping account id
  to balance) with `event` applied to `state`. Do not mutate `state`.
  `AccountOpened` introduces the account at balance 0; `Deposited` and `Withdrawn`
  add and subtract from an existing balance.
- `replay(events)`: fold a sequence of events from an empty ledger into a balances
  dict.

## `Bank` API (`ledger/bank.py`)

Every state change must be expressed as an event appended to a single ordered log
and applied through the reducer.

- `open_account(account_id)`: open a new account at balance 0. `ValueError` if the
  account already exists.
- `deposit(account_id, amount)`: add `amount` to an existing account.
- `withdraw(account_id, amount)`: remove `amount`; raise `InsufficientFunds` if the
  balance is below `amount`.
- `transfer(src, dst, amount)`: move `amount` from `src` to `dst`.
- `balance(account_id)`: the current balance of an existing account.
- `snapshot()`: a copy of the current balances.
- `history()`: the ordered event log.
- `Bank.from_history(events)` (classmethod): rebuild a bank by replaying `events`.

For `deposit`, `withdraw`, and `transfer`, the named account(s) must already exist
(`ValueError` otherwise) and `amount` must be positive (`ValueError` otherwise).

## Required behavior

- **State is derived from events.** The live balances must always equal
  `replay(history())`. Maintain them by applying each emitted event through the
  reducer, not by mutating a separate counter.
- **Rejected commands change nothing.** If a command fails validation (unknown
  account, non-positive amount, insufficient funds), it must emit **no** event and
  leave all balances unchanged.
- **Transfers are atomic.** A successful `transfer` emits exactly two events (a
  `Withdrawn` from `src` and a `Deposited` to `dst`). If it cannot complete, it
  emits neither — the log must never contain a half-applied transfer.
- **Rebuild matches.** `Bank.from_history(bank.history())` must reproduce the
  original bank's balances exactly.
