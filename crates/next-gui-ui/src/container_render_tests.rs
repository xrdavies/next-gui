use super::*;
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_soft::SoftRenderer;

#[test]
fn transparent_nested_containers_preserve_the_surface_behind_rounded_tables() {
    let surface = Color::rgb(50, 90, 120);
    let body = Color::rgb(180, 190, 200);
    let font = load_default_cjk_font().unwrap();
    for background in [None, Some(Color::rgb(90, 110, 130))] {
        for kind in [
            "column", "row", "toolbar", "status", "panel", "grid", "form",
        ] {
            let mut ui = Ui::with_font(font.clone());
            ui.set_theme(Theme::light());
            let mut scene = Scene::new();
            let mut table = WidgetId(0);
            ui.column(
                ColumnStyle {
                    width: 200.0,
                    height: 180.0,
                    padding: 0.0,
                    background: Some(surface),
                    ..ColumnStyle::default()
                },
                |outer| {
                    let build = |inner: &mut FlexBuilder<'_>| {
                        table = inner.table(
                            vec![TableColumn::new("Column", 80.0)],
                            Vec::new(),
                            TableStyle {
                                width: 120.0,
                                height: 70.0,
                                bg: body,
                                ..TableStyle::default()
                            },
                        );
                    };
                    match kind {
                        "column" => outer.column(
                            ColumnStyle {
                                width: 160.0,
                                height: 140.0,
                                padding: 10.0,
                                background,
                                ..ColumnStyle::default()
                            },
                            build,
                        ),
                        "row" => outer.row(
                            RowStyle {
                                width: 160.0,
                                height: 140.0,
                                padding: 10.0,
                                background,
                                ..RowStyle::default()
                            },
                            build,
                        ),
                        "toolbar" => outer.toolbar(
                            ToolbarStyle {
                                width: 160.0,
                                height: 140.0,
                                padding: 10.0,
                                background,
                                ..ToolbarStyle::default()
                            },
                            build,
                        ),
                        "status" => outer.status_bar(
                            StatusBarStyle {
                                width: 160.0,
                                height: 140.0,
                                padding: 10.0,
                                background,
                                ..StatusBarStyle::default()
                            },
                            build,
                        ),
                        "panel" => outer.panel(
                            "Panel",
                            PanelStyle {
                                width: 160.0,
                                height: 140.0,
                                padding: 10.0,
                                background,
                                ..PanelStyle::default()
                            },
                            build,
                        ),
                        "grid" => outer.grid(
                            1,
                            GridStyle {
                                width: 160.0,
                                height: 140.0,
                                padding: 10.0,
                                background,
                                ..GridStyle::default()
                            },
                            build,
                        ),
                        "form" => outer.form(
                            FormStyle {
                                width: 160.0,
                                height: 140.0,
                                padding: 10.0,
                                background,
                                ..FormStyle::default()
                            },
                            |form| {
                                form.field("data", "", Vec::new(), build);
                            },
                        ),
                        _ => unreachable!(),
                    };
                },
            );
            let mut renderer = SoftRenderer::new(200, 180, Color::rgb(255, 0, 0));
            for repaint in 0..2 {
                ui.sync(&mut scene);
                let bounds = scene.get(ui.widgets[&table].node.unwrap()).unwrap().bounds;
                scene.take_dirty();
                if repaint == 0 {
                    scene.mark_full_dirty(200.0, 180.0);
                } else {
                    scene.mark_dirty(Rect::new(
                        bounds.x,
                        bounds.y + bounds.height - 4.0,
                        bounds.width,
                        4.0,
                    ));
                }
                let dirty = scene.take_dirty();
                renderer.render(&encode_scene(&scene, &dirty), &dirty);
                let x = bounds.x.floor() as u32;
                let y = (bounds.y + bounds.height).floor() as u32 - 1;
                assert_eq!(
                    renderer.pixel(x, y),
                    Some(background.unwrap_or(surface).to_xrgb8888()),
                    "{kind}: bottom corner must expose the parent surface"
                );
                assert_eq!(
                    renderer.pixel(x + bounds.width as u32 / 2, y - 3),
                    Some(body.to_xrgb8888()),
                    "{kind}: child table must still paint"
                );
                assert_eq!(
                    ui.hit_widget(&scene, bounds.x + 20.0, bounds.y + 10.0),
                    Some(table),
                    "{kind}: child hit targets must remain active"
                );
            }
        }
    }
}
