//! Save the visible plot through the same raster renderer used on screen.
use crate::{db, Demo};
use next_gui_scene::RgbaImage;
use std::{io::{Seek, Write}, path::{Path, PathBuf}};

#[derive(Clone, Copy)]
enum Format { Png, Jpeg, Bmp, Pdf }

fn destination(path: &Path) -> (PathBuf, Format) {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => (path.into(), Format::Png),
        "jpg" | "jpeg" => (path.into(), Format::Jpeg),
        "bmp" => (path.into(), Format::Bmp),
        "pdf" => (path.into(), Format::Pdf),
        _ => { let mut name = path.as_os_str().to_os_string(); name.push(".png"); (PathBuf::from(name), Format::Png) }
    }
}

fn encode(mut output: &mut std::fs::File, image: &RgbaImage, format: Format) -> Result<(), String> {
    use image::{ColorType, ImageEncoder};
    let result = match format {
        Format::Png => image::codecs::png::PngEncoder::new(&mut output).write_image(image.pixels(), image.width(), image.height(), ColorType::Rgba8),
        Format::Jpeg => image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 95).encode(image.pixels(), image.width(), image.height(), ColorType::Rgba8),
        Format::Bmp => image::codecs::bmp::BmpEncoder::new(&mut output).encode(image.pixels(), image.width(), image.height(), ColorType::Rgba8),
        Format::Pdf => return pdf(output, image).map_err(|e| e.to_string()),
    };
    result.map_err(|e| e.to_string())
}

// shortcut: PDF embeds lossless raster pixels; add vector primitives for resolution-independent PDF/print parity.
fn pdf(output: &mut std::fs::File, image: &RgbaImage) -> std::io::Result<()> {
    let (width, height) = (image.width(), image.height());
    let mut offsets = [0u64; 6];
    output.write_all(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n")?;
    offsets[1] = output.stream_position()?;
    output.write_all(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;
    offsets[2] = output.stream_position()?;
    output.write_all(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n")?;
    offsets[3] = output.stream_position()?;
    writeln!(output, "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << /XObject << /Plot 4 0 R >> >> /Contents 5 0 R >>\nendobj")?;
    offsets[4] = output.stream_position()?;
    writeln!(output, "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream", u64::from(width)*u64::from(height)*3)?;
    let mut row = vec![0u8; width as usize * 3];
    for pixels in image.pixels().chunks_exact(width as usize * 4) {
        for (source, target) in pixels.chunks_exact(4).zip(row.chunks_exact_mut(3)) {
            for channel in 0..3 { target[channel] = ((u16::from(source[channel])*u16::from(source[3]) + 255*(255-u16::from(source[3])) + 127)/255) as u8; }
        }
        output.write_all(&row)?;
    }
    output.write_all(b"\nendstream\nendobj\n")?;
    offsets[5] = output.stream_position()?;
    let content = format!("q {width} 0 0 {height} 0 0 cm /Plot Do Q\n");
    writeln!(output, "5 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj", content.len())?;
    let xref = output.stream_position()?;
    output.write_all(b"xref\n0 6\n0000000000 65535 f \n")?;
    for offset in &offsets[1..] { writeln!(output, "{offset:010} 00000 n ")?; }
    writeln!(output, "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF")
}

fn save(conn: &rusqlite::Connection, path: &Path, image: &RgbaImage) -> Result<PathBuf, String> {
    save_with(conn, path, |output, format| encode(output, image, format))
}

fn save_with(conn: &rusqlite::Connection, path: &Path, write: impl FnOnce(&mut std::fs::File, Format) -> Result<(), String>) -> Result<PathBuf, String> {
    let (path, format) = destination(path);
    db::guard_output_path(conn, &path)?;
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    write(temp.as_file_mut(), format)?;
    temp.as_file_mut().sync_all().map_err(|e| e.to_string())?;
    db::guard_output_path(conn, &path)?;
    temp.persist(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

impl Demo {
    pub(crate) fn visible_plot_image(&self) -> Result<RgbaImage,String> {
        let (width,height)=self.ui.chart_plot_size(self.ids.chart,0).ok_or("No visible plot to copy or save")?;
        self.ui.chart_plot_image(self.ids.chart,0,width,height).ok_or_else(||"Plot image exceeds the 128 MiB image budget".into())
    }

    pub(crate) fn copy_plot(&mut self,ctx:&mut next_gui_platform::WindowCtx<'_>) {
        let result=self.visible_plot_image().and_then(|image|ctx.clipboard_set_image(&image).map_err(|error|error.to_string()));
        match result {
            Ok(())=>{self.clear_error();self.state.status_msg="Plot image copied".into();}
            Err(error)=>self.set_error(error),
        }
        self.rebuild=true;
    }

    pub(crate) fn save_plot(&mut self) {
        let result = (|| {
            self.state.guard_background_idle()?;
            let conn = self.state.conn.as_ref().ok_or("No idle database connection")?;
            let Some(path) = crate::file_dlg::pick_save_plot() else { return Ok(None); };
            let image = self.visible_plot_image()?;
            save(conn, &path, &image).map(Some)
        })();
        match result {
            Ok(Some(path)) => { self.clear_error(); self.state.status_msg = format!("Plot saved to {}", path.display()); }
            Ok(None) => return,
            Err(error) => self.set_error(error),
        }
        self.rebuild = true;
    }
}

#[cfg(test)]
pub(crate) fn capture(demo:&mut Demo,folder:&Path,name:&str) {
    use next_gui_render::{encode_scene,Renderer};
    demo.scene.mark_full_dirty(demo.win_w as f32,demo.win_h as f32); let dirty=demo.scene.take_dirty();
    let commands=encode_scene(&demo.scene,&dirty); let clear=crate::ui_build::clear_color(&demo.theme);
    let mut soft=next_gui_render_soft::SoftRenderer::new(demo.win_w,demo.win_h,clear);soft.render(&commands,&dirty);
    let pixels=soft.frame_pixels().unwrap().0;
    crate::write_ppm(folder.join(format!("window-{name}.ppm")).to_str().unwrap(),pixels,demo.win_w,demo.win_h).unwrap();
    if let Ok(mut gpu)=next_gui_render_gpu::GpuRenderer::try_new(demo.win_w,demo.win_h,clear) {
        gpu.render(&commands,&dirty);let rendered=gpu.frame_pixels().unwrap().0;
        crate::write_ppm(folder.join(format!("window-{name}-metal.ppm")).to_str().unwrap(),rendered,demo.win_w,demo.win_h).unwrap();
        let bounds=demo.scene.iter_paint_order().into_iter().find(|n|demo.ui.widget_from_node(n.id)==Some(demo.ids.chart)).unwrap().bounds;
        let mut delta=0;let mut count=0;
        for y in (bounds.y.ceil().max(0.0) as usize+2)..((bounds.y+bounds.height).floor().min(demo.win_h as f32) as usize).saturating_sub(2) {
            for x in (bounds.x.ceil().max(0.0) as usize+2)..((bounds.x+bounds.width).floor().min(demo.win_w as f32) as usize).saturating_sub(2) {
                for shift in [0,8,16] {delta=delta.max((((pixels[y*demo.win_w as usize+x]>>shift)&255) as i32-((rendered[y*demo.win_w as usize+x]>>shift)&255) as i32).abs());}
                count+=1;
            }
        }
        assert!(count>1000 && delta<=2,"{name}: delta={delta},pixels={count}");
        eprintln!("Plot export {name}: delta={delta},pixels={count}");
        if demo.ui.popup_owner()==Some(demo.ids.context_menu) {
            let style=demo.ui.context_menu_style();
            let panel=demo.scene.iter_paint_order().into_iter().rev().find(|n|n.visible && n.fill==style.popup_bg && n.bounds.width>=style.popup_width && n.bounds.height>=style.item_height).unwrap().bounds;
            assert!(panel.x>=0.0 && panel.y>=0.0 && panel.x+panel.width<=demo.win_w as f32 && panel.y+panel.height<=demo.win_h as f32);
            let mut delta=0;
            for y in panel.y.ceil() as usize+2..(panel.y+panel.height).floor() as usize-2 {for x in panel.x.ceil() as usize+2..(panel.x+panel.width).floor() as usize-2 {
                for shift in [0,8,16] {delta=delta.max((((pixels[y*demo.win_w as usize+x]>>shift)&255) as i32-((rendered[y*demo.win_w as usize+x]>>shift)&255) as i32).abs());}
            }}
            assert!(delta<=2,"Context menu {name}: {delta}");eprintln!("Context menu {name}: delta={delta}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{plot, state::{DockKind, MainTab}, PresentBackend};

    #[test]
    fn formats_roundtrip_case_suffixes_and_pdf_cross_references() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let pixels = (0..32*20).flat_map(|index| if index%32<16 { [230,20,40,255] } else { [10,40,220,255] }).collect::<Vec<_>>();
        let image = RgbaImage::new(32,20,pixels.clone()).unwrap();
        for extension in ["png","PNG","jpg","JPEG","bmp","BMP","pdf","PDF","other",""] {
            let name = if extension.is_empty() { "plot".into() } else { format!("plot.{extension}") };
            let path = save(&conn, &dir.path().join(name), &image).unwrap();
            assert!(path.is_file());
            if matches!(extension,"other"|"") { assert_eq!(path.extension().unwrap(), "png"); }
            let bytes = std::fs::read(&path).unwrap();
            if extension.eq_ignore_ascii_case("pdf") {
                assert!(bytes.starts_with(b"%PDF-1.4"));
                let text = String::from_utf8_lossy(&bytes);
                let xref = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse::<usize>().unwrap();
                assert!(bytes[xref..].starts_with(b"xref\n0 6\n"));
                let xref_text = std::str::from_utf8(&bytes[xref..]).unwrap();
                for (object, line) in xref_text.lines().skip(3).take(5).enumerate() {
                    let offset:usize = line[..10].parse().unwrap();
                    assert!(bytes[offset..].starts_with(format!("{} 0 obj",object+1).as_bytes()));
                }
                assert!(text.contains("/MediaBox [0 0 32 20]"));
                let header = format!("/BitsPerComponent 8 /Length {} >>\nstream\n",32*20*3);
                let begin = bytes.windows(header.len()).position(|part|part==header.as_bytes()).unwrap()+header.len();
                let rgb = pixels.chunks_exact(4).flat_map(|p|p[..3].iter().copied()).collect::<Vec<_>>();
                assert_eq!(&bytes[begin..begin+rgb.len()],rgb);
            } else {
                let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
                assert_eq!(decoded.dimensions(),(32,20));
                if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
                    assert!(decoded.get_pixel(4,4)[0]>200 && decoded.get_pixel(24,4)[2]>190);
                } else { assert_eq!(decoded.as_raw(), &pixels); }
            }
        }
        let alpha = RgbaImage::new(1,1,vec![255,0,0,128]).unwrap();
        let alpha_path = save(&conn,&dir.path().join("alpha.pdf"),&alpha).unwrap();
        assert!(std::fs::read(alpha_path).unwrap().windows(3).any(|bytes|bytes==[255,127,127]));
    }

    #[test]
    fn staged_save_preserves_destinations_databases_and_pending_edits() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.png"); let aux = dir.path().join("aux.pdf");
        let conn = rusqlite::Connection::open(&main).unwrap();
        conn.execute("ATTACH ? AS aux",[aux.to_str().unwrap()]).unwrap();
        conn.execute_batch("CREATE TABLE kept(x); CREATE TABLE aux.kept(x); BEGIN; INSERT INTO kept VALUES(19)").unwrap();
        let before = std::fs::read(&main).unwrap(); let aux_before = std::fs::read(&aux).unwrap();
        let image = RgbaImage::new(2,2,vec![255;16]).unwrap();
        for path in [&main,&aux] { assert!(save(&conn,path,&image).unwrap_err().contains("open database")); }
        assert_eq!(std::fs::read(&main).unwrap(),before); assert_eq!(std::fs::read(&aux).unwrap(),aux_before);
        let path = dir.path().join("existing.png"); std::fs::write(&path,b"previous output").unwrap();
        let count = std::fs::read_dir(dir.path()).unwrap().count();
        let error = save_with(&conn,&path,|output,_| { output.write_all(b"incomplete encoding").unwrap(); Err("encoding failed".into()) }).unwrap_err();
        assert_eq!(error,"encoding failed"); assert_eq!(std::fs::read(&path).unwrap(),b"previous output");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(),count);
        let folder = dir.path().join("folder.png"); std::fs::create_dir(&folder).unwrap();
        assert!(save(&conn,&folder,&image).is_err()); assert!(folder.is_dir());
        save(&conn,&path,&image).unwrap(); assert!(image::open(path).is_ok());
        assert!(!conn.is_autocommit());
        assert_eq!(conn.query_row("SELECT x FROM kept",[],|row|row.get::<_,i64>(0)).unwrap(),19);
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM kept",[],|row|row.get::<_,i64>(0)).unwrap(),0);
    }

    #[test]
    fn plot_hover_keeps_source_and_selection_with_bounded_dual_axis_tooltips() {
        use next_gui_ui::UiKey;
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(crate::CURRENT_PLOT_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
        demo.state.tab = MainTab::Browse; demo.state.dock_focus = DockKind::Plot;
        demo.state.sync_current_plot().unwrap();
        for (row, axis) in [(1, 1), (2, 2), (3, 3)] { assert!(demo.state.plot.check(row, axis, true)); }
        demo.state.sync_current_plot().unwrap();
        let source = demo.state.plot.data.series[0].points.clone();
        let folder = std::env::var("NEXT_GUI_PLOT_HOVER_QA_OUT").ok().map(PathBuf::from);
        if let Some(folder) = &folder { std::fs::create_dir_all(folder).unwrap(); }
        for (name, theme) in [("dark", next_gui_ui::Theme::dark()), ("light", next_gui_ui::Theme::light())] {
            demo.theme = crate::ui_build::browser_theme(theme, demo.state.prefs.font_size);
            for (width, height) in [(1400, 1000), (800, 600)] {
                demo.win_w = width; demo.win_h = height; demo.do_rebuild();
                let image = demo.scene.iter_paint_order().into_iter().find(|n| n.visible && n.image.is_some()).unwrap().bounds;
                let (x, y) = (image.x + image.width * 0.63, image.y + image.height * 0.5);
                assert_eq!(demo.ui.hit_widget(&demo.scene, x, y), Some(demo.ids.chart));
                demo.ui.pointer_hover(&demo.scene, x, y); demo.ui.pointer_moved(x, y); demo.ui.sync(&mut demo.scene);
                let style = demo.ui.tooltip_style();
                let panel = demo.scene.iter_paint_order().into_iter().rev().find(|n| n.visible && n.fill == style.bg && n.mask.is_none()).unwrap();
                let bounds = panel.bounds; let panel_id = panel.id;
                assert!(bounds.width > 50.0 && bounds.height > style.size * 3.0);
                assert!(bounds.x >= 0.0 && bounds.y >= 0.0 && bounds.x + bounds.width <= width as f32 && bounds.y + bounds.height <= height as f32);
                let labels = demo.scene.iter_paint_order().into_iter().rev().find(|n| n.visible && n.mask.is_some() && n.fill == style.fg).unwrap().mask.as_ref().unwrap().clone();
                if let Some(folder) = &folder { capture(&mut demo, folder, &format!("hover-{name}-{width}")); }
                demo.ui.set_focus(Some(demo.ids.chart)); assert!(demo.ui.key_input(UiKey::Right)); demo.ui.sync(&mut demo.scene);
                let changed = demo.scene.iter_paint_order().into_iter().rev().find(|n| n.visible && n.mask.is_some() && n.fill == style.fg).unwrap().mask.as_ref().unwrap().clone();
                assert_ne!(labels.pixels, changed.pixels, "Navigation must refresh the stationary pointer's coordinates");
                assert!(std::sync::Arc::ptr_eq(&source, &demo.ui.chart_plot(demo.ids.chart, 0).unwrap().series[0].points));
                assert!(demo.ui.chart_plot(demo.ids.chart, 0).unwrap().state.selected_rows.is_empty());
                assert!(demo.ui.pointer_left()); demo.ui.sync(&mut demo.scene);
                assert!(!demo.scene.get(panel_id).unwrap().visible);
                demo.ui.pointer_moved(x, y); demo.ui.sync(&mut demo.scene);
                assert!(demo.handle_plot_context_key(crate::KeyCode::Function(10), crate::Modifiers { shift: true, ..crate::Modifiers::default() }));
                demo.ui.sync(&mut demo.scene);
                assert!(!demo.scene.get(panel_id).unwrap().visible, "Context actions hide the hover overlay");
                demo.ui.key_input(UiKey::Escape); demo.ui.pointer_left(); demo.ui.sync(&mut demo.scene);
            }
        }
        assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM quarterly_revenue", [], |row| row.get::<_, i64>(0)).unwrap(), 18);
    }

    #[test]
    fn grouped_and_stacked_plot_menu_keeps_view_source_and_project_settings() {
        use next_gui_ui::UiKey;
        let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(crate::BAR_PLOT_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","quarterly_revenue").unwrap();
        demo.state.tab=MainTab::Browse;demo.state.sync_current_plot().unwrap();
        for (row,axis) in [(1,1),(2,2),(3,2)] {assert!(demo.state.plot.check(row,axis,true));}
        demo.state.sync_current_plot().unwrap();
        assert!(demo.state.plot.data.series.iter().all(|s|s.line==next_gui_ui::PlotLine::Bars));
        demo.state.dock_focus=DockKind::Plot;demo.state.dock_plot=true;demo.state.body_ratio=0.52;
        let points=demo.state.plot.data.series[0].points.clone();
        let folder=std::env::var("NEXT_GUI_PLOT_BARS_QA_OUT").ok().map(std::path::PathBuf::from);
        if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
        for (name,theme) in [("dark",next_gui_ui::Theme::dark()),("light",next_gui_ui::Theme::light())] {
            demo.theme=crate::ui_build::browser_theme(theme,demo.state.prefs.font_size);
            for (width,height) in [(1400,1000),(800,600)] {
                demo.win_w=width;demo.win_h=height;demo.do_rebuild();
                let grouped=demo.visible_plot_image().unwrap();
                if let Some(folder)=&folder {capture(&mut demo,folder,&format!("grouped-{name}-{width}"));}
                demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);
                assert!(demo.handle_plot_context_key(crate::KeyCode::Function(10),crate::Modifiers{shift:true,..crate::Modifiers::default()}));
                demo.ui.key_input(UiKey::Home);for _ in 0..5 {demo.ui.key_input(UiKey::Down);}demo.ui.key_input(UiKey::Enter);
                let (_,item)=demo.ui.take_menu_action().unwrap();assert_eq!(item,crate::mid::PLOT_STACKED_BARS);
                assert!(demo.handle_plot_menu_action(item));demo.do_rebuild();
                assert!(demo.state.plot.settings.stacked_bars && demo.state.plot.data.stacked_bars);
                assert_ne!(grouped.pixels(),demo.visible_plot_image().unwrap().pixels());
                assert!(demo.state.plot.data.auto_view().y1.1>29.0 && demo.state.plot.data.auto_view().y1.0< -11.0);
                if let Some(folder)=&folder {capture(&mut demo,folder,&format!("stacked-{name}-{width}"));}
                let data=demo.ui.chart_plot_mut(demo.ids.chart,0).unwrap();assert!(data.zoom(0.8,0.5,0.5));data.select_row(1,false);
                let view=data.view();demo.snapshot_inputs();
                assert!(demo.handle_plot_menu_action(item));demo.do_rebuild();assert!(!demo.state.plot.data.stacked_bars);
                assert_eq!(demo.ui.chart_plot(demo.ids.chart,0).unwrap().view(),view);
                assert!(demo.ui.chart_plot(demo.ids.chart,0).unwrap().state.selected_rows.contains(&1));
                assert!(std::sync::Arc::ptr_eq(&points,&demo.state.plot.data.series[0].points));
                demo.fit_plot();demo.do_rebuild();
            }
        }
        demo.handle_plot_menu_action(crate::mid::PLOT_STACKED_BARS);demo.do_rebuild();
        let project=crate::project::ProjectFile::from_state(&demo.state);
        let json:crate::project::ProjectFile=serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();assert!(json.plot_settings.stacked_bars);
        let xml=crate::project_xml::encode(&project,Path::new("/tmp/next-gui-bars.sqbpro")).unwrap();
        assert!(crate::project_xml::decode(&xml).unwrap().plot_settings.stacked_bars);
        assert!(!serde_json::from_str::<plot::Settings>("{}").unwrap().stacked_bars);
    }

    #[test]
    fn plot_context_menu_preserves_full_data_view_and_project_format_settings() {
        use next_gui_ui::UiKey;
        let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(crate::FULL_PLOT_QA_SQL).unwrap();
        demo.state.tab=MainTab::ExecuteSql;
        *demo.state.sql_editor_mut()="SELECT sequence,measured/1000.0 AS measured,reference/1000.0 AS reference FROM telemetry ORDER BY sequence".into();
        demo.state.run_sql_sync().unwrap();demo.state.sync_current_plot().unwrap();
        for (row,axis) in [(1,1),(2,2),(3,3)] {demo.state.plot.check(row,axis,true);}
        demo.state.start_plot_load().unwrap();
        for _ in 0..10000 {
            if demo.state.plot_work.is_none() {break;}
            demo.state.poll_plot_load().unwrap();std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(demo.state.plot_work.is_none() && demo.state.plot.has_full());
        demo.state.dock_focus=DockKind::Plot;demo.state.dock_plot=true;demo.state.body_ratio=0.52;demo.do_rebuild();
        let data=demo.ui.chart_plot_mut(demo.ids.chart,0).unwrap();
        assert!(data.zoom(0.75,0.5,0.5));assert!(data.select_row(101,false));
        let view=data.view();demo.snapshot_inputs();
        let points=demo.state.plot.data.series[0].points.clone();
        let snapshot=demo.state.sql_result.as_ref().unwrap().all_values.clone().unwrap();
        let folder=std::env::var("NEXT_GUI_PLOT_MENU_QA_OUT").ok().map(std::path::PathBuf::from);
        if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
        for (name,theme) in [("dark",next_gui_ui::Theme::dark()),("light",next_gui_ui::Theme::light())] {
            demo.theme=crate::ui_build::browser_theme(theme,demo.state.prefs.font_size);
            for (width,height) in [(1400,1000),(800,600)] {
                demo.win_w=width;demo.win_h=height;demo.do_rebuild();
                demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);
                assert!(!demo.handle_plot_context_key(crate::KeyCode::Function(10),crate::Modifiers::default()));
                assert!(demo.handle_plot_context_key(crate::KeyCode::Function(10),crate::Modifiers{shift:true,..crate::Modifiers::default()}));
                assert_eq!(demo.ui.popup_owner(),Some(demo.ids.context_menu));
                assert_eq!(demo.ui.accessible_name(demo.ids.context_menu).as_deref(),Some("Plot actions"));
                assert!(demo.ui.key_input(UiKey::Home));assert!(demo.ui.key_input(UiKey::Enter));
                assert_eq!(demo.ui.take_menu_action(),Some((demo.ids.context_menu,crate::mid::PLOT_COPY)));
                assert!(demo.ui.popup_owner().is_none());
                let normal=demo.visible_plot_image().unwrap();
                assert!(demo.handle_plot_context_key(crate::KeyCode::Function(10),crate::Modifiers{shift:true,..crate::Modifiers::default()}));
                assert!(demo.ui.key_input(UiKey::Home));for _ in 0..4 {assert!(demo.ui.key_input(UiKey::Down));}
                assert!(demo.ui.key_input(UiKey::Enter));
                let (owner,item)=demo.ui.take_menu_action().unwrap();assert_eq!(owner,demo.ids.context_menu);assert_eq!(item,crate::mid::PLOT_FIXED_FORMAT);
                assert!(demo.handle_plot_menu_action(item));demo.do_rebuild();
                assert!(demo.state.plot.settings.fixed_number_format && demo.ui.chart_plot(demo.ids.chart,0).unwrap().fixed_number_format);
                assert_eq!(demo.ui.chart_plot(demo.ids.chart,0).unwrap().view(),view);
                assert_ne!(demo.visible_plot_image().unwrap().pixels(),normal.pixels());
                if let Some(folder)=&folder {
                    capture(&mut demo,folder,&format!("fixed-{name}-{width}"));
                    assert!(demo.handle_plot_context_key(crate::KeyCode::Function(10),crate::Modifiers{shift:true,..crate::Modifiers::default()}));
                    demo.ui.key_input(UiKey::Home);for _ in 0..4 {demo.ui.key_input(UiKey::Down);}
                    demo.ui.sync(&mut demo.scene);capture(&mut demo,folder,&format!("menu-{name}-{width}"));
                    assert!(demo.ui.key_input(UiKey::Escape));assert_eq!(demo.ui.focused(),Some(demo.ids.chart));
                }
                assert!(demo.handle_plot_menu_action(crate::mid::PLOT_FIXED_FORMAT));demo.do_rebuild();
                assert!(!demo.state.plot.data.fixed_number_format);
                assert!(std::sync::Arc::ptr_eq(&points,&demo.state.plot.data.series[0].points));
                assert!(std::sync::Arc::ptr_eq(&snapshot,demo.state.sql_result.as_ref().unwrap().all_values.as_ref().unwrap()));
            }
        }
        demo.handle_plot_menu_action(crate::mid::PLOT_FIXED_FORMAT);demo.handle_plot_menu_action(crate::mid::PLOT_LEGEND);demo.do_rebuild();
        assert!(demo.state.plot.data.fixed_number_format && demo.state.plot.data.show_legend);
        let project=crate::project::ProjectFile::from_state(&demo.state);
        let json:crate::project::ProjectFile=serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();assert!(json.plot_settings.fixed_number_format);
        let xml=crate::project_xml::encode(&project,Path::new("/tmp/next-gui-plot-menu.sqbpro")).unwrap();
        let xml=crate::project_xml::decode(&xml).unwrap();assert!(xml.plot_settings.fixed_number_format && xml.plot_settings.show_legend);
        let old:plot::Settings=serde_json::from_str("{}").unwrap();assert!(!old.fixed_number_format);
        assert!(demo.handle_plot_menu_action(crate::mid::PLOT_FIT));demo.do_rebuild();assert!(demo.state.plot.data.state.view.is_none());
        demo.state.overlay=Some(crate::state::OverlayKind::Preferences);demo.do_rebuild();
        assert!(!demo.open_plot_context_menu(600.0,400.0));
        demo.state.conn=None;demo.save_plot();assert_eq!(demo.state.error_msg.as_deref(),Some("No idle database connection"));
    }

    #[test]
    fn plot_controls_and_chart_stay_reachable_after_focus_and_resize() {
        let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE samples(x,y);INSERT INTO samples VALUES(1,2),(2,3);").unwrap();
        demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","samples").unwrap();
        demo.state.tab=MainTab::Browse;demo.state.dock_focus=DockKind::Plot;demo.state.dock_plot=true;
        demo.state.body_ratio=0.9;
        demo.state.sync_current_plot().unwrap();
        for (row,axis) in [(1,1),(2,2)] {demo.state.plot.check(row,axis,true);}
        for theme in [next_gui_ui::Theme::dark(),next_gui_ui::Theme::light()] {
            demo.theme=crate::ui_build::browser_theme(theme,demo.state.prefs.font_size);
            for (width,height) in [(800,600),(1000,760),(1400,1000)] {
                demo.win_w=width;demo.win_h=height;demo.do_rebuild();
                for id in [demo.ids.plot.mode,demo.ids.plot.legend,demo.ids.plot.load_all,demo.ids.plot.line,demo.ids.plot.marker,demo.ids.plot.color,demo.ids.plot.fit,demo.ids.plot.save,demo.ids.plot.copy,demo.ids.chart] {
                    assert_ne!(id.0,0);
                    demo.ui.set_focus(Some(id));demo.ui.sync(&mut demo.scene);
                    let bounds=demo.scene.iter_paint_order().into_iter().find(|n|demo.ui.widget_from_node(n.id)==Some(id)).unwrap().bounds;
                    assert!(bounds.x>=0.0 && bounds.x+bounds.width<=width as f32,"{width}: clipped {id:?} at {bounds:?}");
                    assert_eq!(demo.ui.scroll_offset(demo.ids.dock_scrolls[DockKind::Plot as usize]).unwrap().0,0.0,"Focusing plot controls must not scroll away the chart");
                    if id==demo.ids.chart {assert!(demo.ui.chart_plot_size(id,0).unwrap().0>=260);}
                    demo.snapshot_inputs();assert_eq!(demo.state.body_ratio,0.9,"Adaptive sizing must preserve the saved ratio");
                }
            }
        }
    }

    #[test]
    fn plot_export_uses_current_view_selection_palette_and_no_query_replay() {
        let mut demo = Demo::new(1400,1000,PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(crate::FULL_PLOT_QA_SQL).unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        *demo.state.sql_editor_mut() = "UPDATE telemetry SET measured=measured+1 RETURNING sequence,measured,reference".into();
        demo.state.run_sql_sync().unwrap();
        demo.state.plot.settings.manual=false; demo.state.sync_current_plot().unwrap();
        for (row,axis) in [(1,1),(2,2),(3,3)] { demo.state.plot.check(row,axis,true); }
        demo.state.plot.set_styles(1,4);
        demo.state.plot.settings.show_legend=true;
        demo.state.dock_focus=DockKind::Plot; demo.state.dock_plot=true;
        demo.do_rebuild();
        assert_eq!(demo.ui.accessible_name(demo.ids.plot.save).as_deref(),Some("Save plot as PNG, JPEG, BMP or PDF"));
        assert_eq!(demo.ui.accessible_name(demo.ids.plot.copy).as_deref(),Some("Copy plot image to the clipboard"));
        demo.ui.set_focus(Some(demo.ids.plot.copy));
        assert!(demo.ui.key_input(next_gui_ui::UiKey::Space));
        assert_eq!(demo.ui.take_button_activate(),Some(demo.ids.plot.copy));
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(),Some(demo.ids.plot.copy));
        let data = demo.ui.chart_plot_mut(demo.ids.chart,0).unwrap();
        assert!(data.zoom(0.7,0.5,0.5)); assert!(data.select_row(101,false));
        let view = data.view();
        demo.snapshot_inputs(); demo.poll_selection_changes(); demo.do_rebuild();
        assert_eq!(demo.ui.chart_plot(demo.ids.chart,0).unwrap().view(),view);
        let directory = std::env::var("NEXT_GUI_PLOT_EXPORT_QA_OUT").ok();
        let temporary = tempfile::tempdir().unwrap();
        let folder = directory.as_deref().map(Path::new).unwrap_or(temporary.path());
        std::fs::create_dir_all(folder).unwrap();
        let result = demo.state.sql_result.as_ref().unwrap().all_values.clone().unwrap();
        for (name,theme) in [("dark",next_gui_ui::Theme::dark()),("light",next_gui_ui::Theme::light())] {
            demo.theme=crate::ui_build::browser_theme(theme,demo.state.prefs.font_size);
            for (width,height) in [(1400,1000),(800,600)] {
                demo.win_w=width; demo.win_h=height; demo.do_rebuild();
                assert!(demo.ui.button_enabled(demo.ids.plot.save).unwrap());
                let size = demo.ui.chart_plot_size(demo.ids.chart,0).unwrap();
                let image = demo.ui.chart_plot_image(demo.ids.chart,0,size.0,size.1).unwrap();
                assert_eq!(demo.visible_plot_image().unwrap().pixels(),image.pixels());
                assert!(image.pixels().chunks_exact(4).any(|p|p[..3]!=image.pixels()[..3]));
                assert_eq!(demo.ui.chart_plot(demo.ids.chart,0).unwrap().view(),view);
                for extension in ["png","jpg","bmp","pdf"] {
                    save(demo.state.conn.as_ref().unwrap(),&folder.join(format!("plot-{name}-{width}.{extension}")),&image).unwrap();
                }
                let decoded=image::open(folder.join(format!("plot-{name}-{width}.png"))).unwrap().to_rgba8();
                assert_eq!(decoded.as_raw().as_slice(),image.pixels().as_ref());
                if directory.is_some() { capture(&mut demo,folder,&format!("{name}-{width}")); }
            }
        }
        assert!(std::sync::Arc::ptr_eq(&result,demo.state.sql_result.as_ref().unwrap().all_values.as_ref().unwrap()));
        let mut statement=demo.state.conn.as_ref().unwrap().prepare("SELECT measured FROM telemetry WHERE sequence=1").unwrap();
        assert_eq!(statement.query_row([],|row|row.get::<_,f64>(0)).unwrap(),1344.0);
        assert!(matches!(demo.state.plot.source,Some(plot::Source::Query(0))));
    }


}
