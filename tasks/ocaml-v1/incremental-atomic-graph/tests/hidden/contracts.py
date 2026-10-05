import hashlib
from pathlib import Path

EXPECTED = {'lib/graph.mli': 'c024d45923986f0764fd09fc8557a410dc647136ebfd6df8eb5fe1a313fc43a0'}
for name, digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
