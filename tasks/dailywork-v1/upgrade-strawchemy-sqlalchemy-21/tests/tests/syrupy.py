from __future__ import annotations

from typing import TYPE_CHECKING

from syrupy.extensions.amber import AmberSnapshotExtension
from syrupy.extensions.amber.serializer import AmberDataSerializer
from syrupy.extensions.single_file import SingleFileSnapshotExtension, WriteMode
from syrupy.utils import exclusive_file_lock
from typing_extensions import override

from tests.utils import IS_SQLALCHEMY_20, strip_loader_labels

if TYPE_CHECKING:
    from syrupy.data import SnapshotCollection
    from syrupy.types import PropertyFilter, PropertyMatcher, SerializableData, SerializedData

__all__ = (
    "GraphQLFileExtension",
    "SnapshotMergeError",
    "SnapshotWriteError",
    "VerifiedAmberSnapshotExtension",
)


class SnapshotMergeError(RuntimeError):
    """A merged amber write dropped entries that were on disk."""


class SnapshotWriteError(RuntimeError):
    """SQL snapshots were written on a SQLAlchemy version other than 2.1."""


class GraphQLFileExtension(SingleFileSnapshotExtension):
    file_extension = "gql"
    _write_mode = WriteMode.TEXT

    @override
    def serialize(
        self,
        data: SerializableData,
        *,
        exclude: PropertyFilter | None = None,
        include: PropertyFilter | None = None,
        matcher: PropertyMatcher | None = None,
    ) -> SerializedData:
        return AmberDataSerializer.serialize(data, exclude=exclude, include=include, matcher=matcher)


class VerifiedAmberSnapshotExtension(AmberSnapshotExtension):
    @override
    def matches(self, *, serialized_data: SerializedData, snapshot_data: SerializedData) -> bool:
        if IS_SQLALCHEMY_20:
            return strip_loader_labels(str(serialized_data)) == strip_loader_labels(str(snapshot_data))
        return super().matches(serialized_data=serialized_data, snapshot_data=snapshot_data)

    @classmethod
    @override
    def write_snapshot_collection(
        cls, *, snapshot_collection: SnapshotCollection, name_order: dict[str, int] | None = None
    ) -> None:
        if IS_SQLALCHEMY_20:
            msg = "SQL snapshots are only written on SQLAlchemy 2.1"
            raise SnapshotWriteError(msg)
        location = snapshot_collection.location
        # Reading the entries before and after the merge only proves anything while no other
        # process can write in between, so the whole check runs inside the write lock.
        with exclusive_file_lock(location):
            expected = _entry_names(cls.serializer_class.read_file(location)) | _entry_names(snapshot_collection)
            cls.serializer_class.write_file(
                snapshot_collection, merge=True, name_order=name_order, file_lock=True, _already_locked=True
            )
            dropped = expected - _entry_names(cls.serializer_class.read_file(location))
        if dropped:
            msg = (
                f"Merged write of '{location}' dropped {len(dropped)} entries, "
                f"starting with '{min(dropped)}'. Concurrent writers clobbered the file: "
                f"restore it, then re-run the update with --snapshot-file-lock or -n=0."
            )
            raise SnapshotMergeError(msg)


def _entry_names(snapshot_collection: SnapshotCollection) -> set[str]:
    return {snapshot.name for snapshot in snapshot_collection}
