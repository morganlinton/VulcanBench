"""Form field validators. Each validator raises ValidationError on bad input and
returns the cleaned value on success."""


class ValidationError(Exception):
    def __init__(self, field, message):
        super().__init__(f"{field}: {message}")
        self.field = field
        self.message = message


def validate_email(value):
    value = value.strip()
    if "@" not in value:
        raise ValidationError("email", "must contain @")
    return value.lower()


def validate_age(value):
    if not isinstance(value, int):
        raise ValidationError("age", "must be an integer")
    if value < 0 or value > 150:
        raise ValidationError("age", "out of range")
    return value
