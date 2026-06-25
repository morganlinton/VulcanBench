def parse_bool(s):
    """Parse a string into a bool."""
    if s == "true":
        return True
    if s == "false":
        return False
    raise ValueError(s)
