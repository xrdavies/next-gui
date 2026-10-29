import hashlib,json,pathlib,subprocess,time
prefix='/tmp/next-gui-filter-menu-v1-'
for _ in range(1200):
 p=pathlib.Path(prefix+'checks.json');checks=json.loads(p.read_text()) if p.exists() else {}
 assert all(c['exit']==0 for c in checks.values()),checks
 if len(checks)==5:break
 time.sleep(1)
else:raise SystemExit('Final checks did not finish')
sources=json.loads(pathlib.Path(prefix+'source.json').read_text());assert len(sources)==157
assert all(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==sha for p,sha in sources.items())
binary=hashlib.sha256(pathlib.Path('target/debug/e28_db_browser').read_bytes()).hexdigest();pathlib.Path(prefix+'binary.json').write_text(json.dumps({'sha256':binary},indent=2))
subprocess.run(['swiftc',prefix+'native-key.swift','-o',prefix+'native-key'],check=True)
subprocess.run(['python3',prefix+'native-keys.py'],check=True)
subprocess.run(['python3',prefix+'office-native.py'],check=True)
assert hashlib.sha256(pathlib.Path('target/debug/e28_db_browser').read_bytes()).hexdigest()==binary
assert all(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==sha for p,sha in sources.items())
