"""Hidden test for oss-urllib3-reject-invalid-host.

``parse_url`` must reject hosts that contain invalid characters instead of
accepting them silently. Four distinct rules must all hold, and valid hosts
(including legitimate percent-encoding) must continue to parse unchanged.
Expected behavior captured from the upstream fix (urllib3/urllib3 PR #5095).

Run with PYTHONPATH=src so the workspace's src/urllib3 is the urllib3 under test.
"""
from __future__ import annotations

import pytest

from urllib3.exceptions import LocationParseError
from urllib3.util.url import parse_url


@pytest.mark.parametrize(
    "url",
    [
        "http://ex ample.com",       # raw space
        "http://ex\x01ample.com",    # raw control character
        "http://exa\tmple.com",      # raw tab
        "http://ex\x7fample.com",    # raw DEL
    ],
)
def test_raw_invalid_host_chars_rejected(url):
    with pytest.raises(LocationParseError):
        parse_url(url)


def test_percent_encoded_control_char_rejected():
    # A percent-encoded control character (%00) must be rejected, even though it
    # is syntactically valid percent-encoding.
    with pytest.raises(LocationParseError):
        parse_url("http://ex%00ample.com")


def test_bare_percent_in_host_rejected():
    with pytest.raises(LocationParseError):
        parse_url("http://exa%mple.com")


def test_valid_hosts_still_parse():
    # Ordinary hosts and legitimate percent-encoding must be unaffected.
    assert parse_url("http://example.com/path").host == "example.com"
    assert parse_url("https://sub.example.co.uk:8443/x").host == "sub.example.co.uk"
    # %20 is valid percent-encoding and must not raise.
    parse_url("http://ex%20ample.com")
