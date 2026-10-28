use super::*;

fn bounds(demo: &Demo, id: WidgetId) -> next_gui_scene::Rect {
    demo.scene.iter_paint_order().into_iter().find(|node| demo.ui.widget_from_node(node.id) == Some(id)).unwrap().bounds
}

#[test]
fn toolbar_styles_reach_all_groups_reveal_actions_and_keep_cell_drafts_and_query_sources() {
    let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap(); demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
    demo.state.refresh_schema().unwrap(); demo.state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
    demo.state.tab = MainTab::Browse; demo.state.goto_browse_record(1).unwrap(); demo.state.browse_mut().sel_col = Some(1);
    demo.state.sync_edit_cell_from_selection();
    *demo.state.sql_editor_mut() = "SELECT observed, revenue, expenses, region FROM quarterly_revenue;".into();
    demo.state.run_sql_sync().unwrap(); demo.state.sql_selection.rows = Some((0,0)); demo.state.sql_selection.columns = Some((1,1));
    demo.state.goto_browse_record(1).unwrap(); demo.state.browse_mut().sel_col = Some(1); demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus = DockKind::Plot; demo.state.plot.settings.manual = false; demo.state.sync_current_plot().unwrap();
    demo.state.plot.check(1,1,true); demo.state.plot.check(2,2,true); demo.state.body_ratio = 0.45;
    let snapshot = demo.state.sql_result.as_ref().unwrap().all_values.clone().unwrap();
    let folder = std::env::var("NEXT_GUI_TOOLBAR_STYLES_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder) = &folder { std::fs::create_dir_all(folder).unwrap(); }
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
        for style in 0..5 {
            demo.state.prefs.toolbar_styles = [style;5];
            for (width,height) in [(1400,860),(800,600)] {
                demo.win_w = width; demo.win_h = height;
                for (page,tab) in [("structure",MainTab::Structure),("browse",MainTab::Browse),("sql",MainTab::ExecuteSql)] {
                    demo.state.tab = tab; demo.do_rebuild();
                    let mut buttons = vec![demo.ids.btn_new, demo.ids.btn_open, demo.ids.btn_write, demo.ids.btn_revert, demo.ids.toolbar_extra,
                        demo.ids.btn_integrity, demo.ids.toolbar_project, demo.ids.btn_project_save, demo.ids.btn_theme,
                        demo.ids.btn_cell_import, demo.ids.btn_cell_export, demo.ids.btn_cell_null, demo.ids.btn_cell_indent];
                    buttons.extend(match tab {
                        MainTab::Structure => vec![demo.ids.btn_struct_new_table,demo.ids.btn_struct_new_index,demo.ids.btn_struct_modify,demo.ids.btn_struct_delete,demo.ids.btn_struct_refresh],
                        MainTab::Browse => vec![demo.ids.btn_add_row,demo.ids.btn_del_row,demo.ids.btn_form_view,demo.ids.btn_freeze,demo.ids.btn_cond_fmt,demo.ids.btn_fk_nav],
                        _ => vec![demo.ids.btn_sql_new_tab,demo.ids.btn_sql_open,demo.ids.btn_sql_save,demo.ids.btn_sql_save_as,demo.ids.btn_sql_run,demo.ids.btn_sql_line,demo.ids.btn_sql_stop,demo.ids.btn_sql_find,demo.ids.btn_sql_comment],
                    });
                    for button in buttons {
                        let rect = bounds(&demo,button);
                        if matches!(style,0|4) { assert_eq!(rect.width,rect.height); }
                        else if style == 3 { assert!(rect.height > 40.0); }
                        else { assert!(rect.width > rect.height); }
                        assert!(demo.ui.accessible_name(button).unwrap().len() >= 5);
                        if demo.ui.button_enabled(button) == Some(true) {
                            demo.ui.set_focus(Some(button)); demo.ui.sync(&mut demo.scene);
                            let rect = bounds(&demo,button);
                            assert_eq!(demo.ui.focused(),Some(button));
                            assert_eq!(demo.ui.hit_widget(&demo.scene,rect.x+rect.width*0.5,rect.y+rect.height*0.5),Some(button),"{name}-{style}-{page}-{width}: {rect:?}");
                        }
                    }
                    assert_eq!(demo.ui.button_enabled(demo.ids.btn_cell_import),Some(tab == MainTab::Browse));
                    assert_eq!(demo.ui.button_enabled(demo.ids.btn_cell_null),Some(tab == MainTab::Browse));
                    let capture = matches!((name,style,page,width),
                        ("dark",0,"browse",1400)|("light",1,"sql",800)|("dark",2,"structure",800)|("light",2,"browse",1400)|
                        ("dark",3,"structure",1400)|("light",3,"browse",800)|("dark",3,"sql",800)|("light",4,"sql",1400));
                    if capture {
                        demo.ui.set_focus(Some(demo.ids.btn_new)); demo.ui.sync(&mut demo.scene); demo.ui.set_focus(None);
                        demo.ui.set_scroll_offset(demo.ids.dock_scrolls[DockKind::EditCell as usize],0.0,0.0); demo.ui.sync(&mut demo.scene);
                        if let Some(folder) = &folder { crate::plot_export::capture(&mut demo,folder,&format!("toolbar-style-{style}-{name}-{page}-{width}")); }
                    }
                    assert!(!demo.state.has_cell_draft());
                    assert!(std::sync::Arc::ptr_eq(&snapshot,demo.state.sql_result.as_ref().unwrap().all_values.as_ref().unwrap()));
                }
            }
        }
    }
    demo.state.tab = MainTab::Browse; demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.btn_cell_null)); assert!(demo.ui.key_input(UiKey::Enter));
    let button = demo.ui.take_button_activate().unwrap(); demo.handle_button(button); demo.do_rebuild();
    assert!(demo.state.has_cell_draft()); assert_eq!(demo.state.edit_type,0);
    demo.state.prefs.toolbar_styles = [2,1,3,0,2]; demo.do_rebuild(); assert!(demo.state.has_cell_draft());
    assert_eq!(demo.state.edit_type,0); demo.state.discard_cell_draft(); assert_eq!(demo.state.edit_value,"1430");
}

#[test]
fn toolbar_preferences_scroll_restore_open_choices_and_cancel_without_saving() {
    let mut demo = Demo::new(1400,860,PresentBackend::Softbuffer);
    let saved = demo.state.prefs.toolbar_styles;
    let folder = std::env::var("NEXT_GUI_TOOLBAR_STYLES_QA_OUT").ok().map(PathBuf::from);
    for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        demo.theme = ui_build::browser_theme(theme,demo.state.prefs.font_size);
        demo.win_w=1400; demo.win_h=860; demo.state.sync_prefs_drafts(); demo.state.overlay=Some(OverlayKind::Preferences); demo.do_rebuild();
        for (index,id) in demo.ids.ov_prefs_toolbars.into_iter().enumerate() {
            assert!(demo.ui.accessible_name(id).unwrap().contains(crate::prefs::TOOLBAR_GROUPS[index]));
            demo.ui.set_select_selected(id,Some(index));
        }
        let id=demo.ids.ov_prefs_toolbars[4]; demo.ui.set_focus(Some(id)); assert!(demo.ui.key_input(UiKey::Space));
        demo.ui.sync(&mut demo.scene); assert_eq!(demo.ui.popup_owner(),Some(id)); demo.snapshot_inputs();
        assert_eq!(demo.state.prefs_toolbar_styles,[0,1,2,3,4]); assert_eq!(demo.state.prefs.toolbar_styles,saved);
        demo.win_w=800; demo.win_h=600; demo.do_rebuild(); demo.ui.sync(&mut demo.scene);
        let id=demo.ids.ov_prefs_toolbars[4]; assert_eq!(demo.ui.focused(),Some(id)); assert_eq!(demo.ui.popup_owner(),Some(id));
        let rect=bounds(&demo,id); assert_eq!(demo.ui.hit_widget(&demo.scene,rect.x+rect.width*0.5,rect.y+rect.height*0.5),Some(id));
        assert!(demo.ui.scroll_offset(demo.ids.ov_scroll).unwrap().1>0.0);
        demo.snapshot_inputs(); let scroll=demo.state.prefs_scroll; demo.do_rebuild();
        assert_eq!(demo.ui.scroll_offset(demo.ids.ov_scroll),Some(scroll));
        assert_eq!(demo.ui.popup_owner(),Some(demo.ids.ov_prefs_toolbars[4]));
        let popup_style=demo.ui.select_style();
        let popup=demo.scene.iter_paint_order().into_iter().find(|node| node.visible && node.fill==popup_style.popup_bg
            && demo.ui.widget_from_node(node.id)==Some(demo.ids.ov_prefs_toolbars[4]) && node.bounds.height >= popup_style.item_height*5.0)
            .expect("The restored popup must be painted before further input").bounds;
        assert!(popup.x>=0.0 && popup.y>=0.0 && popup.x+popup.width<=800.0 && popup.y+popup.height<=600.0);
        if let Some(folder)=&folder {
            std::fs::create_dir_all(folder).unwrap();
            demo.scene.mark_full_dirty(demo.win_w as f32,demo.win_h as f32); let dirty=demo.scene.take_dirty();
            let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
            let mut soft=SoftRenderer::new(demo.win_w,demo.win_h,clear); soft.render(&commands,&dirty);
            write_ppm(folder.join(format!("window-toolbar-prefs-{name}-800.ppm")).to_str().unwrap(),soft.frame_pixels().unwrap().0,demo.win_w,demo.win_h).unwrap();
            if let Ok(mut gpu)=GpuRenderer::try_new(demo.win_w,demo.win_h,clear) {
                gpu.render(&commands,&dirty);
                write_ppm(folder.join(format!("window-toolbar-prefs-{name}-800-metal.ppm")).to_str().unwrap(),gpu.frame_pixels().unwrap().0,demo.win_w,demo.win_h).unwrap();
            }
        }
        assert!(demo.ui.key_input(UiKey::Escape)); demo.handle_button(demo.ids.ov_btn_cancel); demo.do_rebuild();
        assert_eq!(demo.state.prefs.toolbar_styles,saved); assert!(demo.state.overlay.is_none());
    }
}
