The client cannot suppress the library-identification metadata it sends to the server, because passing `None` is indistinguishable from not passing the argument at all.

On connect, redis-py sends handshake metadata identifying the client library name and version (the `LIB-NAME` / `LIB-VER` fields). A user who wants to omit this (for privacy, or because a wrapping library sets its own) would naturally pass `None`. Today `None` is treated as "not provided" and gets overwritten by the auto-detected default, so there is no way to say "send nothing here."

Introduce a proper three-way distinction for the library name and version, consistently across the metadata configuration surface (the `DriverInfo` type and the `resolve_driver_info` resolver):

- Argument omitted entirely: apply the default (name `"redis-py"`, auto-detected version).
- Argument explicitly `None`: suppress that field. It must not be replaced by a default, and it must not be sent.
- Argument set to a value: use that value.

Concretely: `DriverInfo(lib_version=None).lib_version` must stay `None` (not an auto-filled version); `DriverInfo(name=None)` must produce no formatted name; resolving with an explicit `None` for a field must suppress it, and resolving with every field explicitly `None` must produce no metadata object at all. Omitting the arguments must still yield the redis-py defaults.
