import hashlib,json,os,pathlib,signal,subprocess,time
prefix='/tmp/next-gui-filter-menu-v1-';root=pathlib.Path(prefix+'native-keys');root.mkdir(exist_ok=True);results={}
for theme in ['dark','light']:
 for mode in ['column','column-narrow','global','global-narrow']:
  name=theme+'-'+mode;out=root/name;out.mkdir(exist_ok=True)
  page=('filter-column' if mode.startswith('column') else 'filter-auto')+('-narrow' if mode.endswith('narrow') else '')
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
    def shot(stage):subprocess.run(['/usr/sbin/screencapture','-x','-l',str(window),str(out/(stage+'.png'))],check=True)
    def key(action):subprocess.run([prefix+'native-key',str(app.pid),action],stdout=subprocess.DEVNULL,check=True)
    shot('before');key('open');shot('menu');key('expressions');shot('submenu');key('comparison' if mode.startswith('column') else 'regexp');shot('placeholder');key('number' if mode.startswith('column') else 'word');shot('result');key('undo');shot('undo');key('redo');shot('redo');key('clear');shot('cleared')
    results[name]={'captured':True,'pid':app.pid,'window':window,'expected_filtered':3 if mode.startswith('column') else 2,'expected_cleared':5}
   finally:
    if app.poll() is None:app.send_signal(signal.SIGINT);app.wait(timeout=8)
    results.setdefault(name,{})['owned_exit']=app.returncode;print(name+' owned exit='+str(app.returncode),flush=True);(root/'results.json').write_text(json.dumps(results,indent=2))
(root/'binary.json').write_text(json.dumps({'sha256':hashlib.sha256(pathlib.Path('target/debug/e28_db_browser').read_bytes()).hexdigest()}))
