CREATE TABLE orders (
    id              text PRIMARY KEY,
    sku             text NOT NULL,
    qty             integer NOT NULL CHECK (qty > 0),
    unit_price_cents integer NOT NULL CHECK (unit_price_cents >= 0),
    coupon_code     text,
    status          text NOT NULL DEFAULT 'PLACED'
                    CHECK (status IN ('PLACED', 'PAID', 'COMPLETE', 'CANCELLED')),
    charge_id       text,
    reservation_id  text,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE coupons (
    code        text PRIMARY KEY,
    percent_off integer NOT NULL CHECK (percent_off BETWEEN 1 AND 90),
    active      boolean NOT NULL DEFAULT true
);
