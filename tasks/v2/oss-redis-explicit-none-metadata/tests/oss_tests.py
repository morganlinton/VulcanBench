"""Hidden test for oss-redis-explicit-none-metadata.

Client handshake metadata (library name and version) must distinguish "not
provided" (fall back to the default) from "explicitly set to None" (do not send
this field). Passing ``None`` for ``name`` or ``lib_version`` must suppress that
field rather than being overwritten by an auto-detected default; omitting the
argument entirely must still apply the default. This distinction must hold both
on ``DriverInfo`` directly and through ``resolve_driver_info``. Expected behavior
captured from the upstream fix (redis/redis-py PR #4081).

Run with PYTHONPATH=. so the workspace's redis/ is the redis under test.
"""
from __future__ import annotations

from redis.driver_info import DriverInfo, resolve_driver_info


def test_explicit_none_version_is_not_autofilled():
    # Passing lib_version=None means "do not send a version"; it must stay None,
    # not be overwritten by an auto-detected package version.
    info = DriverInfo(lib_version=None)
    assert info.lib_version is None


def test_explicit_none_name_is_not_sent():
    # Passing name=None means "do not send a lib name"; formatted_name must be None.
    info = DriverInfo(name=None)
    assert info.formatted_name is None


def test_resolve_respects_explicit_none():
    # The same distinction must hold through resolve_driver_info: explicit None
    # for a single field suppresses it, and explicit None for everything means
    # send no metadata at all (resolve returns None).
    assert resolve_driver_info(lib_version=None).lib_version is None
    assert resolve_driver_info(lib_name=None, lib_version=None) is None


def test_defaults_applied_when_omitted():
    # Omitting the arguments entirely must still apply the redis-py defaults.
    info = DriverInfo()
    assert info.name == "redis-py"
    assert isinstance(info.lib_version, str) and info.lib_version

    resolved = resolve_driver_info()
    assert resolved.name == "redis-py"
    assert isinstance(resolved.lib_version, str) and resolved.lib_version
