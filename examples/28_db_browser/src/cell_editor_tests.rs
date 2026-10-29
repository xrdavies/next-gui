use super::*;

#[test]
fn cell_invalid_format_modal_keeps_drafts_history_focus_and_explicit_apply() {
    use crate::state::EditCellMode;
    use rusqlite::types::Value;
    let mut demo=Demo::new(1400,860,PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    demo.state.prefs.auto_switch_mode=false;demo.state.prefs.indent_compact=true;
    demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","quarterly_revenue").unwrap();
    demo.state.tab=MainTab::Browse;demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus=DockKind::Plot;demo.state.dock_ratio=0.62;demo.state.body_ratio=0.45;
    demo.state.sync_current_plot().unwrap();demo.state.plot.check(1,1,true);demo.state.plot.check(2,2,true);
    let folder=std::env::var("NEXT_GUI_CELL_FORMAT_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
    let stored=|demo:&Demo|demo.state.conn.as_ref().unwrap().query_row("SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",[],|r|r.get::<_,Value>(0)).unwrap();
    for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        for (width,height,size) in [(1400,860,13.0),(800,600,18.0)] {
            demo.win_w=width;demo.win_h=height;demo.state.prefs.font_size=size;demo.theme=ui_build::browser_theme(theme,size);
            for (kind,mode,bad) in [("json",EditCellMode::Json,"{\n    \"name\": \"中文\",\n    \"steps\": [1,]\n}"),
                ("xml",EditCellMode::Xml,"<?xml version='1.0'?>\n<report><name>中文</name>\n<item></report>")] {
                demo.state.discard_cell_draft();
                demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",[if mode==EditCellMode::Json {"{\"saved\":true}"} else {"<r><saved/></r>"}]).unwrap();
                demo.state.refresh_browse().unwrap();demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();
                demo.state.set_edit_mode(mode).unwrap();demo.do_rebuild();let original=stored(&demo);
                demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text(bad);demo.snapshot_inputs();
                assert!(demo.state.edit_format_issue.as_ref().unwrap().position.is_some());assert!(demo.ui.accessible_name(demo.ids.edit_value).unwrap().contains("Invalid"));
                demo.ui.select_text_area_range(demo.ids.edit_value,2..5);demo.ui.sync(&mut demo.scene);let selection=demo.ui.text_area_selection(demo.ids.edit_value);
                let revision=demo.state.edit_revision;
                if let Some(folder)=&folder {crate::plot_export::capture(&mut demo,folder,&format!("cell-format-mark-{kind}-{name}-{width}"));}
                assert!(demo.handle_cell_apply_key(KeyCode::Enter,Modifiers {meta:true,..Modifiers::default()}));
                assert!(demo.state.confirm_open);assert_eq!(demo.state.confirm_kind,ConfirmKind::CellFormat);assert_eq!(demo.ui.focused(),Some(demo.ids.modal));
                assert_eq!(stored(&demo),original);let modal=demo.ui.modal_state(demo.ids.modal).unwrap();demo.do_rebuild();assert_eq!(demo.ui.modal_state(demo.ids.modal),Some(modal));
                if let Some(folder)=&folder {crate::plot_export::capture(&mut demo,folder,&format!("cell-format-confirm-{kind}-{name}-{width}"));}
                demo.ui.key_input(UiKey::Enter);demo.handle_modal();demo.do_rebuild();
                assert!(!demo.state.confirm_open);assert_eq!(demo.state.edit_value,bad);assert_eq!(stored(&demo),original);assert!(demo.state.has_cell_draft());
                assert_eq!(demo.state.edit_revision,revision);assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value),selection);assert_eq!(demo.ui.focused(),Some(demo.ids.edit_value));
                assert!(demo.ui.key_input(UiKey::Undo));demo.snapshot_inputs();assert!(demo.state.edit_format_issue.is_none());
                assert!(demo.ui.key_input(UiKey::Redo));demo.snapshot_inputs();assert_eq!(demo.state.edit_value,bad);assert!(demo.state.edit_format_issue.is_some());
                demo.handle_button(demo.ids.btn_edit_cell);demo.do_rebuild();assert!(demo.state.confirm_open);
                demo.ui.key_input(UiKey::Tab);demo.do_rebuild();demo.ui.key_input(UiKey::Enter);demo.handle_modal();demo.do_rebuild();
                assert!(!demo.state.confirm_open);assert_eq!(stored(&demo),Value::Text(bad.into()));assert_eq!(demo.state.edit_value,bad);assert!(!demo.state.has_cell_draft());
                assert_eq!(demo.ui.focused(),Some(demo.ids.edit_value));
                demo.state.tab=MainTab::Pragmas;demo.do_rebuild();assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(false));
                assert!(demo.state.edit_format_issue.is_some());assert!(!demo.handle_cell_apply_key(KeyCode::Enter,Modifiers {meta:true,..Modifiers::default()}));
                if width==800 {if let Some(folder)=&folder {crate::plot_export::capture(&mut demo,folder,&format!("cell-format-readonly-{kind}-{name}-{width}"));}}
                demo.state.tab=MainTab::Browse;demo.do_rebuild();
                // A valid edit clears both the marker and its accessible diagnostic.
                demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text(if mode==EditCellMode::Json {"{}"}else{"<r/>"});demo.snapshot_inputs();
                assert!(demo.state.edit_format_issue.is_none());assert_eq!(demo.ui.accessible_name(demo.ids.edit_value),Some("Cell value".into()));demo.state.discard_cell_draft();
                // Reset storage for the next mode/size so Undo reaches valid text.
                demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region='original' WHERE observed='2026-10-01'",[]).unwrap();
                demo.state.refresh_browse().unwrap();demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();
            }
        }
    }
}

#[test]
fn cell_invalid_format_save_all_keeps_destinations_and_defers_document_actions() {
    use crate::state::EditCellMode;
    let dir=tempfile::tempdir().unwrap();let database=dir.path().join("saved.sqlite");let project=dir.path().join("saved.sqbpro");let sql=dir.path().join("saved.sql");
    let mut demo=Demo::new(1000,740,PresentBackend::Softbuffer);demo.state.new_memory_db().unwrap();
    demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES ('saved');").unwrap();
    demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","t").unwrap();demo.state.tab=MainTab::Browse;demo.state.goto_browse_record(1).unwrap();
    demo.state.save_as(database.to_str().unwrap()).unwrap();demo.state.project_path=Some(project.to_string_lossy().into_owned());
    std::fs::write(&sql,"SELECT 'saved';").unwrap();demo.state.open_sql_file(&sql).unwrap();*demo.state.sql_editor_mut()="SELECT 'draft';".into();
    demo.state.tab=MainTab::Browse;demo.state.goto_browse_record(1).unwrap();demo.state.set_edit_mode(EditCellMode::Json).unwrap();demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("{\"键\":\"中文\",bad}");demo.snapshot_inputs();let bad=demo.state.edit_value.clone();
    assert!(demo.state.save_documents(None,Some(&project)).is_err());assert_eq!(std::fs::read_to_string(&sql).unwrap(),"SELECT 'saved';");assert!(!project.exists());
    demo.handle_menu(mid::FILE_SAVE_ALL);demo.do_rebuild();assert!(demo.state.confirm_open);
    assert_eq!(demo.state.pending_cell_save,Some((None,Some(project.clone()))));assert_eq!(std::fs::read_to_string(&sql).unwrap(),"SELECT 'saved';");
    demo.ui.key_input(UiKey::Escape);demo.handle_modal();demo.do_rebuild();
    assert!(!demo.state.confirm_open);assert!(demo.state.pending_cell_save.is_none());assert!(demo.state.has_cell_draft());assert!(!project.exists());assert_eq!(std::fs::read_to_string(&sql).unwrap(),"SELECT 'saved';");
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT v FROM t",[],|r|r.get::<_,String>(0)).unwrap(),"saved");
    demo.request_document_action(DocumentAction::Exit);demo.do_rebuild();assert_eq!(demo.state.confirm_kind,ConfirmKind::Documents);
    demo.ui.key_input(UiKey::Enter);demo.handle_modal();demo.do_rebuild();
    assert_eq!(demo.state.confirm_kind,ConfirmKind::CellFormat);assert_eq!(demo.state.pending_document,Some(DocumentAction::Exit));assert!(!demo.exit_requested);assert!(!project.exists());
    demo.ui.key_input(UiKey::Enter);demo.handle_modal();demo.do_rebuild();assert!(demo.state.has_cell_draft());assert!(!demo.exit_requested);assert!(demo.state.pending_document.is_none());
    demo.request_document_action(DocumentAction::Exit);demo.do_rebuild();demo.ui.key_input(UiKey::Enter);demo.handle_modal();demo.do_rebuild();
    demo.ui.key_input(UiKey::Tab);demo.ui.key_input(UiKey::Enter);demo.handle_modal();demo.do_rebuild();
    assert!(demo.exit_requested);assert!(!demo.state.has_cell_draft());assert!(!demo.state.dirty);assert!(demo.state.pending_cell_save.is_none());
    assert_eq!(std::fs::read_to_string(&sql).unwrap(),"SELECT 'draft';");assert!(project.exists());
    let conn=rusqlite::Connection::open(&database).unwrap();assert_eq!(conn.query_row("SELECT v FROM t",[],|r|r.get::<_,String>(0)).unwrap(),bad);
}

#[test]
fn cell_indent_toggle_keeps_clean_storage_drafts_undo_focus_and_read_only_views() {
    use crate::state::EditCellMode;
    use rusqlite::types::Value;
    let mut demo=Demo::new(1400,860,PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    demo.state.prefs.auto_switch_mode=true;demo.state.prefs.indent_compact=false;
    demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","quarterly_revenue").unwrap();
    demo.state.tab=MainTab::Browse;demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus=DockKind::Plot;demo.state.dock_ratio=0.62;demo.state.body_ratio=0.45;
    demo.state.sync_current_plot().unwrap();demo.state.plot.check(1,1,true);demo.state.plot.check(2,2,true);
    let folder=std::env::var("NEXT_GUI_CELL_INDENT_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
    for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        for (width,height,size) in [(1400,860,13.0),(800,600,18.0)] {
            demo.win_w=width;demo.win_h=height;demo.state.prefs.font_size=size;demo.theme=ui_build::browser_theme(theme,size);
            for (kind,raw,mode) in [("json","{\"n\":18446744073709551616,\"text\":\"中文\"}",EditCellMode::Json),
                ("xml","<?xml version='1.0'?><r><item a='1'>中文</item><items><item/></items></r>",EditCellMode::Xml)] {
                demo.state.tab=MainTab::Browse;demo.state.discard_cell_draft();demo.state.prefs.indent_compact=false;
                demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",[raw]).unwrap();
                demo.state.refresh_browse().unwrap();demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();demo.do_rebuild();
                assert_eq!(demo.state.edit_mode,mode);assert_eq!(demo.state.edit_value,raw);
                for readonly in [false,true] {
                    demo.state.tab=if readonly {MainTab::Pragmas}else{MainTab::Browse};demo.do_rebuild();
                    let revision=demo.state.edit_revision;demo.ui.set_focus(Some(demo.ids.btn_cell_indent));demo.handle_button(demo.ids.btn_cell_indent);demo.do_rebuild();
                    assert!(demo.state.prefs.indent_compact);assert!(demo.state.edit_value.contains('\n'));assert!(!demo.state.has_cell_draft());
                    assert_eq!(demo.ui.focused(),Some(demo.ids.btn_cell_indent));assert!(demo.ui.accessible_name(demo.ids.btn_cell_indent).unwrap().ends_with("(on)"));
                    assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(!readonly));
                    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",[],|r|r.get::<_,Value>(0)).unwrap(),Value::Text(raw.into()));
                    if !readonly {
                        assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value),Some(0));assert_eq!(demo.ui.text_area_scroll_offsets(demo.ids.edit_value),Some((0.0,0.0)));
                        assert_eq!(demo.state.edit_revision,revision);let pretty=demo.state.edit_value.clone();demo.ui.set_focus(Some(demo.ids.edit_value));
                        assert!(demo.ui.key_input(UiKey::Undo));demo.snapshot_inputs();assert_eq!(demo.state.edit_value,raw);assert!(!demo.state.has_cell_draft());
                        assert!(demo.ui.key_input(UiKey::Redo));demo.snapshot_inputs();assert_eq!(demo.state.edit_value,pretty);assert!(!demo.state.has_cell_draft());
                    }
                    if let Some(folder)=&folder {if width==1400 || readonly {demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);crate::plot_export::capture(&mut demo,folder,&format!("cell-indent-{kind}-{name}-{width}-{readonly}"));}}
                    demo.ui.set_focus(Some(demo.ids.btn_cell_indent));demo.handle_button(demo.ids.btn_cell_indent);demo.do_rebuild();
                    assert!(!demo.state.prefs.indent_compact);assert_eq!(demo.state.edit_value,raw);assert!(!demo.state.has_cell_draft());assert_eq!(demo.ui.focused(),Some(demo.ids.btn_cell_indent));
                }
            }
        }
    }
    demo.state.tab=MainTab::Browse;demo.do_rebuild();demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.key_input(UiKey::SelectAll);
    demo.ui.paste_into_focused_text("<r><item>draft 中文</item></r>");demo.snapshot_inputs();let draft=demo.state.edit_value.clone();assert!(demo.state.has_cell_draft());
    demo.ui.set_focus(Some(demo.ids.btn_cell_indent));demo.handle_button(demo.ids.btn_cell_indent);demo.do_rebuild();assert!(demo.state.edit_value.contains('\n'));assert!(demo.state.has_cell_draft());
    demo.ui.set_focus(Some(demo.ids.edit_value));assert!(demo.ui.key_input(UiKey::Undo));demo.snapshot_inputs();assert_eq!(demo.state.edit_value,draft);
    assert!(demo.ui.key_input(UiKey::Redo));demo.snapshot_inputs();demo.state.apply_cell_draft().unwrap();demo.do_rebuild();
    assert_eq!(demo.state.edit_saved,Value::Text(draft));assert!(!demo.state.has_cell_draft());
}

#[test]
fn cell_image_preview_fits_its_pane_and_retains_zoom_focus_without_a_text_editor() {
    use crate::state::EditCellMode;
    use rusqlite::types::Value;
    let mut demo=Demo::new(1400,860,PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    let pixels=image::RgbaImage::from_fn(640,360,|x,y|image::Rgba([(x*255/639) as u8,(y*255/359) as u8,150,if x<320 {255}else{128}]));
    let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(pixels).write_to(&mut bytes,image::ImageOutputFormat::Png).unwrap();
    let value=Value::Blob(bytes.into_inner());
    demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",[&value]).unwrap();
    demo.state.prefs.auto_switch_mode=true;demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","quarterly_revenue").unwrap();
    demo.state.tab=MainTab::Browse;demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus=DockKind::Plot;demo.state.dock_ratio=0.62;demo.state.body_ratio=0.45;
    demo.state.sync_current_plot().unwrap();demo.state.plot.check(1,1,true);demo.state.plot.check(2,2,true);
    let folder=std::env::var("NEXT_GUI_CELL_IMAGE_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
    let bounds=|demo:&Demo,id|demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
    for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        for (width,height,size) in [(1400,860,13.0),(800,600,18.0),(800,600,28.0)] {
            demo.win_w=width;demo.win_h=height;demo.state.prefs.font_size=size;demo.theme=ui_build::browser_theme(theme,size);
            for readonly in [false,true] {
                demo.state.tab=if readonly {MainTab::Pragmas}else{MainTab::Browse};demo.do_rebuild();
                assert_eq!(demo.state.edit_mode,EditCellMode::Image);assert_eq!(demo.ids.edit_value,WidgetId(0));
                assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(!readonly));
                let scroll=demo.ids.dock_scrolls[DockKind::EditCell as usize];
                demo.ui.set_scroll_offset(scroll,0.0,10000.0);demo.ui.sync(&mut demo.scene);
                assert_eq!(demo.ui.scroll_offset(scroll).unwrap().1,0.0);
                let pane=bounds(&demo,scroll);let preview=bounds(&demo,demo.ids.edit_image);
                assert!(preview.height>=96.0 && preview.y>=pane.y && preview.y+preview.height<=pane.y+pane.height+0.1);
                assert!(bounds(&demo,demo.ids.edit_mode).y<preview.y);
                for id in [demo.ids.image_fit,demo.ids.image_actual,demo.ids.image_in,demo.ids.image_out,demo.ids.image_export] {
                    assert_eq!(demo.ui.button_enabled(id),Some(true));assert!(bounds(&demo,id).y<preview.y);
                    assert!(demo.ui.accessible_name(id).is_some());
                }
                let footer=bounds(&demo,demo.ids.btn_edit_cell);
                demo.ui.set_focus(Some(demo.ids.image_actual));demo.handle_button(demo.ids.image_actual);demo.do_rebuild();
                assert_eq!(demo.ui.focused(),Some(demo.ids.image_actual));assert_eq!(demo.state.edit_image_view.scale,Some(1.0));
                demo.ui.set_focus(Some(demo.ids.edit_image));demo.ui.sync(&mut demo.scene);
                let mut view=demo.ui.image_view(demo.ids.edit_image).unwrap();view.offset_x=10000.0;view.offset_y=10000.0;
                demo.ui.set_image_view(demo.ids.edit_image,view);demo.snapshot_inputs();let view=demo.state.edit_image_view;
                demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.edit_image));assert_eq!(demo.ui.image_view(demo.ids.edit_image),Some(view));
                demo.ui.set_focus(Some(demo.ids.image_in));demo.handle_button(demo.ids.image_in);demo.do_rebuild();
                assert_eq!(demo.ui.focused(),Some(demo.ids.image_in));assert_eq!(demo.state.edit_image_view.scale,Some(2.0));
                demo.ui.set_focus(Some(demo.ids.image_fit));demo.handle_button(demo.ids.image_fit);demo.do_rebuild();
                assert_eq!(demo.ui.focused(),Some(demo.ids.image_fit));assert_eq!(demo.state.edit_image_view.scale,None);
                assert_eq!(bounds(&demo,demo.ids.btn_edit_cell),footer);assert_eq!(demo.state.edit_saved,value);assert!(!demo.state.has_cell_draft());
                if let Some(folder)=&folder {demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);crate::plot_export::capture(&mut demo,folder,&format!("cell-image-{name}-{width}-{size}-{readonly}"));}
            }
        }
    }
    demo.state.tab=MainTab::Browse;demo.do_rebuild();demo.state.set_edit_mode(EditCellMode::Text).unwrap();demo.do_rebuild();
    assert_ne!(demo.ids.edit_value,WidgetId(0));assert_eq!(demo.ids.edit_image,WidgetId(0));
    demo.state.set_edit_mode(EditCellMode::Image).unwrap();demo.state.edit_image_rgba=None;demo.do_rebuild();
    assert_eq!(demo.ids.edit_value,WidgetId(0));assert_ne!(demo.ids.edit_image,WidgetId(0));
    for id in [demo.ids.image_fit,demo.ids.image_actual,demo.ids.image_in,demo.ids.image_out,demo.ids.image_export] {assert_eq!(demo.ui.button_enabled(id),Some(false));}
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",[],|row|row.get::<_,Value>(0)).unwrap(),value);
}

#[test]
fn cell_auto_mode_controls_keep_focus_drafts_storage_history_and_read_only_views() {
    use crate::state::EditCellMode;
    use rusqlite::types::Value;
    let mut demo=Demo::new(1400,860,PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    let raw="{\"name\":\"中文\",\"steps\":[1,true,null]}";
    demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",[raw]).unwrap();
    demo.state.prefs.auto_switch_mode=true;demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","quarterly_revenue").unwrap();
    demo.state.tab=MainTab::Browse;demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus=DockKind::Plot;demo.state.dock_ratio=0.62;demo.state.body_ratio=0.45;
    demo.state.sync_current_plot().unwrap();demo.state.plot.check(1,1,true);demo.state.plot.check(2,2,true);
    let folder=std::env::var("NEXT_GUI_CELL_AUTO_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
    for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        for (width,height,size) in [(1400,860,13.0),(800,600,18.0)] {
            demo.win_w=width;demo.win_h=height;demo.state.prefs.font_size=size;demo.theme=ui_build::browser_theme(theme,size);
            demo.state.set_cell_auto_switch(false).unwrap();demo.state.set_edit_mode(EditCellMode::Hex).unwrap();demo.do_rebuild();
            for enabled in [false,true] {
                demo.ui.set_focus(Some(demo.ids.btn_cell_auto));
                if demo.state.prefs.auto_switch_mode!=enabled {demo.handle_button(demo.ids.btn_cell_auto);demo.do_rebuild();}
                demo.ui.sync(&mut demo.scene);
                assert_eq!(demo.ui.focused(),Some(demo.ids.btn_cell_auto));assert_eq!(demo.state.prefs.auto_switch_mode,enabled);
                assert_eq!(demo.state.edit_mode,if enabled {EditCellMode::Json}else{EditCellMode::Hex});
                assert_eq!(demo.state.edited_cell_value().unwrap(),Value::Text(raw.into()));assert!(!demo.state.has_cell_draft());
                if let Some(folder)=&folder {demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);crate::plot_export::capture(&mut demo,folder,&format!("cell-auto-{name}-{width}-{}",if enabled {"on"}else{"off"}));}
            }
        }
        demo.win_w=1400;demo.win_h=860;demo.state.prefs.font_size=13.0;demo.theme=ui_build::browser_theme(theme,13.0);
        for (format,kind) in [(image::ImageOutputFormat::Png,"png"),(image::ImageOutputFormat::Bmp,"bmp")] {
            let pixels=image::RgbaImage::from_fn(320,180,|x,y|image::Rgba([(x*255/319) as u8,(y*255/179) as u8,150,if x<160 {255}else{128}]));
            let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(pixels).write_to(&mut bytes,format).unwrap();
            let value=Value::Blob(bytes.into_inner());
            demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",[&value]).unwrap();
            demo.state.refresh_browse().unwrap();demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();demo.do_rebuild();
            assert_eq!(demo.state.edit_mode,EditCellMode::Image);assert_eq!(demo.state.edit_saved,value);assert!(!demo.state.has_cell_draft());
            assert_eq!(demo.state.edit_image_kind.to_lowercase(),kind);assert!(demo.state.edit_image_rgba.is_some());
            demo.ui.set_focus(Some(demo.ids.edit_image));demo.ui.sync(&mut demo.scene);
            if let Some(folder)=&folder {demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);crate::plot_export::capture(&mut demo,folder,&format!("cell-auto-image-{kind}-{name}"));}
        }
        demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",[raw]).unwrap();
        demo.state.refresh_browse().unwrap();demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);demo.state.sync_edit_cell_from_selection();
        demo.state.set_cell_auto_switch(false).unwrap();demo.state.set_edit_mode(EditCellMode::Hex).unwrap();demo.state.tab=MainTab::Pragmas;demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.btn_cell_auto));demo.handle_button(demo.ids.btn_cell_auto);demo.do_rebuild();
        assert_eq!(demo.ui.focused(),Some(demo.ids.btn_cell_auto));assert_eq!(demo.state.edit_mode,EditCellMode::Json);
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(false));assert!(!demo.state.has_cell_draft());
        if let Some(folder)=&folder {demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);crate::plot_export::capture(&mut demo,folder,&format!("cell-auto-readonly-{name}"));}
        demo.state.tab=MainTab::Browse;
    }
    demo.do_rebuild();demo.state.set_edit_mode(EditCellMode::Text).unwrap();demo.do_rebuild();demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("draft 中文");demo.snapshot_inputs();
    let revision=demo.state.edit_revision;
    for _ in 0..2 {demo.ui.set_focus(Some(demo.ids.btn_cell_auto));demo.handle_button(demo.ids.btn_cell_auto);demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.btn_cell_auto));}
    assert_eq!(demo.state.edit_revision,revision,"mode={:?}, value={:?}, prefs={}, error={:?}",demo.state.edit_mode,demo.state.edit_value,demo.state.prefs.auto_switch_mode,demo.state.error_msg);assert_eq!(demo.state.edit_value,"draft 中文");assert!(demo.state.has_cell_draft());
    demo.ui.set_focus(Some(demo.ids.edit_value));assert!(demo.ui.key_input(UiKey::Undo));demo.snapshot_inputs();assert_eq!(demo.state.edit_value,raw);assert!(!demo.state.has_cell_draft());
    assert!(demo.ui.key_input(UiKey::Redo));demo.snapshot_inputs();assert_eq!(demo.state.edit_value,"draft 中文");
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",[],|row|row.get::<_,String>(0)).unwrap(),raw);
}

#[test]
fn cell_editor_sessions_survive_rebuilds_but_never_follow_another_cell_or_replacement() {
    let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    let original = (0..40).map(|line| format!("第{line:02}行 alpha 中文🙂\n")).collect::<String>();
    let conn = demo.state.conn.as_ref().unwrap();
    conn.execute_batch("CREATE TABLE cells(id INTEGER PRIMARY KEY, value TEXT); CREATE TABLE other(id INTEGER PRIMARY KEY, value TEXT);").unwrap();
    for table in ["cells", "other"] {
        for id in [1, 2] {
            conn.execute(&format!("INSERT INTO {table} VALUES(?1,?2)"), rusqlite::params![id, original]).unwrap();
        }
    }
    demo.state.refresh_schema().unwrap(); demo.state.tab = MainTab::Browse;
    demo.state.switch_browse_table_in("main", "cells").unwrap();
    demo.state.goto_browse_record(1).unwrap(); demo.state.browse_mut().sel_col = Some(1);
    demo.state.sync_edit_cell_from_selection(); demo.state.dock_focus = DockKind::Plot;
    demo.state.dock_ratio = 0.55;
    let folder = std::env::var("NEXT_GUI_CELL_SESSION_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder) = &folder { std::fs::create_dir_all(folder).unwrap(); }
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.edit_value));
        demo.ui.select_text_area_range(demo.ids.edit_value, 2..6);
        demo.ui.paste_into_focused_text("编辑"); demo.snapshot_inputs();
        let changed = demo.state.edit_value.clone();
        assert_ne!(changed, original); assert!(demo.state.has_cell_draft());
        for (width, height, size) in [(1400, 1000, 13.0), (800, 600, 18.0)] {
            demo.win_w = width; demo.win_h = height; demo.state.prefs.font_size = size;
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.edit_value));
            assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value), Some(4));
            assert_eq!(demo.ui.text_area_value(demo.ids.edit_value), Some(changed.as_str()));
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs();
            demo.state.browse_mut().sel_row_start = None;
            demo.state.browse_mut().sel_col_start = None;
            let revision = demo.state.edit_revision;
            demo.poll_selection_changes();
            assert_eq!(demo.state.edit_revision, revision, "Range anchors must not reload the current cell");
            demo.do_rebuild();
            assert_eq!(demo.state.edit_value, original);
            assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value), Some(2..6));
            assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.state.edit_value, changed);
            let end = changed.chars().count(); demo.ui.select_text_area_range(demo.ids.edit_value, end..end); demo.ui.sync(&mut demo.scene); demo.snapshot_inputs();
            let scroll = demo.ui.text_area_scroll(demo.ids.edit_value).unwrap(); assert!(scroll > 0.0);
            demo.do_rebuild(); assert_eq!(demo.ui.text_area_scroll(demo.ids.edit_value), Some(scroll));
            demo.ui.select_text_area_range(demo.ids.edit_value, std::ops::Range { start: 4, end: 2 });
            demo.ui.ime_preedit("東京", Some((3, 6))); demo.ui.sync(&mut demo.scene);
            let candidate = demo.ui.ime_cursor_area(&demo.scene); assert!(candidate.is_some());
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene), candidate);
            assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value), Some(2));
            assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("编辑"));
            if let Some(folder) = &folder { crate::plot_export::capture(&mut demo, folder, &format!("cell-session-{name}-{width}")); }
            assert!(demo.ui.ime_commit("東京")); demo.snapshot_inputs(); demo.do_rebuild();
            assert!(demo.state.edit_value.contains("東京"));
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.state.edit_value, changed);
            demo.ui.select_text_area_range(demo.ids.edit_value, 2..4); demo.snapshot_inputs();
        }
        demo.ui.set_focus(Some(demo.ids.edit_col)); demo.ui.key_input(UiKey::SelectAll);
        demo.ui.paste_into_focused_text("temporary"); demo.snapshot_inputs(); demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.edit_col));
        assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); demo.do_rebuild();
        assert_eq!(demo.state.edit_col, "value");
        demo.ui.set_focus(Some(demo.ids.edit_type)); demo.ui.key_input(UiKey::Space); demo.do_rebuild();
        assert_eq!(demo.ui.popup_owner(), Some(demo.ids.edit_type)); demo.ui.key_input(UiKey::Escape);
        demo.ui.set_focus(Some(demo.ids.edit_value));
        // Closing removes the widgets; their same-cell session must outlive the panel.
        demo.snapshot_inputs(); demo.handle_menu(mid::VIEW_DOCK_EDIT); demo.do_rebuild();
        assert_eq!(demo.ids.edit_value, WidgetId(0));
        demo.handle_menu(mid::VIEW_DOCK_EDIT); demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.edit_value));
        assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value), Some(2..4));
        assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); demo.do_rebuild();
        assert_eq!(demo.state.edit_value, original);
        assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs();
        assert_eq!(demo.state.edit_value, changed);
        demo.state.discard_cell_draft(); demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, original);
        demo.win_w = 1400; demo.win_h = 1000; demo.state.prefs.font_size = 13.0;
    }
    // Leave redo and column-name history whose current text equals the next cell.
    demo.do_rebuild(); demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.paste_into_focused_text("old cell only"); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
    demo.ui.set_focus(Some(demo.ids.edit_col)); demo.ui.key_input(UiKey::SelectAll);
    demo.ui.paste_into_focused_text("temporary"); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
    assert_eq!(demo.state.edit_col, "value"); assert_eq!(demo.state.edit_value, original);
    demo.state.goto_browse_record(2).unwrap();
    demo.snapshot_inputs(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_col)); demo.ui.key_input(UiKey::Redo);
    assert_eq!(demo.ui.text_input_value(demo.ids.edit_col), Some("value"));
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::Redo);
    demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, original);
    // File/NULL/mode replacement also invalidates history before a stale UI snapshot.
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("value.txt");
    std::fs::write(&path, "replacement").unwrap();
    demo.ui.paste_into_focused_text("stale draft"); demo.snapshot_inputs();
    demo.state.import_cell_file(&path).unwrap(); demo.snapshot_inputs(); demo.do_rebuild();
    assert_eq!(demo.state.edit_value, "replacement"); demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, "replacement");
    demo.state.set_cell_null().unwrap(); demo.do_rebuild(); demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs(); assert_eq!(demo.state.edit_type, 0);
    assert_eq!(demo.state.edit_value, ""); demo.state.discard_cell_draft(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.paste_into_focused_text("not for other table");
    demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
    demo.state.switch_browse_table_in("main", "other").unwrap(); demo.state.goto_browse_record(1).unwrap();
    demo.state.browse_mut().sel_col = Some(1); demo.state.sync_edit_cell_from_selection(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
    assert_eq!(demo.state.edit_value, original); assert!(!demo.state.has_cell_draft());
    demo.state.tab = MainTab::ExecuteSql;
    demo.state.run_sql_text_sync("SELECT value FROM cells ORDER BY id").unwrap();
    demo.state.sql_selection.rows = Some((0,0)); demo.state.sync_edit_cell_from_sql_selection().unwrap(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.select_text_area_range(demo.ids.edit_value, 2..6);
    demo.snapshot_inputs(); demo.do_rebuild();
    assert_eq!(demo.ui.focused(), Some(demo.ids.edit_value));
    assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("0行 a"));
    assert!(!demo.ui.ime_preedit("unsafe", None)); demo.ui.key_input(UiKey::Redo);
    demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, original); assert!(!demo.state.has_cell_draft());
    demo.state.run_sql_text_sync("SELECT value FROM cells ORDER BY id").unwrap();
    demo.state.sql_selection.rows = Some((0,0)); assert!(demo.state.sync_edit_cell_from_sql_selection().unwrap());
    demo.snapshot_inputs(); demo.do_rebuild();
    let end = original.chars().count();
    assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value), Some(end..end));
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT value FROM cells WHERE id=1", [], |row| row.get::<_,String>(0)).unwrap(), original);
}

#[test]
fn cell_modes_keep_keyboard_popups_formatting_undo_null_input_and_explicit_evaluation() {
    use crate::state::EditCellMode;
    use rusqlite::types::Value;
    let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    let xml = "<report xmlns='urn:quarter'><name>中文</name><items><item value='42'/></items></report>";
    demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'", [xml]).unwrap();
    demo.state.refresh_schema().unwrap(); demo.state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
    demo.state.tab = MainTab::Browse; demo.state.goto_browse_record(1).unwrap();
    demo.state.browse_mut().sel_col = Some(3); demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus = DockKind::Plot; demo.state.dock_ratio = 0.62; demo.state.body_ratio = 0.45;
    demo.state.sync_current_plot().unwrap(); demo.state.plot.check(1,1,true); demo.state.plot.check(2,2,true);
    let folder = std::env::var("NEXT_GUI_CELL_FORMATS_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder) = &folder { std::fs::create_dir_all(folder).unwrap(); }
    for (name, theme) in [("dark",Theme::dark()), ("light",Theme::light())] {
        for (width,height,size) in [(1400,860,13.0),(800,600,18.0)] {
            demo.win_w = width; demo.win_h = height; demo.state.prefs.font_size = size;
            demo.theme = ui_build::browser_theme(theme,size);
            for index in 0..EditCellMode::LABELS.len() {
                demo.state.discard_cell_draft(); demo.do_rebuild();
                let id = demo.ids.edit_mode; demo.ui.set_focus(Some(id));
                assert_eq!(demo.ui.accessible_name(id).as_deref(), Some("Cell editor mode"));
                assert!(demo.ui.key_input(UiKey::Space)); demo.ui.sync(&mut demo.scene); demo.snapshot_inputs();
                demo.do_rebuild(); assert_eq!(demo.ui.popup_owner(),Some(demo.ids.edit_mode));
                let style = demo.ui.select_style();
                let popup = demo.scene.iter_paint_order().into_iter().find(|node| node.visible && node.fill == style.popup_bg
                    && demo.ui.widget_from_node(node.id) == Some(demo.ids.edit_mode) && node.bounds.height >= style.item_height*6.0).unwrap().bounds;
                assert!(popup.x >= 0.0 && popup.y >= 0.0 && popup.x+popup.width <= width as f32 && popup.y+popup.height <= height as f32);
                if index == 0 && width == 800 {
                    if let Some(folder) = &folder { crate::plot_export::capture(&mut demo,folder,&format!("cell-mode-menu-{name}-{width}")); }
                }
                assert_eq!(demo.ui.select_selected(demo.ids.edit_mode),Some(0));
                for _ in 0..index {
                    assert!(demo.ui.key_input(UiKey::Down));
                    demo.snapshot_inputs(); demo.poll_selection_changes();
                    demo.do_rebuild();
                    assert_eq!(demo.ui.popup_owner(),Some(demo.ids.edit_mode)); assert_eq!(demo.state.edit_mode,EditCellMode::Text);
                }
                assert!(demo.ui.key_input(UiKey::Enter)); demo.snapshot_inputs(); demo.poll_selection_changes(); demo.do_rebuild();
                assert_eq!(demo.state.edit_mode,EditCellMode::from_index(index).unwrap());
                assert_eq!(demo.ui.focused(),Some(demo.ids.edit_mode)); assert!(!demo.ui.has_open_popup());
                assert!(!demo.state.has_cell_draft());
                if demo.state.edit_mode == EditCellMode::Image {
                    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.paste_into_focused_text("unsafe"); demo.snapshot_inputs();
                    assert!(demo.state.edit_value.contains("read-only")); assert!(!demo.state.has_cell_draft());
                }
                if matches!(demo.state.edit_mode,EditCellMode::Xml|EditCellMode::Evaluation) {
                    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.sync(&mut demo.scene);
                    if let Some(folder) = &folder { crate::plot_export::capture(&mut demo,folder,&format!("cell-mode-{}-{name}-{width}",EditCellMode::LABELS[index].to_lowercase())); }
                }
            }
        }
    }
    demo.state.discard_cell_draft(); demo.state.set_edit_mode(EditCellMode::Xml).unwrap(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::SelectAll);
    demo.ui.paste_into_focused_text("<r><item>值</item></r>"); demo.snapshot_inputs();
    demo.handle_button(demo.ids.btn_cell_indent); demo.do_rebuild();
    assert!(demo.state.edit_value.contains('\n')); assert!(demo.state.has_cell_draft());
    assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); demo.do_rebuild();
    assert_eq!(demo.state.edit_value,"<r><item>值</item></r>");
    assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs(); demo.do_rebuild(); assert!(demo.state.edit_value.contains('\n'));
    demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text("<r><broken>"); demo.snapshot_inputs();
    demo.handle_button(demo.ids.btn_cell_indent); demo.do_rebuild();
    assert!(!demo.state.prefs.indent_compact);assert_eq!(demo.state.edit_value,"<r><broken>");
    demo.handle_button(demo.ids.btn_cell_indent); demo.do_rebuild();
    assert!(demo.state.error_msg.as_deref().unwrap().contains("Invalid XML")); assert_eq!(demo.state.edit_value,"<r><broken>");
    demo.ui.set_select_selected(demo.ids.edit_mode,Some(EditCellMode::Text as usize)); demo.snapshot_inputs(); demo.poll_selection_changes(); demo.do_rebuild();
    assert_eq!(demo.state.edit_value,"<r><broken>"); assert!(demo.state.has_cell_draft());
    demo.state.discard_cell_draft(); demo.do_rebuild();
    demo.ui.set_select_selected(demo.ids.edit_type,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
    assert_eq!(demo.state.edit_type,0); assert!(demo.state.edit_value.is_empty());
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.paste_into_focused_text("new text"); demo.snapshot_inputs(); demo.do_rebuild();
    assert_eq!(demo.state.edit_type,3); assert_eq!(demo.state.edited_cell_value().unwrap(),Value::Text("new text".into()));
    demo.state.set_cell_null().unwrap(); demo.state.set_edit_mode(EditCellMode::Hex).unwrap(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.paste_into_focused_text("00FF"); demo.snapshot_inputs();
    assert_eq!(demo.state.edit_type,4); assert_eq!(demo.state.edited_cell_value().unwrap(),Value::Blob(vec![0,255]));
    demo.state.discard_cell_draft(); demo.state.set_edit_mode(EditCellMode::Evaluation).unwrap(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text("upper('中文a')");
    demo.snapshot_inputs(); demo.do_rebuild(); assert!(demo.state.has_cell_draft());
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",[],|row|row.get::<_,String>(0)).unwrap(),xml);
    let command = Modifiers { meta:true, ..Modifiers::default() };
    assert!(!demo.handle_cell_apply_key(KeyCode::Enter,Modifiers::default()));
    assert!(!demo.handle_cell_apply_key(KeyCode::Enter,Modifiers { alt:true, ..command }));
    assert!(demo.handle_cell_apply_key(KeyCode::Enter,command)); demo.do_rebuild();
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",[],|row|row.get::<_,String>(0)).unwrap(),"中文A");
    assert!(!demo.state.has_cell_draft()); assert_eq!(demo.state.edit_mode,EditCellMode::Text);
    assert_eq!(demo.state.edit_value,"中文A"); assert_eq!(demo.ui.focused(),Some(demo.ids.edit_value));
    if let Some(folder) = &folder { crate::plot_export::capture(&mut demo,folder,"cell-evaluation-applied-light-800"); }
    demo.ui.key_input(UiKey::SelectAll); assert_eq!(demo.ui.focused_text_for_copy().as_deref(),Some("中文A"));
    demo.state.tab = MainTab::Pragmas; demo.do_rebuild(); demo.ui.set_focus(Some(demo.ids.edit_value));
    assert!(!demo.handle_cell_apply_key(KeyCode::Enter,command));
}

#[test]
fn cell_dock_actions_and_content_fit_font_toolbar_and_window_changes() {
    let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    demo.state.refresh_schema().unwrap(); demo.state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
    demo.state.tab = MainTab::Browse; demo.state.goto_browse_record(1).unwrap();
    demo.state.browse_mut().sel_col = Some(3); demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus = DockKind::Plot; demo.state.dock_ratio = 0.46; demo.state.body_ratio = 0.45;
    demo.state.sync_current_plot().unwrap(); demo.state.plot.check(1,1,true); demo.state.plot.check(2,2,true);
    demo.do_rebuild(); demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::SelectAll);
    demo.ui.paste_into_focused_text("<r><item>中文 draft</item></r>"); demo.snapshot_inputs();
    let draft = demo.state.edit_value.clone();
    let caret = demo.ui.text_area_caret(demo.ids.edit_value);
    let folder = std::env::var("NEXT_GUI_CELL_LAYOUT_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder) = &folder { std::fs::create_dir_all(folder).unwrap(); }
    let bounds = |demo: &Demo, id| demo.scene.iter_paint_order().into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(id)).unwrap_or_else(|| panic!("No node for {id:?}")).bounds;
    for (name, theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        for size in [13.0,18.0,28.0] { for style in 0..5 { for (width,height) in [(1400,860),(800,600)] {
            demo.win_w = width; demo.win_h = height; demo.state.prefs.font_size = size; demo.state.prefs.toolbar_styles = [style;5];
            demo.theme = ui_build::browser_theme(theme,size); demo.state.shell_scroll = (0.0,0.0);
            demo.do_rebuild();
            assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value),caret);
            assert_eq!(demo.state.edit_value,draft); assert!(demo.state.has_cell_draft());
            assert!((demo.state.dock_ratio-0.46).abs()<0.001);
            let line = demo.ui.font().measure_line("Ag",demo.theme.typography.body).1;
            let value = bounds(&demo,demo.ids.edit_value);
            let apply = bounds(&demo,demo.ids.btn_edit_cell);
            let discard = bounds(&demo,demo.ids.btn_edit_discard);
            let meta = bounds(&demo,demo.ids.edit_meta);
            let chart = bounds(&demo,demo.ids.chart);
            assert!(value.height>=64.0 && apply.height>=line+10.0 && discard.height>=line+10.0,
                "{name}/{width}/{size}/{style}: value={value:?},apply={apply:?},line={line}");
            assert!(value.y+value.height<=apply.y && apply.y+apply.height<=meta.y && meta.y+meta.height<chart.y,
                "{name}/{width}/{size}/{style}: value={value:?},apply={apply:?},meta={meta:?},chart={chart:?}");
            assert!(chart.height>=164.0);
            let plot_image = demo.scene.iter_paint_order().into_iter().find(|node| node.visible && node.image.is_some()).unwrap().bounds;
            assert!(plot_image.height>=110.0,"{name}/{width}/{size}/{style}: plot body={plot_image:?}");
            // Focus reveals controls through the existing shell/row scrolls at large fonts.
            for id in [demo.ids.btn_edit_cell,demo.ids.btn_edit_discard,demo.ids.edit_mode,demo.ids.btn_cell_export,demo.ids.chart] {
                demo.ui.set_focus(Some(id)); demo.ui.sync(&mut demo.scene);
                let b = bounds(&demo,id);
                let x = b.x+b.width*0.5; let y = b.y+b.height*0.5;
                assert!(x>=0.0 && x<width as f32 && y>=0.0 && y<height as f32-40.0,
                    "{name}/{width}/{size}/{style}: control={id:?},bounds={b:?}");
                assert_eq!(demo.ui.hit_widget(&demo.scene,x,y),Some(id),"{name}/{width}/{size}/{style}: bounds={b:?}");
            }
            demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.sync(&mut demo.scene);
            demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value),caret); assert_eq!(demo.state.edit_value,draft);
            if (size==13.0 && width==1400 && style==0) || (width==800 && ((size==18.0 && matches!(style,2|3)) || (size==28.0 && style==1))) {
                if let Some(folder) = &folder {
                    if width==800 { demo.ui.set_focus(Some(demo.ids.chart)); demo.ui.sync(&mut demo.scene); }
                    crate::plot_export::capture(&mut demo,folder,&format!("cell-layout-{name}-{width}-font{size}-style{style}"));
                }
            }
        }}}
    }
    demo.win_w=800; demo.win_h=600; demo.state.prefs.font_size=18.0; demo.state.prefs.toolbar_styles=[3;5];
    demo.theme=ui_build::browser_theme(Theme::dark(),18.0);
    demo.state.discard_cell_draft(); demo.state.set_edit_mode(crate::state::EditCellMode::Image).unwrap(); demo.do_rebuild();
    let footer = bounds(&demo,demo.ids.btn_edit_cell);
    let scroll = demo.ids.dock_scrolls[DockKind::EditCell as usize];
    demo.ui.set_scroll_offset(scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene);
    assert_eq!(demo.ui.scroll_offset(scroll).unwrap().1,0.0,"Image preview must fit its pane");
    assert_eq!(bounds(&demo,demo.ids.btn_edit_cell),footer,"Image preview must not move the action footer");
    demo.state.tab=MainTab::Pragmas; demo.do_rebuild();
    assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(false));
    assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_discard),Some(false));
}

#[test]
fn cell_wrap_toggle_preserves_drafts_selection_history_and_read_only_views() {
    let mut demo=Demo::new(1400,860,PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    let line="SQL 中文🙂 value ".repeat(80);
    let original=format!("开始 • {line} — FIRST_END\n第二行 • {line} — LAST_END");
    demo.state.conn.as_ref().unwrap().execute("UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",[&original]).unwrap();
    demo.state.refresh_schema().unwrap();demo.state.switch_browse_table_in("main","quarterly_revenue").unwrap();
    demo.state.tab=MainTab::Browse;demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(3);
    demo.state.sync_edit_cell_from_selection();demo.state.dock_focus=DockKind::Plot;demo.state.body_ratio=0.45;
    demo.state.sync_current_plot().unwrap();demo.state.plot.check(1,1,true);demo.state.plot.check(2,2,true);
    demo.state.prefs.editor_word_wrap=true;demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.paste_into_focused_text(" draft 编辑");demo.snapshot_inputs();
    let draft=demo.state.edit_value.clone();assert_ne!(draft,original);let end=draft.chars().count();
    let revision=demo.state.edit_revision;
    let folder=std::env::var("NEXT_GUI_CELL_WRAP_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
    for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        for (width,height,size) in [(1400,860,13.0),(800,600,18.0)] {
            demo.win_w=width;demo.win_h=height;demo.state.prefs.font_size=size;demo.theme=ui_build::browser_theme(theme,size);
            demo.do_rebuild();
            for enabled in [false,true] {
                demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.select_text_area_range(demo.ids.edit_value,end-5..end);
                demo.ui.sync(&mut demo.scene);demo.snapshot_inputs();
                demo.ui.set_focus(Some(demo.ids.btn_cell_wrap));
                if demo.state.prefs.editor_word_wrap!=enabled {demo.handle_button(demo.ids.btn_cell_wrap);}
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(),Some(demo.ids.btn_cell_wrap));
                assert_eq!(demo.state.edit_revision,revision);assert_eq!(demo.state.edit_value,draft);assert!(demo.state.has_cell_draft());
                assert_eq!(demo.ui.text_area_wrap(demo.ids.edit_value),Some(enabled));
                assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value),Some(end-5..end));
                assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value),Some(end));
                let (x,y)=demo.ui.text_area_scroll_offsets(demo.ids.edit_value).unwrap();
                if enabled {assert_eq!(x,0.0);assert!(y>0.0);} else {assert!(x>100.0);}
                demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.sync(&mut demo.scene);
                assert_eq!(demo.ui.focused_text_for_copy(),Some(draft.chars().skip(end-5).collect()));
                if let Some(folder)=&folder {
                    demo.ui.set_focus(Some(demo.ids.chart));demo.ui.sync(&mut demo.scene);
                    crate::plot_export::capture(&mut demo,folder,&format!("cell-wrap-{name}-{width}-{}",if enabled {"on"} else {"off"}));
                }
            }
        }
    }
    demo.ui.set_focus(Some(demo.ids.edit_value));assert!(demo.ui.key_input(UiKey::Undo));demo.snapshot_inputs();demo.do_rebuild();
    assert_eq!(demo.state.edit_value,original);assert!(!demo.state.has_cell_draft());
    assert!(demo.ui.key_input(UiKey::Redo));demo.snapshot_inputs();demo.do_rebuild();assert_eq!(demo.state.edit_value,draft);
    demo.state.tab=MainTab::Pragmas;demo.do_rebuild();
    assert_eq!(demo.ui.button_enabled(demo.ids.btn_cell_wrap),Some(true));
    assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(false));
    demo.handle_button(demo.ids.btn_cell_wrap);demo.do_rebuild();
    assert_eq!(demo.state.edit_value,draft);assert_eq!(demo.ui.text_area_wrap(demo.ids.edit_value),Some(false));
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",[],|row|row.get::<_,String>(0)).unwrap(),original);
}
