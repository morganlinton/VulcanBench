"""Public API gateway: the storefront's single entrypoint."""

from __future__ import annotations

from orderflow.ofkit import client, topology
from orderflow.ofkit.http import ApiError, Request, Router
from orderflow.gateway import checkout

router = Router("gateway")


@router.route("GET", "/healthz")
def healthz(_request: Request) -> tuple[int, dict]:
    return 200, {"ok": True, "service": "gateway"}


@router.route("POST", "/orders")
def create_order(request: Request) -> tuple[int, dict]:
    return 201, client.post(topology.service_url("orders") + "/orders", request.body)


@router.route("GET", "/orders/{order_id}")
def get_order(request: Request) -> tuple[int, dict]:
    return 200, client.get(
        topology.service_url("orders") + f"/orders/{request.params['order_id']}"
    )


@router.route("POST", "/orders/{order_id}/coupon")
def coupon(request: Request) -> tuple[int, dict]:
    return 200, client.post(
        topology.service_url("orders") + f"/orders/{request.params['order_id']}/coupon",
        request.body,
    )


@router.route("GET", "/availability/{sku}")
def availability(request: Request) -> tuple[int, dict]:
    return 200, client.get(
        topology.service_url("inventory") + f"/availability/{request.params['sku']}"
    )


@router.route("POST", "/checkout/{order_id}")
def do_checkout(request: Request) -> tuple[int, dict]:
    try:
        return 200, checkout.run(request.params["order_id"])
    except client.UpstreamError as exc:
        raise ApiError(exc.status, str(exc.body.get("error")), str(exc.body.get("message")))
