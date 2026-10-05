import hashlib
from pathlib import Path

EXPECTED = {'lib/window.mli': 'db359e617d01d0fa4dc74f5727ea6c58d394cd7a34a247e4c98fcf8300840f36'}
for name, digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
