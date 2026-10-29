import hashlib,json,pathlib,re,subprocess
prefix='/tmp/next-gui-filter-menu-v1-';root=pathlib.Path(prefix+'native-keys')
flows=json.loads((root/'results.json').read_text());assert len(flows)==8 and all(x['owned_exit']==-2 for x in flows.values())
for theme in ['dark','light']:
 for kind in ['column','global']:
  frames=[str(root/(theme+'-'+kind+suffix)/(stage+'.png')) for suffix in ['', '-narrow'] for stage in ['before','menu','submenu','placeholder','result','undo','redo','cleared']]
  assert all(pathlib.Path(p).exists() for p in frames)
  subprocess.run(['swift','/tmp/next-gui-scroll-contact.swift',prefix+'keyboard-'+theme+'-'+kind+'-contact.png',*frames],check=True)
office=pathlib.Path(prefix+'office-native');frames=sorted(office.glob('*.png'));assert len(frames)==14
subprocess.run(['swift','/tmp/next-gui-scroll-contact.swift',prefix+'office-contact.png',*[str(p) for p in frames]],check=True)
sources=json.loads(pathlib.Path(prefix+'source.json').read_text());assert len(sources)==157 and all(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==sha for p,sha in sources.items())
checks=json.loads(pathlib.Path(prefix+'checks.json').read_text());assert len(checks)==5 and all(x['exit']==0 for x in checks.values())
counts=[tuple(map(int,x)) for x in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',pathlib.Path(prefix+'workspace-final.log').read_text())];assert len(counts)==43 and sum(x[0] for x in counts)==806 and all(x[1:]==(0,0) for x in counts)
render=json.loads(pathlib.Path(prefix+'converted/metrics.json').read_text());assert len(render)==13 and all(x['max_channel_delta']<=2 and x['channels_above_2']==0 for x in render.values())
binary=json.loads(pathlib.Path(prefix+'binary.json').read_text())['sha256'];assert hashlib.sha256(pathlib.Path('target/debug/e28_db_browser').read_bytes()).hexdigest()==binary
assert json.loads(pathlib.Path(prefix+'native-keys/binary.json').read_text())['sha256']==binary
metrics={'workspace':{'passed':806,'failed':0,'ignored':0,'groups':43,'e28':511,'shared_ui':221},'checks':checks,'render':render,'source_sha256':sources,'browser_sha256':binary,'native_keys':flows,'office_windows':14,'ordinary_frontmost_input':'not_run','native_ime_a11y_print_pickers':'not_run','optional_ocr_first_attempt':'failed: Vision e5rtError 13 / exit 133; visual review used'}
pathlib.Path(prefix+'metrics.json').write_text(json.dumps(metrics,indent=2));print('806 tests / 13 render pairs / 8 native flows / 14 Office / 157 hashes; browser '+binary)
