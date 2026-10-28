use std::{sync::Arc,time::Instant,io::Write};
use next_gui_ui::{Ui,WidgetId,ColumnStyle,ChartStyle,ChartTile,PlotStyle,PlotData,PlotSeries,PlotPoint,PlotLine,PlotMarker,PlotAxisFormat};
use next_gui_scene::Color;
fn output(path:&str,image:next_gui_scene::RgbaImage){let mut f=std::fs::File::create(path).unwrap();write!(f,"P6\n{} {}\n255\n",image.width(),image.height()).unwrap();for p in image.pixels().chunks_exact(4){f.write_all(&p[..3]).unwrap();}}
fn main(){
 let mut ui=Ui::new();let mut id=WidgetId(0);
 let markers=[PlotMarker::None,PlotMarker::Cross,PlotMarker::Plus,PlotMarker::Circle,PlotMarker::Disc,PlotMarker::Square,PlotMarker::Diamond,PlotMarker::Star,PlotMarker::Triangle,PlotMarker::TriangleInverted,PlotMarker::CrossSquare,PlotMarker::PlusSquare,PlotMarker::CrossCircle,PlotMarker::PlusCircle,PlotMarker::Peace];
 let lines=[PlotLine::None,PlotLine::Line,PlotLine::StepLeft,PlotLine::StepRight,PlotLine::StepCenter,PlotLine::Impulse];
 let mut data=PlotData{x_label:"Observation date".into(),y1_label:"Left · signed values".into(),y2_label:"Right · thousands".into(),x_format:PlotAxisFormat::Date,..PlotData::default()};
 let epoch=data.x_format.parse("2026-10-01").unwrap();
 for (i,marker) in markers.into_iter().enumerate(){let right=i>=8;let offset=if right{i-8}else{i};data.series.push(PlotSeries{name:format!("{marker:?} / {:?}",lines[i%6]),color:Color::rgb((40+i*37%180)as u8,(50+i*59%150)as u8,(80+i*43%150)as u8),right_axis:right,points:(0..14).map(|j|PlotPoint{x:epoch+j as f64*86400.0,y:((j as f64*0.8+i as f64).sin()+offset as f64*3.0)*if right{1000.0}else{1.0},row:j}).collect::<Vec<_>>().into(),line:lines[i%6],marker});}
 data.state.selected_rows=Arc::new([3,4].into_iter().collect());
 ui.column(ColumnStyle::default(),|c|id=c.chart(vec![ChartTile::plot("Styles",data,PlotStyle::default())],ChartStyle::default()));
 output("/tmp/next-gui-plot-styles.ppm",ui.chart_plot_image(id,0,1200,760).unwrap());
 let category=PlotData{x_label:"Region".into(),y1_label:"Balance".into(),x_format:PlotAxisFormat::Labels(vec!["East".into(),"West".into(),"East".into(),"North".into()].into()),series:vec![PlotSeries{name:"Balance".into(),color:Color::rgb(20,110,220),right_axis:false,points:vec![PlotPoint{x:0.0,y:12.0,row:0},PlotPoint{x:1.0,y:-8.0,row:1},PlotPoint{x:2.0,y:4.0,row:2},PlotPoint{x:3.0,y:17.0,row:3}].into(),line:PlotLine::Bars,marker:PlotMarker::None}],..PlotData::default()};
 ui.set_chart_tiles(id,vec![ChartTile::plot("Categories",category,PlotStyle::default())]);
 output("/tmp/next-gui-plot-categories.ppm",ui.chart_plot_image(id,0,800,480).unwrap());
 let plot=ui.chart_plot_mut(id,0).unwrap();plot.series.truncate(1);plot.x_format=PlotAxisFormat::Numeric;plot.x_label="sample".into();plot.series[0].marker=PlotMarker::None;plot.series[0].line=PlotLine::Line;plot.state=Default::default();
 for count in [100_000,1_000_000]{ui.chart_plot_mut(id,0).unwrap().series[0].points=(0..count).map(|row|PlotPoint{x:row as f64,y:(row as f64*0.001).sin(),row}).collect::<Vec<_>>().into();let start=Instant::now();let image=ui.chart_plot_image(id,0,1000,600).unwrap();assert_eq!(image.pixels().len(),2_400_000);println!("{count} samples render {:?}",start.elapsed());}
 let mut scene=next_gui_scene::Scene::new();ui.sync(&mut scene);
 let before=scene.iter_paint_order().into_iter().find_map(|n|n.image.clone()).unwrap();let start=Instant::now();ui.sync(&mut scene);let elapsed=start.elapsed();
 let after=scene.iter_paint_order().into_iter().find_map(|n|n.image.clone()).unwrap();assert!(Arc::ptr_eq(before.pixels(),after.pixels()));println!("cached million-point sync {:?}",elapsed);
}
