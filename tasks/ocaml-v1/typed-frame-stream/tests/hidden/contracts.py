import hashlib
from pathlib import Path

EXPECTED = {'lib/stream.mli': '076471db92e3dc4f6c7c2d29b7d21eb1e1e2ba7b2cc89b561e8208f9b8b1a3f4', 'lib/wire.mli': '61582ef1750db378171dd4b861ec2f5b3eaccc6f5f3ab961e8e4185e3c79b87a'}
for name, digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
