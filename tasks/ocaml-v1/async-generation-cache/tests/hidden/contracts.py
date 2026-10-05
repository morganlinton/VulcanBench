import hashlib
from pathlib import Path

EXPECTED = {'lib/policy.mli': '20e97ec94f6b68f72eacc9b31344b5d8caeda202a81d8b0a73692a9dd255c1fc', 'lib/cache.mli': 'aaa3eb7cb7e16e0139bbdb28f39654a703f2e98521648e52dd8a37c3b55b5f40', 'lib/alarm.mli': '1959a565ee92bad45114698b7c90d8944da389ae36816ef36739d36ec69064da', 'lib/request.mli': '76694ede2057198658da6c47723f9b61c583469ad61219a091b50a775be39401', 'lib/client.mli': '976580016587123f4e7d95fb82854567443f75cbe27fa29eb4a449d9f65aba10'}
for name,digest in EXPECTED.items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==digest,name
