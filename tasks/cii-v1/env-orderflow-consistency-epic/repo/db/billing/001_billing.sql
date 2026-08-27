CREATE TABLE charges (
    id           text PRIMARY KEY,
    order_id     text NOT NULL,
    amount_cents integer NOT NULL CHECK (amount_cents > 0),
    idem_key     text NOT NULL UNIQUE,
    status       text NOT NULL DEFAULT 'CREATED'
                 CHECK (status IN ('CREATED', 'SETTLED')),
    created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX charges_order ON charges (order_id);

-- Settlement notifications for the worker; processed rows are kept for audit.
CREATE TABLE settle_events (
    id           bigserial PRIMARY KEY,
    charge_id    text NOT NULL REFERENCES charges (id),
    order_id     text NOT NULL,
    amount_cents integer NOT NULL,
    processed    boolean NOT NULL DEFAULT false,
    created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX settle_events_pending ON settle_events (processed, id);
