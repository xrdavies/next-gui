import concurrent.futures,json,os,pathlib,signal,subprocess,time
out=pathlib.Path('/tmp/next-gui-auto-filter-v2-office-native');out.mkdir(exist_ok=True)
def capture(name):
    env=os.environ.copy();env['NEXT_GUI_GPU_SURFACE']='1'
    if name=='e28_db_browser':env['NEXT_GUI_DB_BROWSER_QA_TAB']='table-fields'
    with (out/(name+'.log')).open('w') as log:
        p=subprocess.Popen(['target/debug/'+name],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        try:
            for _ in range(40):
                time.sleep(1)
                info=subprocess.run(['/tmp/next-gui-own-window-probe',str(p.pid)],capture_output=True,text=True,check=True).stdout
                ws=[json.loads(l) for l in info.splitlines() if l.startswith('{')]
                ws=[w for w in ws if w.get('kCGWindowLayer')==0 and w.get('kCGWindowIsOnscreen') and w.get('kCGWindowBounds',{}).get('Width',0)>500 and w.get('kCGWindowBounds',{}).get('Height',0)>300]; ws.sort(key=lambda w:w['kCGWindowBounds']['Width']*w['kCGWindowBounds']['Height'],reverse=True)
                if ws:
                    time.sleep(1)
                    (out/(name+'.windows.txt')).write_text(info)
                    subprocess.run(['/usr/sbin/screencapture','-x','-l',str(ws[0]['kCGWindowNumber']),str(out/(name+'.png'))],check=True)
                    (out/(name+'.windows.txt')).write_text(info)
                    return name+' captured'
                if p.poll() is not None:break
            return name+' NO WINDOW exit='+str(p.poll())
        finally:
            if p.poll() is None:
                p.send_signal(signal.SIGINT)
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:p.terminate();p.wait(timeout=5)
            print(name+' owned child exit='+str(p.returncode),flush=True)
names=['e10_office_panel','e11_office_shell','e12_office_inputs','e13_office_data','e14_office_nav','e15_office_feedback','e20_office_industrial','e21_office_p1_rest','e22_office_p1_deferred','e23_office_charts','e24_profile_dual','e25_immediate_viewport','e26_office_p2','e27_theme_tokens']
with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
    for result in pool.map(capture,names):
        print(result,flush=True)
        if not result.endswith(' captured'):raise SystemExit(1)
