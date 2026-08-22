# Wheel tags with non-identifier interpreter components are silently accepted

`parse_tag` accepts interpreter components that cannot possibly be valid
interpreter names, and compressed-set splitting makes the result actively
wrong:

```python
>>> from packaging import tags
>>> tags.parse_tag("2.7.6-none-any")
frozenset({<2-none-any>, <7-none-any>, <6-none-any>})
```

A real-world wheel named `playlyfe-0.1.1-2.7.6-none-any.whl` (the version
was accidentally repeated where the tag belongs) parses "successfully" into
three nonsense tags instead of being rejected by `parse_wheel_filename`.
Interpreter components like `2`, `2.7.6`, `py3.2` or `py+3` should raise
`InvalidTag` (and `InvalidWheelFilename` at the wheel-filename level), the
same way empty components and wrong component counts already do.

Any interpreter component that is a valid Python identifier (e.g.
`sillywalk`, `graalpy311`, `_custom`) must remain accepted — the check must
not be a hard-coded interpreter list. Compressed tag sets of valid
identifiers (`py3.cp312-none-any`) must still expand as today.
