"""HTTP serving: a thin JSON router over the stdlib threading server.

Handlers are ``(request) -> (status, body_dict)``; paths may contain
``{param}`` segments. Unhandled exceptions become 500s with an error body,
domain errors raise :class:`ApiError` with their own status.
"""

from __future__ import annotations

import json
import re
import threading
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Callable

from orderflow.ofkit.log import get_logger

Handler = Callable[["Request"], tuple[int, dict]]


class ApiError(Exception):
    def __init__(self, status: int, code: str, message: str):
        super().__init__(message)
        self.status = status
        self.code = code
        self.message = message


@dataclass
class Request:
    method: str
    path: str
    params: dict[str, str]
    query: dict[str, str]
    headers: dict[str, str]
    body: dict[str, Any] = field(default_factory=dict)

    def require(self, *names: str) -> list[Any]:
        missing = [n for n in names if n not in self.body]
        if missing:
            raise ApiError(422, "missing_field", f"missing fields: {', '.join(missing)}")
        return [self.body[n] for n in names]


class Router:
    def __init__(self, service: str):
        self.service = service
        self.log = get_logger(service)
        self._routes: list[tuple[str, re.Pattern[str], Handler]] = []

    def route(self, method: str, pattern: str) -> Callable[[Handler], Handler]:
        regex = re.compile(
            "^" + re.sub(r"\{(\w+)\}", r"(?P<\1>[^/]+)", pattern) + "$"
        )

        def register(handler: Handler) -> Handler:
            self._routes.append((method.upper(), regex, handler))
            return handler

        return register

    def dispatch(self, request: Request) -> tuple[int, dict]:
        for method, regex, handler in self._routes:
            match = regex.match(request.path)
            if match and method == request.method:
                request.params = match.groupdict()
                return handler(request)
        return 404, {"error": "not_found", "message": f"no route for {request.path}"}


def serve(router: Router, port: int = 0) -> tuple[ThreadingHTTPServer, int]:
    """Start serving on ``port`` (0 = ephemeral) in a daemon thread."""

    class _Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def _run(self) -> None:
            length = int(self.headers.get("Content-Length") or 0)
            raw = self.rfile.read(length) if length else b""
            try:
                body = json.loads(raw) if raw else {}
                if not isinstance(body, dict):
                    raise ValueError("body must be an object")
            except ValueError:
                self._reply(400, {"error": "bad_json", "message": "body must be a JSON object"})
                return
            path, _, query_string = self.path.partition("?")
            query = {}
            for pair in query_string.split("&"):
                if "=" in pair:
                    key, _, value = pair.partition("=")
                    query[key] = value
            request = Request(
                method=self.command,
                path=path,
                params={},
                query=query,
                headers={k.lower(): v for k, v in self.headers.items()},
                body=body,
            )
            try:
                status, payload = router.dispatch(request)
            except ApiError as exc:
                status, payload = exc.status, {"error": exc.code, "message": exc.message}
            except Exception as exc:  # noqa: BLE001 - boundary
                from orderflow.ofkit.client import UpstreamError

                if isinstance(exc, UpstreamError):
                    # A proxied call failed downstream; hand the caller the
                    # downstream verdict instead of masking it as our 500.
                    status, payload = exc.status, exc.body
                else:
                    router.log.error(
                        "unhandled error on %s %s: %r", self.command, path, exc
                    )
                    status, payload = 500, {"error": "internal", "message": str(exc)}
            self._reply(status, payload)

        def _reply(self, status: int, payload: dict) -> None:
            data = json.dumps(payload).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self) -> None:  # noqa: N802 - stdlib API
            self._run()

        def do_POST(self) -> None:  # noqa: N802
            self._run()

        def do_DELETE(self) -> None:  # noqa: N802
            self._run()

        def log_message(self, *args: object) -> None:  # quiet access log
            pass

    server = ThreadingHTTPServer(("127.0.0.1", port), _Handler)
    thread = threading.Thread(
        target=server.serve_forever, name=f"{router.service}-http", daemon=True
    )
    thread.start()
    return server, server.server_address[1]
