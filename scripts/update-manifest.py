# Maintainer helper: re-pin release-manifest.json after an intentional, versioned change.
from pathlib import Path
import hashlib, json
root = Path(__file__).resolve().parents[1]
path = root / 'release-manifest.json'
m = json.loads(path.read_text())
m['files'] = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in m['files']}
m['contractSha256'] = hashlib.sha256((root / 'CONTRACT-v2.md').read_bytes()).hexdigest()
path.write_text(json.dumps(m, indent=2) + '\n')
print(json.dumps(m, indent=2))
