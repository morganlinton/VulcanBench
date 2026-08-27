# orderflow

A small commerce platform: five services, one Postgres, one Redis.

| service   | owns                | responsibilities |
|-----------|---------------------|------------------|
| gateway   | (stateless)         | storefront API, checkout orchestration |
| orders    | `orders_db`         | order lifecycle, coupons, pricing, read model table |
| inventory | `inventory_db`      | stock, reservations, availability cache |
| billing   | `billing_db`        | idempotent charges, settle events |
| worker    | (stateless)         | settle-event processing, hot cache sync, reconciliation |

Services talk HTTP to each other and never touch another service's
database, with one deliberate exception: the worker reads `orders_db` and
`billing_db` to build the support read model, and writes
`orders_db.order_read_model`.

## Running

Infrastructure (postgres, redis) must already be up, with published ports
recorded in `.vb_services.json` (see `orderflow/ofkit/topology.py`). Then:

```
python scripts/dev_up.py
```

boots every service on an ephemeral port and writes `.of_topology.json`.
Each service is one OS process (`python -m orderflow.<service>`).

## Checkout flow

`POST /checkout/{order_id}` on the gateway: reserve stock, quote the order,
charge (idempotent per key at billing), record the payment on the order,
commit the reservation. The worker's `tick` job turns settle events into
order completions and keeps the read model current; `hot_sync` bulk-
refreshes availability for flash-sale SKUs; `reconcile` re-aligns orders
and the read model. Reservation expiry is inventory's `/internal/expire`,
driven by the ops scheduler.

## Ops jobs

Every scheduled job is also on-demand: `POST /admin/tick`,
`POST /admin/hot_sync`, `POST /admin/reconcile` on the worker, and
`POST /internal/expire` on inventory.
