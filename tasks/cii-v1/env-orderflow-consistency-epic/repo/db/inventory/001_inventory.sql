CREATE TABLE products (
    sku       text PRIMARY KEY,
    available integer NOT NULL CHECK (available >= 0),
    reserved  integer NOT NULL DEFAULT 0
);

CREATE TABLE reservations (
    id         text PRIMARY KEY,
    sku        text NOT NULL REFERENCES products (sku),
    order_id   text NOT NULL,
    qty        integer NOT NULL CHECK (qty > 0),
    status     text NOT NULL DEFAULT 'ACTIVE'
               CHECK (status IN ('ACTIVE', 'COMMITTED', 'RELEASED')),
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX reservations_expiry ON reservations (status, expires_at);
CREATE INDEX reservations_order ON reservations (order_id);
