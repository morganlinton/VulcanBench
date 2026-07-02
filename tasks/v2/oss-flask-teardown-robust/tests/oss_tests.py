"""Hidden test for oss-flask-teardown-robust.

When a teardown callback (``teardown_request`` / ``teardown_appcontext``) or a
teardown signal receiver raises, every remaining teardown callback and receiver must
still run, and the collected errors must propagate as an ``ExceptionGroup`` instead
of the first error aborting teardown. Expected behavior captured from the upstream
fix (pallets/flask PR #5928). Assertions are structural (which callbacks ran, group
nesting and leaf exceptions), not tied to exact message wording.

Run with PYTHONPATH=src so the workspace's src/flask is the flask under test.
"""
from __future__ import annotations

import pytest

import flask


def _build_app(count: list[str]):
    app = flask.Flask("flask_test")
    app.testing = True

    @app.teardown_request
    def request_teardown(e):
        count.append("request_teardown")
        raise ValueError("request_teardown")

    @app.teardown_appcontext
    def app_teardown(e):
        count.append("app_teardown")
        raise ValueError("app_teardown")

    @app.get("/")
    def index():
        return ""

    return app


def _leaf_exceptions(exc):
    if isinstance(exc, BaseExceptionGroup):
        leaves = []
        for sub in exc.exceptions:
            leaves.extend(_leaf_exceptions(sub))
        return leaves
    return [exc]


def test_all_teardown_callbacks_run_and_errors_aggregate():
    count: list[str] = []
    app = _build_app(count)

    def request_signal(sender, exc):
        count.append("request_signal")
        raise ValueError("request_signal")

    def app_signal(sender, exc):
        count.append("app_signal")
        raise ValueError("app_signal")

    client = app.test_client()
    with (
        flask.request_tearing_down.connected_to(request_signal, app),
        flask.appcontext_tearing_down.connected_to(app_signal, app),
    ):
        with pytest.raises(BaseExceptionGroup) as exc_info:
            client.get("/")

    # Every teardown callback and signal receiver ran despite the earlier raises.
    assert sorted(count) == [
        "app_signal",
        "app_teardown",
        "request_signal",
        "request_teardown",
    ]
    # All four errors surface in the propagated exception group.
    leaves = _leaf_exceptions(exc_info.value)
    assert sorted(str(e) for e in leaves) == [
        "app_signal",
        "app_teardown",
        "request_signal",
        "request_teardown",
    ]


def test_clean_teardown_unaffected():
    # Teardown callbacks that do not raise must keep working exactly as before.
    app = flask.Flask("flask_test")
    app.testing = True
    called: list[str] = []

    @app.teardown_request
    def request_teardown(e):
        called.append("request_teardown")

    @app.teardown_appcontext
    def app_teardown(e):
        called.append("app_teardown")

    @app.get("/")
    def index():
        return "ok"

    client = app.test_client()
    resp = client.get("/")
    assert resp.status_code == 200
    assert called == ["request_teardown", "app_teardown"]
