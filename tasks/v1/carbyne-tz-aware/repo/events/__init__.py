"""Event helpers. All timestamps are timezone-aware UTC datetimes."""

from datetime import datetime, timezone


def now():
    return datetime.now(timezone.utc)


def age_seconds(event):
    """Seconds since the event's created_at (a timezone-aware UTC datetime)."""
    return (now() - event["created_at"]).total_seconds()
