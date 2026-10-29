import os,pathlib,subprocess
prefix='/tmp/next-gui-filter-menu-v1-'
pathlib.Path(prefix+'render').mkdir(exist_ok=True)
env=os.environ.copy();env['NEXT_GUI_FILTER_QA_OUT']=prefix+'render'
with open(prefix+'render.log','w') as log:subprocess.run(['cargo','--offline','test','-p','e28_db_browser','filter_menu::tests','--','--nocapture'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
with open(prefix+'convert.log','w') as log:subprocess.run(['python3',prefix+'convert.py'],stdout=log,stderr=subprocess.STDOUT,check=True)
