import json,pathlib,subprocess
root=pathlib.Path('/tmp/next-gui-filter-menu-v1-render');out=pathlib.Path('/tmp/next-gui-filter-menu-v1-converted');out.mkdir(exist_ok=True)
files=sorted(root.glob('*.ppm'));assert len(files)==26,files
frames={}
for path in files:
 raw=path.read_bytes();magic,dimensions,limit,body=raw.split(b'\n',3)
 assert magic==b'P6' and limit==b'255'
 w,h=map(int,dimensions.split());assert len(body)==w*h*3
 frames[path.stem]=(w,h,body)
 subprocess.run(['/usr/bin/sips','-s','format','png',str(path),'--out',str(out/(path.stem+'.png'))],stdout=subprocess.DEVNULL,check=True)
metrics={}
for name,(w,h,a) in frames.items():
 if name.endswith('-metal'):continue
 mw,mh,b=frames[name+'-metal'];assert (w,h)==(mw,mh)
 maximum=0;above=0
 for x,y in zip(a,b):
  delta=abs(x-y);maximum=max(maximum,delta);above+=delta>2
 assert maximum<=2,(name,maximum,above)
 metrics[name]={'width':w,'height':h,'max_channel_delta':maximum,'channels_above_2':above}
(out/'metrics.json').write_text(json.dumps(metrics,indent=2));print(json.dumps(metrics),flush=True)
for kind in ['soft','metal']:
 pngs=[str(out/(name+'.png')) for name in sorted(frames) if name.endswith('-metal')==(kind=='metal')]
 subprocess.run(['swift','/tmp/next-gui-scroll-contact.swift','/tmp/next-gui-filter-menu-v1-'+kind+'-contact.png',*pngs],check=True)
