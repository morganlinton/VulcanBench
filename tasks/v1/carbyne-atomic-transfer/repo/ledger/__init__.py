"""An in-memory ledger. deposit/withdraw validate inputs and keep balances
non-negative; withdraw raises InsufficientFunds and changes nothing if the
balance is too low."""


class InsufficientFunds(Exception):
    pass


class Ledger:
    def __init__(self):
        self._balances = {}

    def open(self, account):
        self._balances.setdefault(account, 0)

    def balance(self, account):
        return self._balances[account]

    def deposit(self, account, amount):
        if amount <= 0:
            raise ValueError("amount must be positive")
        self._balances[account] += amount

    def withdraw(self, account, amount):
        if amount <= 0:
            raise ValueError("amount must be positive")
        if self._balances[account] < amount:
            raise InsufficientFunds(account)
        self._balances[account] -= amount
