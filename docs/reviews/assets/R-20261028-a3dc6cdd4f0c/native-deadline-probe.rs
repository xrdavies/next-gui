use next_gui_platform::{AppCallbacks,Event,WindowAttrs,WindowCtx};
use std::time::{Duration,Instant};
struct Probe { phase:u8, since:Instant, total:Instant, count:u32, focus:u32 }
impl AppCallbacks for Probe {
 fn on_start(&mut self,ctx:&mut WindowCtx<'_>) {
  self.since=Instant::now();self.total=self.since;
  ctx.request_redraw_after(Duration::from_millis(120));
  ctx.request_redraw_after(Duration::from_millis(700));
 }
 fn on_event(&mut self,_:&mut WindowCtx<'_>,event:Event) {if matches!(event,Event::FocusChanged{..}) {self.focus+=1;}}
 fn on_redraw(&mut self,ctx:&mut WindowCtx<'_>) {
  self.count+=1;
  if std::env::var_os("NEXT_GUI_SOAK").is_none() {assert!(self.count<20,"deadline kept event loop busy");}
  let elapsed=self.since.elapsed();
  let delay=match self.phase {0=>120,1=>70,_=>90};
  if elapsed<Duration::from_millis(delay) {return;}
  assert!(elapsed<Duration::from_millis(600),"earlier request was postponed: {elapsed:?}");
  eprintln!("phase={} actual_delay_ms={} redraws={} focus_events={}",self.phase,elapsed.as_millis(),self.count,self.focus);
  if self.phase==2 {eprintln!("native deadline probe passed total_ms={}",self.total.elapsed().as_millis());ctx.request_exit();return;}
  self.phase+=1;self.since=Instant::now();
  ctx.request_redraw_after(Duration::from_millis(if self.phase==1 {70}else{90}));
  ctx.request_redraw_after(Duration::from_millis(700));
 }
}
fn main() {let now=Instant::now();next_gui_platform::run(WindowAttrs::new("next-gui delayed redraw probe",320,160),Probe{phase:0,since:now,total:now,count:0,focus:0}).unwrap();}
