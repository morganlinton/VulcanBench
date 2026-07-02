"""Hidden test for oss-click-version-distribution-name.

``click.version_option`` resolves the version through ``importlib.metadata``. When
the (detected or given) package name is an import module name that differs from the
installed distribution's name (e.g. module ``PIL`` shipped by distribution
``Pillow``), version lookup must fall back to resolving the module name to its
distribution via ``importlib.metadata.packages_distributions``. Expected behavior
captured from the upstream fix (pallets/click PR #3582).

A fake installed distribution (module ``mymod``, distribution ``mydist``) is
fabricated on sys.path. Run with PYTHONPATH=src so the workspace's src/click is the
click under test.
"""
from __future__ import annotations

import sys
from pathlib import Path

import pytest

import click
from click.testing import CliRunner


@pytest.fixture
def fake_dist(tmp_path: Path):
    # Module name "mymod", distribution name "mydist" 1.2.3.
    (tmp_path / "mymod.py").write_text("x = 1\n")
    di = tmp_path / "mydist-1.2.3.dist-info"
    di.mkdir()
    (di / "METADATA").write_text("Metadata-Version: 2.1\nName: mydist\nVersion: 1.2.3\n")
    (di / "top_level.txt").write_text("mymod\n")
    (di / "RECORD").write_text("mymod.py,,\n")
    sys.path.insert(0, str(tmp_path))
    try:
        yield
    finally:
        sys.path.remove(str(tmp_path))


def _cli(package_name: str):
    @click.command()
    @click.version_option(package_name=package_name)
    def cli():
        pass

    return cli


def test_version_resolves_module_name_to_distribution(fake_dist):
    # "mymod" is not a distribution; it is the import name of distribution
    # "mydist". Version lookup must resolve it instead of erroring out.
    result = CliRunner().invoke(_cli("mymod"), ["--version"])
    assert result.exit_code == 0, result.output
    assert "1.2.3" in result.output


def test_version_direct_distribution_name_stable(fake_dist):
    # Looking up an actual installed distribution name keeps working.
    result = CliRunner().invoke(_cli("mydist"), ["--version"])
    assert result.exit_code == 0, result.output
    assert "1.2.3" in result.output
