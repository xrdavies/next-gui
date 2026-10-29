import hashlib,json,os,pathlib,subprocess
prefix='/tmp/next-gui-auto-filter-v2-';root=pathlib.Path(prefix+'native');root.mkdir(exist_ok=True);results={}
for theme in ['dark','light']:
 for page in ['prefs','prefs-narrow']:
  key=theme+'-'+page;out=root/key
  env=os.environ.copy();env.update(NEXT_GUI_WIZARD_NATIVE_OUT=str(out),NEXT_GUI_WIZARD_NATIVE_TAB=page,NEXT_GUI_DB_BROWSER_QA_THEME=theme)
  if page.endswith('narrow'):env.update(NEXT_GUI_NATIVE_MIN_WIDTH='800',NEXT_GUI_NATIVE_MIN_HEIGHT='600')
  log=root/(key+'.driver.log')
  with log.open('w') as output:result=subprocess.run(['python3','/tmp/next-gui-wizard-native-ready-probe.py'],env=env,stdout=output,stderr=subprocess.STDOUT)
  assert ('ready '+('800x600' if page.endswith('narrow') else '1400x860')) in (out/'e28_db_browser.log').read_text()
  results[key]={'exit':result.returncode,'driver':str(log)};(root/'results.json').write_text(json.dumps(results,indent=2));print(key+' exit='+str(result.returncode),flush=True);assert result.returncode==0
(root/'binary.json').write_text(json.dumps({'sha256':hashlib.sha256(pathlib.Path('target/debug/e28_db_browser').read_bytes()).hexdigest()}))
