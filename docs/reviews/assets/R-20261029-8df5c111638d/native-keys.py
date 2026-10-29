import hashlib,json,os,pathlib,signal,subprocess,time
prefix='/tmp/next-gui-auto-filter-v2-';root=pathlib.Path(prefix+'native-keys');root.mkdir(exist_ok=True);results={}
for theme in ['dark','light']:
 for mode in ['auto','auto-narrow','escape','escape-narrow']:
  key=theme+'-'+mode;out=root/key;out.mkdir(exist_ok=True)
  page='filter-'+mode
  env=os.environ.copy();env.update(NEXT_GUI_GPU_SURFACE='1',NEXT_GUI_DB_BROWSER_QA_THEME=theme,NEXT_GUI_DB_BROWSER_QA_TAB=page)
  with (out/'app.log').open('w') as log:
   app=subprocess.Popen(['target/debug/e28_db_browser'],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
   try:
    windows=[]
    for _ in range(40):
     time.sleep(1);info=subprocess.check_output(['/tmp/next-gui-own-window-probe',str(app.pid)],text=True)
     windows=[json.loads(line) for line in info.splitlines() if line.startswith('{')];windows=[w for w in windows if w.get('kCGWindowLayer')==0 and w.get('kCGWindowIsOnscreen') and w.get('kCGWindowBounds',{}).get('Width',0)>=(800 if mode.endswith('narrow') else 1400)]
     if windows and 'e28_db_browser ready' in (out/'app.log').read_text():break
     assert app.poll() is None
    assert windows,'No owned ready window'
    window=windows[0]['kCGWindowNumber'];(out/'window.json').write_text(json.dumps({'pid':app.pid,'window':window,'info':info},indent=2));time.sleep(1)
    subprocess.run(['/usr/sbin/screencapture','-x','-l',str(window),str(out/'before.png')],check=True)
    subprocess.run([prefix+'native-key',str(app.pid),'escape' if mode.startswith('escape') else 'auto'],check=True)
    subprocess.run(['/usr/sbin/screencapture','-x','-l',str(window),str(out/'filtered.png')],check=True)
    subprocess.run([prefix+'native-key',str(app.pid),'clear'],check=True)
    subprocess.run(['/usr/sbin/screencapture','-x','-l',str(window),str(out/'cleared.png')],check=True)
    results[key]={'captured':True,'pid':app.pid,'window':window}
   finally:
    if app.poll() is None:app.send_signal(signal.SIGINT);app.wait(timeout=8)
    results.setdefault(key,{})['owned_exit']=app.returncode;print(key+' owned exit='+str(app.returncode),flush=True);(root/'results.json').write_text(json.dumps(results,indent=2))
(root/'binary.json').write_text(json.dumps({'sha256':hashlib.sha256(pathlib.Path('target/debug/e28_db_browser').read_bytes()).hexdigest()}))
