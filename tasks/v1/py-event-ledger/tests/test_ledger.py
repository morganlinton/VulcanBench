"""An event-sourced ledger split across two files.

``ledger/events.py`` defines the immutable event schema and the pure reducer
(``apply_event`` / ``replay``); ``ledger/bank.py`` is the command side that emits
events and keeps live balances. The two must stay consistent: the live state must
always equal a replay of the log, transfers must emit two events atomically, and a
bank rebuilt from its history must match the original.
"""

import pytest

from ledger import Bank, InsufficientFunds, replay


def test_construct():
    # passes pre-fix: construction only, no commands issued
    assert isinstance(Bank(), Bank)


def test_open_deposit_balance():
    b = Bank()
    b.open_account("alice")
    b.deposit("alice", 100)
    assert b.balance("alice") == 100


def test_withdraw_and_insufficient_funds():
    b = Bank()
    b.open_account("alice")
    b.deposit("alice", 50)
    b.withdraw("alice", 20)
    assert b.balance("alice") == 30
    before = b.snapshot()
    n = len(b.history())
    with pytest.raises(InsufficientFunds):
        b.withdraw("alice", 1000)
    assert b.snapshot() == before    # rejected command changes nothing
    assert len(b.history()) == n     # and emits no event


def test_validation_errors():
    b = Bank()
    with pytest.raises(ValueError):
        b.deposit("ghost", 10)       # unknown account
    b.open_account("alice")
    with pytest.raises(ValueError):
        b.open_account("alice")      # duplicate open
    with pytest.raises(ValueError):
        b.deposit("alice", 0)        # non-positive amount
    with pytest.raises(ValueError):
        b.withdraw("alice", -5)


def test_transfer_moves_funds_with_two_events():
    b = Bank()
    b.open_account("alice")
    b.open_account("bob")
    b.deposit("alice", 100)
    n = len(b.history())
    b.transfer("alice", "bob", 40)
    assert (b.balance("alice"), b.balance("bob")) == (60, 40)
    assert len(b.history()) == n + 2   # a Withdrawn and a Deposited


def test_transfer_is_atomic_on_insufficient_funds():
    b = Bank()
    b.open_account("alice")
    b.open_account("bob")
    b.deposit("alice", 30)
    before = b.snapshot()
    n = len(b.history())
    with pytest.raises(InsufficientFunds):
        b.transfer("alice", "bob", 100)
    assert b.snapshot() == before      # neither side moved
    assert len(b.history()) == n       # no half-applied transfer in the log


def test_rebuild_from_history_matches_live_state():
    b = Bank()
    for name in ("alice", "bob", "carol"):
        b.open_account(name)
    b.deposit("alice", 100)
    b.deposit("bob", 50)
    b.transfer("alice", "carol", 30)
    b.withdraw("bob", 10)
    rebuilt = Bank.from_history(b.history())
    assert rebuilt.snapshot() == b.snapshot()


def test_snapshot_equals_replay_of_history():
    b = Bank()
    b.open_account("alice")
    b.deposit("alice", 100)
    b.open_account("bob")
    b.transfer("alice", "bob", 25)
    assert b.snapshot() == replay(b.history())
    assert b.snapshot() == {"alice": 75, "bob": 25}
