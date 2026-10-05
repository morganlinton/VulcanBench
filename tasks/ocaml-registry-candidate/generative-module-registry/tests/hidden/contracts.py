import hashlib
from pathlib import Path
EXPECTED={'dune-project': '99c52792bd3d523825f0a674b52adf6b341fa019fdbb7f1ff8cc560a0beeeab5', 'public/dune': 'a9693e8caf57210b5fb1fadaddca2690b0b1242913f87139fbd237d8cd7faeb8', 'public/smoke.ml': '60ceded178bfe4ddaf5336a9df578303110a37005677b6723ed99d8354493c32', 'lib/plugin.ml': '05e2837eaf67dfbff94818be3f578e954ae1c657afc392618dc2b6e1931fd35c', 'lib/key.mli': 'b08644504c3de09da9a156e9056aaa4d10e348cd3f32500da6131b9fce030389', 'lib/dune': '79b14b997484841626344b3b947ce2d5314e761264c8b4a24149f3dc05e0aafb', 'lib/registry.mli': '0a876eefc8489ae266402d255a267b1cff2de77c1d063196013cc4f753386530'}
for name,digest in EXPECTED.items():
 assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==digest,name
