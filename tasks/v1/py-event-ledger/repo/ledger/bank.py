"""The command side of an event-sourced ledger.

Every state change is expressed as an event appended to a single ordered log and
applied through ``ledger.events.apply_event``. The live balances must therefore
always equal ``replay(history())``, and a bank rebuilt from a history must match
the original.
"""

from ledger.events import AccountOpened, Deposited, Withdrawn, apply_event, replay


class InsufficientFunds(Exception):
    """Raised when a withdrawal or transfer exceeds the available balance."""


class Bank:
    """A set of accounts whose balances are derived from an event log."""

    def __init__(self):
        self._log = []
        self._balances = {}

    def open_account(self, account_id):
        """Open a new account at zero balance. ValueError if it already exists."""
        raise NotImplementedError

    def deposit(self, account_id, amount):
        """Add ``amount`` (> 0) to an existing account."""
        raise NotImplementedError

    def withdraw(self, account_id, amount):
        """Remove ``amount`` (> 0); InsufficientFunds if the balance is too low."""
        raise NotImplementedError

    def transfer(self, src, dst, amount):
        """Move ``amount`` from ``src`` to ``dst`` atomically."""
        raise NotImplementedError

    def balance(self, account_id):
        """Return the current balance of an existing account."""
        raise NotImplementedError

    def snapshot(self):
        """Return a copy of the current balances (account id -> balance)."""
        raise NotImplementedError

    def history(self):
        """Return the ordered event log."""
        raise NotImplementedError

    @classmethod
    def from_history(cls, events):
        """Rebuild a bank by replaying ``events``."""
        raise NotImplementedError
