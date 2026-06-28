Add a `process(order, idempotency_key)` method to `PaymentProcessor` that charges the order and returns the charge result. The `_results` map is keyed by idempotency_key.
