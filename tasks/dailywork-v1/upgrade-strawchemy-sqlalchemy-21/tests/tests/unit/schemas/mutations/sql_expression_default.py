from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import TimestampedRecord

strawchemy = Strawchemy("postgresql")


@strawchemy.type(TimestampedRecord, include="all")
class TimestampedRecordType: ...


@strawchemy.create_input(TimestampedRecord, include="all")
class TimestampedRecordCreateInput: ...


@strawberry.type
class Mutation:
    create_record: TimestampedRecordType = strawchemy.create(TimestampedRecordCreateInput)
