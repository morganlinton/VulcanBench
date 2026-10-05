import hashlib
from pathlib import Path

EXPECTED = {'lib/pipeline.mli': 'ef097b4b6ffb0d3c2c75f66e011676547a9db5ced1183acbf9ca813b663d59b7'}
for name, digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
