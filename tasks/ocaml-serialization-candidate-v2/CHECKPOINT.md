# Serialization v2 checkpoint

Status: stopped for prospective short-stream integration reference correction.
Frozen 040052f00a9c preserves one complete native pass (249.420 seconds) and an
interrupted second trace. The passing solver corrected Utils.bin_read_stream
for payloads shorter than its reusable 8-byte header buffer. Its source review
revealed that the reference missed this specified integration behavior.
SHORT_STREAM_REFERENCE.json reproduces the original reference failure offline.

Create v3 with unchanged public issue/source and a corrected gold Utils payload
subview, add tiny/empty Atom and every short string payload stream assertion,
and preserve the old reference as a compiling control. Revalidate and calibrate
three entirely fresh attempts. Earlier scores are not changed or pooled.
