"""A transactional KV store whose nesting semantics span two files.

``txnkv/store.py`` owns the data and computes the inverse of each mutation;
``txnkv/journal.py`` owns the frame stack and the commit/rollback nesting. Both
must be implemented and kept consistent: rollback restores the *exact* prior
state (including keys that did not exist), and committing a nested transaction
must keep its changes undoable by an outer rollback.
"""

import pytest

from txnkv import Store, UndoJournal


def test_construct():
    # passes pre-fix: construction only, no method calls
    assert isinstance(Store(), Store)
    assert isinstance(UndoJournal(), UndoJournal)


def test_autocommit_without_transaction():
    s = Store()
    s.set("a", 1)
    assert s.get("a") == 1
    s.set("a", 2)
    assert s.get("a") == 2
    s.delete("a")
    with pytest.raises(KeyError):
        s.get("a")


def test_rollback_restores_prior_value_and_absence():
    s = Store()
    s.set("a", 1)  # durable
    s.begin()
    s.set("a", 2)       # overwrite existing
    s.set("b", 9)       # brand-new key
    assert (s.get("a"), s.get("b")) == (2, 9)
    s.rollback()
    assert s.get("a") == 1          # old value restored
    with pytest.raises(KeyError):   # b never existed before the txn
        s.get("b")


def test_rollback_restores_deleted_key():
    s = Store()
    s.set("a", 1)
    s.begin()
    s.delete("a")
    with pytest.raises(KeyError):
        s.get("a")
    s.rollback()
    assert s.get("a") == 1  # deletion undone


def test_commit_keeps_changes():
    s = Store()
    s.begin()
    s.set("a", 1)
    s.commit()
    assert s.get("a") == 1
    s.begin()
    s.set("a", 2)
    s.commit()
    assert s.get("a") == 2


def test_nested_commit_then_outer_rollback_undoes_everything():
    s = Store()
    s.set("a", 1)
    s.begin()           # outer
    s.set("a", 2)
    s.begin()           # inner
    s.set("a", 3)
    s.set("b", 9)
    s.commit()          # inner commit folds into outer
    assert (s.get("a"), s.get("b")) == (3, 9)
    s.rollback()        # outer rollback must revert inner-committed work too
    assert s.get("a") == 1
    with pytest.raises(KeyError):
        s.get("b")


def test_nested_inner_rollback_leaves_outer_intact():
    s = Store()
    s.set("a", 1)
    s.begin()
    s.set("a", 2)
    s.begin()
    s.set("a", 3)
    s.rollback()        # inner rollback only
    assert s.get("a") == 2   # back to the outer-txn value, not the durable one
    s.commit()          # outer commit keeps it
    assert s.get("a") == 2


def test_rollback_replays_multiple_writes_in_reverse():
    s = Store()
    s.set("a", 0)
    s.begin()
    s.set("a", 1)
    s.set("a", 2)
    s.set("a", 3)
    s.rollback()
    assert s.get("a") == 0  # all three writes undone in the right order


def test_commit_without_transaction_raises():
    s = Store()
    with pytest.raises(RuntimeError):
        s.commit()
    with pytest.raises(RuntimeError):
        s.rollback()
    # a real begin/commit must still work (this part fails on the unimplemented stub)
    s.begin()
    s.set("a", 1)
    s.commit()
    assert s.get("a") == 1


def test_keys_reflects_transaction_state():
    s = Store()
    s.set("a", 1)
    s.begin()
    s.set("b", 2)
    s.delete("a")
    assert s.keys() == {"b"}
    s.rollback()
    assert s.keys() == {"a"}
