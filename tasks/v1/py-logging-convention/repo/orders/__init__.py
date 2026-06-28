"""Order processing. Use the module logger for diagnostics, never print()."""

import logging

logger = logging.getLogger("orders")


def cancel_order(order):
    """Cancel an order and log the action."""
    order["status"] = "cancelled"
    logger.info("order %s cancelled", order["id"])
    return order


def refund_order(order):
    """Refund an order and log the action."""
    order["status"] = "refunded"
    logger.info("order %s refunded", order["id"])
    return order
