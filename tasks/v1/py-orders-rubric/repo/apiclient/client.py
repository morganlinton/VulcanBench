"""A small JSON API client.

House style: every method goes through ``_request`` (which adds auth and turns
HTTP errors into ``ApiError``) and parses the body with ``_parse``.
"""

import json
import urllib.error
import urllib.request


class ApiError(Exception):
    """Raised when the API returns a non-2xx response."""

    def __init__(self, status, message):
        super().__init__(f"{status}: {message}")
        self.status = status
        self.message = message


class ApiClient:
    def __init__(self, base_url, token):
        self._base = base_url.rstrip("/")
        self._token = token

    def _request(self, method, path):
        req = urllib.request.Request(
            self._base + path,
            method=method,
            headers={"Authorization": f"Bearer {self._token}"},
        )
        try:
            with urllib.request.urlopen(req) as resp:
                return resp.read()
        except urllib.error.HTTPError as e:
            raise ApiError(e.code, e.reason)

    def _parse(self, body):
        return json.loads(body)

    def get_user(self, user_id):
        """Return the user record for user_id."""
        return self._parse(self._request("GET", f"/users/{user_id}"))

    def get_order(self, order_id):
        """Return the order record for order_id."""
        return self._parse(self._request("GET", f"/orders/{order_id}"))
