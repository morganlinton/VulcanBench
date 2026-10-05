import hashlib
from pathlib import Path
EXPECTED={'lib/codec.mli': 'af449506876fd5a6e452cc8543ae269d913a756fba79fce9bffc1de887822f1c', 'lib/domain.mli': 'e29a8d05f19a9141601c385e3ece3e7cd3d832c999fa81614d469fc338aed5e9', 'lib/encoder.mli': 'fae3dbbb9c280b0e07e76f2b41c85ebe9f55216c42e9ca3abafa87f0240c8281', 'lib/archive.mli': '70c09c8a7b5c7a6ecb73d92346d99022d8811f3ee561d8775aa324489c5126be', 'lib/decoder.mli': '8818011f49cc99f8a8e96e3a3683b1cb25086ada1de6691ddf14737bf950a3aa', 'lib/error.mli': '5e976527b043e52d7bd41ccfa82d3daa05858cc1994698f483429a40a1424c35', 'lib/schema.mli': 'f4c5df064f1dbf22f40c7942b13281693c45bb7d5f302a0b4d454006b05ec608', 'lib/wire.mli': '575ecdaec6249007beef0cea7162a706d4277b4c2ebc9cfbb644a3843d33f5cc'}
for name,digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==digest,name

for p in Path('lib').glob('*.ml'):
    source=p.read_text()
    assert not any(x in source for x in ('Obj.magic','Obj.obj','Marshal.from','external ')),p
