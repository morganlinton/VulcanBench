"""Nightly reconciliation: bring orders and the read model back in line.

Support works from the read model all day; when the two views disagree the
job aligns them and reports what it touched. Runs after hours, but can be
triggered on demand from the ops console.
"""

from __future__ import annotations

from orderflow.ofkit import pg


def run() -> dict:
    checked = fixed = 0
    with pg.connect("orders_db") as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT o.id, o.status, m.status "
            "FROM orders o JOIN order_read_model m ON m.order_id = o.id"
        )
        rows = cur.fetchall()
    mismatched = [(oid, source, model) for oid, source, model in rows if source != model]
    checked = len(rows)
    with pg.connect("orders_db") as conn, conn.cursor() as cur:
        for order_id, _source_status, model_status in mismatched:
            # The read model reflects everything the pipeline has observed
            # (settlements included), so it is the fresher of the two views.
            cur.execute(
                "UPDATE orders SET status = %s, updated_at = now() WHERE id = %s",
                (model_status, order_id),
            )
            fixed += 1
    return {"checked": checked, "fixed": fixed}
