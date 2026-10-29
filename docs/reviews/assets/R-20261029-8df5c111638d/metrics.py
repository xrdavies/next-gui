import hashlib,json,pathlib,re,subprocess
prefix='/tmp/next-gui-auto-filter-v2-'
root=pathlib.Path(prefix+'native-keys');flows=json.loads((root/'results.json').read_text());assert len(flows)==8 and all(x['owned_exit']==-2 for x in flows.values())
for theme in ['dark','light']:
 frames=[str(root/name/(stage+'.png')) for name in sorted(flows) if name.startswith(theme) for stage in ['before','filtered','cleared']]
 subprocess.run(['swift','/tmp/next-gui-scroll-contact.swift',prefix+'keyboard-'+theme+'-contact.png',*frames],check=True)
static=json.loads(pathlib.Path(prefix+'native/results.json').read_text());assert len(static)==4 and all(x['exit']==0 for x in static.values())
for item in static.values():assert 'owned app exit=-2' in pathlib.Path(item['driver']).read_text()
subprocess.run(['swift','/tmp/next-gui-scroll-contact.swift',prefix+'native-contact.png',*[str(pathlib.Path(prefix+'native')/name/'e28_db_browser.png') for name in sorted(static)]],check=True)
office=pathlib.Path(prefix+'office-native');frames=sorted(office.glob('*.png'));assert len(frames)==14
subprocess.run(['swift','/tmp/next-gui-scroll-contact.swift',prefix+'office-contact.png',*[str(p) for p in frames]],check=True)
sources=json.loads(pathlib.Path(prefix+'source.json').read_text());assert len(sources)==155 and all(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==sha for p,sha in sources.items())
checks=json.loads(pathlib.Path(prefix+'checks.json').read_text());assert len(checks)==5 and all(x['exit']==0 for x in checks.values())
counts=[tuple(map(int,x)) for x in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',pathlib.Path(prefix+'workspace-final.log').read_text())];assert len(counts)==43 and sum(x[0] for x in counts)==798 and all(x[1:]==(0,0) for x in counts)
render=json.loads(pathlib.Path(prefix+'converted/metrics.json').read_text());assert len(render)==16 and all(x['max_channel_delta']<=2 and x['channels_above_2']==0 for x in render.values())
binary=json.loads(pathlib.Path(prefix+'binary.json').read_text())['sha256'];assert hashlib.sha256(pathlib.Path('target/debug/e28_db_browser').read_bytes()).hexdigest()==binary
for kind in ['native','native-keys']:assert json.loads(pathlib.Path(prefix+kind+'/binary.json').read_text())['sha256']==binary
metrics={'workspace':{'passed':798,'failed':0,'ignored':0,'groups':43,'e28':508},'checks':checks,'render':render,'source_sha256':sources,'browser_sha256':binary,'native_static':static,'native_keys':flows,'office_windows':14}
pathlib.Path(prefix+'metrics.json').write_text(json.dumps(metrics,indent=2));print('798 tests / 16 render pairs / 8 native flows / 4 prefs windows / 14 Office / 155 hashes')
