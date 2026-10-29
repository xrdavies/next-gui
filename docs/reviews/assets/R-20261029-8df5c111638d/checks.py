import json,os,subprocess
prefix='/tmp/next-gui-auto-filter-v2-'
checks=[('clippy',['cargo','--offline','clippy','--workspace','--all-targets','--','-D','warnings']),('workspace',['cargo','--offline','test','--workspace']),('build',['cargo','--offline','build','--workspace']),('macos',['cargo','--offline','check','-p','next-gui-platform','--features','macos']),('windows',['cargo','--offline','check','-p','next-gui-platform','--features','windows'])]
results={}
for name,command in checks:
    env=os.environ.copy()
    if name=='render':env['NEXT_GUI_PLOT_HOVER_QA_OUT']=prefix+'render'
    log=prefix+name+'-final.log'
    with open(log,'w') as out:r=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT)
    results[name]={'exit':r.returncode,'log':log}
    open(prefix+'checks.json','w').write(json.dumps(results,indent=2))
    print(name+' exit='+str(r.returncode),flush=True)
    if name=='focused' and r.returncode:raise SystemExit(r.returncode)
raise SystemExit(any(r['exit'] for r in results.values()))
