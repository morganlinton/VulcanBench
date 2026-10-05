import hashlib
from pathlib import Path

EXPECTED = {'lib/batch.mli': '11737aae6a4a8463ed9d8f24d03bca948f01453c7c970005b48f56c3844ce599', 'lib/interval_map.mli': '22e65f8c1d53b8bf0c6ebcb77ec1bc42f5684921b8fe005cff459ae2c5468fd3'}
for name, digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
