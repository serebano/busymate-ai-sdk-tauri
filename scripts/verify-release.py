from pathlib import Path
import hashlib, json, re
root = Path(__file__).resolve().parents[1]
m = json.loads((root / 'release-manifest.json').read_text())
for name, expected in m['files'].items():
    actual = hashlib.sha256((root / name).read_bytes()).hexdigest()
    assert actual == expected, f"Pinned source changed: {name}"
    print(f"verified {name} {actual}")
assert m['version'] == '1.0.2'
assert m['identityWire']['v2'] == 2 and m['identityWire']['v2Build'] == '2.0.0'
assert m['microphoneWire'] == 1 and m['microphoneAdapter'] == '1.0.0'

# Every distribution surface states the same versions.
crate = (root / 'plugin/Cargo.toml').read_text()
assert re.search(r'^version = "1\.0\.2"$', crate, re.M), 'plugin crate version'
assert json.loads((root / 'plugin/guest-js/package.json').read_text())['version'] == '1.0.2'
assert m['defaultChannel'] == 'desktop_sdk', 'default channel'
assert "channel=desktop_sdk&" in (root / 'plugin/src/lib.rs').read_text(), 'Rust default chat URL'
assert "channel=desktop_sdk&" in (root / 'plugin/guest-js/index.js').read_text(), 'JS default chat URL'
source = (root / 'plugin/src/bridge.rs').read_text()
assert 'pub const BUILD: &str = "2.0.0";' in source and 'pub const VERSION: i64 = 2;' in source
mic = (root / 'plugin/src/microphone.rs').read_text()
assert 'pub const ADAPTER: &str = "1.0.0";' in mic and 'busymate.microphone.v1.request' in mic
# The contract is a verbatim copy of the frozen kit v2 contract.
assert hashlib.sha256((root / 'CONTRACT-v2.md').read_bytes()).hexdigest() == m['contractSha256'], 'CONTRACT-v2.md changed'
# The remote chat only ever gets the two bridge commands; the default set is local-only.
default = (root / 'plugin/permissions/default.toml').read_text()
assert 'allow-bridge' not in default.split('permissions')[-1] and 'allow-microphone' not in default.split('permissions')[-1]
print('verified versions, contract copy and minimal default permission set')

# Every release attaches each adapter source file under its basename, listed in SHA256SUMS.
_assets = m['adapterAssets']
assert _assets, 'adapterAssets must list the microphone adapter source file(s)'
assert len({Path(a).name for a in _assets}) == len(_assets), 'adapter asset basenames must be unique'
for _a in _assets:
    assert _a in m['files'], f'adapter asset {_a} is not pinned in files'
    _sha = hashlib.sha256((root / _a).read_bytes()).hexdigest()
    assert _sha == m['files'][_a], f'adapter asset {_a} changed'
    print(f'verified adapter asset {Path(_a).name} {_sha}')
