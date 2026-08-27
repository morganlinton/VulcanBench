-- The worker's support-facing summary of every order. Rebuilt incrementally
-- as settle events are processed; the reconciliation job compares it with
-- the orders table.
CREATE TABLE order_read_model (
    order_id          text PRIMARY KEY,
    status            text NOT NULL,
    charge_id         text,
    reservation_state text,
    updated_at        timestamptz NOT NULL DEFAULT now()
);
