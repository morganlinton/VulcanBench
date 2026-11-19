from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from syrupy.data import Snapshot, SnapshotCollection
from syrupy.extensions.amber.serializer import AmberDataSerializer

from tests.syrupy import SnapshotMergeError, SnapshotWriteError, VerifiedAmberSnapshotExtension

if TYPE_CHECKING:
    from pathlib import Path

_ENTRY_COUNT = 120
_WORKERS = 4
# The pretty plugin makes pytester unable to parse pytest output, the databases one talks to docker
_PYTEST_ARGS = ("-p", "no:pretty", "-p", "no:pytest_databases")

_PARALLEL_UPDATE_MODULE = """
import pytest

BODY = "\\n".join("line " + str(index) + " " + "x" * 60 for index in range(40))


@pytest.mark.parametrize("case", range(ENTRY_COUNT))
def test_entry(case, snapshot):
    assert "case=" + str(case) + "\\n" + BODY == snapshot
""".replace("ENTRY_COUNT", str(_ENTRY_COUNT))


def _collection(location: Path, names: list[str]) -> SnapshotCollection:
    collection = SnapshotCollection(location=str(location))
    for name in names:
        collection.add(Snapshot(name=name, data=f"data of {name}"))
    return collection


def test_snapshot_file_lock_is_enabled(pytestconfig: pytest.Config) -> None:
    """Test that merged amber writes are serialized for every run of the suite."""
    assert pytestconfig.getoption("snapshot_file_lock") is True


def test_parallel_update_keeps_every_entry(pytester: pytest.Pytester) -> None:
    """Test that a --snapshot-update spread over xdist workers writes every entry of a merged file."""
    pytester.makepyprojecttoml("[tool.pytest.ini_options]\n")
    pytester.makepyfile(test_entries=_PARALLEL_UPDATE_MODULE)

    result = pytester.runpytest_subprocess(*_PYTEST_ARGS, f"-n={_WORKERS}", "--snapshot-update", "--snapshot-file-lock")

    result.assert_outcomes(passed=_ENTRY_COUNT)
    written = AmberDataSerializer.read_file(str(pytester.path / "__snapshots__" / "test_entries.ambr"))
    assert {snapshot.name for snapshot in written} == {f"test_entry[{index}]" for index in range(_ENTRY_COUNT)}


def test_merge_dropping_entries_raises(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Test that a write losing entries already on disk fails instead of leaving them orphaned."""
    location = tmp_path / "snapshots.ambr"
    AmberDataSerializer.write_file(_collection(location, ["test_first", "test_second"]))

    write_file = AmberDataSerializer.write_file

    def clobber(snapshot_collection: SnapshotCollection, **_unused: object) -> None:
        write_file(snapshot_collection, merge=False, file_lock=True, _already_locked=True)

    monkeypatch.setattr(VerifiedAmberSnapshotExtension.serializer_class, "write_file", clobber)
    monkeypatch.setattr("tests.syrupy.IS_SQLALCHEMY_20", False)

    with pytest.raises(SnapshotMergeError, match="dropped 2 entries, starting with 'test_first'"):
        VerifiedAmberSnapshotExtension.write_snapshot_collection(
            snapshot_collection=_collection(location, ["test_third"])
        )


def test_snapshot_write_refused_on_sqlalchemy_20(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Test that writing snapshots fails on SQLAlchemy 2.0 and leaves the file untouched."""
    location = tmp_path / "snapshots.ambr"
    monkeypatch.setattr("tests.syrupy.IS_SQLALCHEMY_20", True)

    with pytest.raises(SnapshotWriteError, match=r"SQLAlchemy 2\.1"):
        VerifiedAmberSnapshotExtension.write_snapshot_collection(
            snapshot_collection=_collection(location, ["test_first"])
        )

    assert not location.exists()


@pytest.mark.parametrize("is_20", [True, False])
def test_snapshot_matches_ignores_loader_labels_on_sqlalchemy_20(is_20: bool, monkeypatch: pytest.MonkeyPatch) -> None:
    """Test that loader labels only stop mattering when running on SQLAlchemy 2.0."""
    monkeypatch.setattr("tests.syrupy.IS_SQLALCHEMY_20", is_20)
    extension = VerifiedAmberSnapshotExtension()

    assert extension.matches(serialized_data="SELECT t.c", snapshot_data="SELECT t.c AS t_c") is is_20
