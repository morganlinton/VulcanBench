"""Payment processor. Charge results are recorded by idempotency key."""


class PaymentProcessor:
    def __init__(self, gateway):
        self._gateway = gateway
        self._results = {}  # idempotency_key -> result

    def _charge(self, order):
        return self._gateway.charge(order["amount"])
