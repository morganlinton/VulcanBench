"""Hidden test for oss-packaging-empty-name.

A wheel or sdist filename whose project-name component is empty is invalid and must
be rejected, not parsed into an empty name. Behavior captured from upstream fix
(packaging PR #1305).
"""
import pytest
from packaging.utils import (
    InvalidSdistFilename,
    InvalidWheelFilename,
    parse_sdist_filename,
    parse_wheel_filename,
)


def test_empty_name_wheel_rejected():
    with pytest.raises(InvalidWheelFilename):
        parse_wheel_filename("-1.0-py3-none-any.whl")


def test_empty_name_sdist_rejected():
    with pytest.raises(InvalidSdistFilename):
        parse_sdist_filename("-1.0.tar.gz")


def test_valid_wheel_still_parses():
    # pass_to_pass no-regression anchor.
    assert str(parse_wheel_filename("foo-1.0-py3-none-any.whl")[0]) == "foo"
