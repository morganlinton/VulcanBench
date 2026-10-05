import hashlib
from pathlib import Path

EXPECTED = {'lib/eval.mli': '6ceed4398bb3e53d3a5f10a58879c1bed8ec3427a2fb44f3181d8aa0c30abbfe', 'lib/expr.mli': '4995aa7b2bc660638afe9440d55c0f13c15fc60ddb54571223c75f74a3918ecd', 'lib/optimizer.mli': '4ae78839579573222dfcef9ebc1feb7205fb3fb6bdc58810082e6af323463b27', 'lib/measure.mli': '182a9f38b0ddd18a389b3b11067c12e04211ec2a0d7f9eefa6158cb46b821e86'}
for name, digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
