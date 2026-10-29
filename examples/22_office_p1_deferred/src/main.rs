//! Office P1 deferred demo: DateTime, Transfer, PropertyGrid, TreeTable.
//!
//! Click DateTime to open picker; Transfer buttons move items; PropertyGrid
//! value clicks toggle/cycle; TreeTable expand + select. Space Soft↔GPU when idle.
//! `NEXT_GUI_P1_DEFERRED_QA=1` opens DateTime popup and dumps a PPM after paint.

use next_gui_layout::CrossAlign;
use next_gui_platform::{
    AppCallbacks, Event, KeyCode, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{
    ColumnStyle, DateTimeStyle, DateTimeValue, PanelStyle, PropertyGridStyle, PropertyRow,
    PropertyValue, RowStyle, StatusBarStyle, TableColumn, ToolbarStyle, TransferItem,
    TransferStyle, TreeTableNode, TreeTableStyle, Ui, UiKey, WidgetId,
};

const CLEAR: Color = Color {
    r: 0x1a,
    g: 0x1a,
    b: 0x1c,
    a: 255,
};

enum Backend {
    Soft(SoftRenderer),
    Gpu(GpuRenderer),
}

impl Backend {
    fn name(&self) -> &str {
        match self {
            Backend::Soft(_) => "Soft",
            Backend::Gpu(_) => "GPU",
        }
    }

    fn detail(&self) -> String {
        match self {
            Backend::Soft(_) => "CPU soft".into(),
            Backend::Gpu(g) => {
                let path = match g.present_path() {
                    GpuPresentPath::ReadbackSoftbuffer => "readback→softbuffer",
                    GpuPresentPath::NativeSurface => "native surface",
                };
                format!("wgpu {} ({})", g.backend_name(), path)
            }
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        match self {
            Backend::Soft(s) => s.resize(w, h),
            Backend::Gpu(g) => g.resize(w, h),
        }
    }

    fn render(
        &mut self,
        cmds: &[next_gui_render::DrawCommand],
        dirty: &next_gui_scene::DirtyRegion,
    ) {
        match self {
            Backend::Soft(s) => s.render(cmds, dirty),
            Backend::Gpu(g) => g.render(cmds, dirty),
        }
    }

    fn frame_pixels(&self) -> Option<(&[u32], u32, u32)> {
        match self {
            Backend::Soft(s) => s.frame_pixels(),
            Backend::Gpu(g) => g.frame_pixels(),
        }
    }
}

fn map_key(key: KeyCode, modifiers: next_gui_platform::Modifiers) -> Option<UiKey> {
    if modifiers.commandish() {
        if let KeyCode::Char(c) = key {
            if c == 'a' || c == 'A' {
                return Some(UiKey::SelectAll);
            }
        }
    }
    // P6d: Shift+Tab → BackTab (focus_prev).
    if matches!(key, KeyCode::Tab) {
        return Some(if modifiers.shift {
            UiKey::BackTab
        } else {
            UiKey::Tab
        });
    }
    Some(match key {
        KeyCode::Backspace => UiKey::Backspace,
        KeyCode::Delete => UiKey::Delete,
        KeyCode::Insert => UiKey::Insert,
        KeyCode::Left => UiKey::Left,
        KeyCode::Right => UiKey::Right,
        KeyCode::Up => UiKey::Up,
        KeyCode::Down => UiKey::Down,
        KeyCode::Home => UiKey::Home,
        KeyCode::End => UiKey::End,
        KeyCode::PageUp => UiKey::PageUp,
        KeyCode::PageDown => UiKey::PageDown,
        KeyCode::Enter => UiKey::Enter,
        KeyCode::Space => UiKey::Space,
        KeyCode::Escape => UiKey::Escape,
        KeyCode::Char(c) => UiKey::Char(c),
        KeyCode::Function(_) | KeyCode::Other(_) => return None,
        KeyCode::Tab => unreachable!("Tab mapped above"),
    })
}

fn write_ppm(path: &str, pixels: &[u32], w: u32, h: u32) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "P6\n{w} {h}\n255")?;
    for px in pixels {
        let r = ((px >> 16) & 0xff) as u8;
        let g = ((px >> 8) & 0xff) as u8;
        let b = (px & 0xff) as u8;
        f.write_all(&[r, g, b])?;
    }
    Ok(())
}

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    datetime: WidgetId,
    transfer: WidgetId,
    props: WidgetId,
    tree_table: WidgetId,
    status: WidgetId,
    log: WidgetId,
    needs_paint: bool,
    pending_native: bool,
    qa_dumps: u32,
}

impl Demo {
    fn new_soft(width: u32, height: u32, window_backend: PresentBackend) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });

        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font);
        let theme = *ui.theme();
        let mut datetime = WidgetId(0);
        let mut transfer = WidgetId(0);
        let mut props = WidgetId(0);
        let mut tree_table = WidgetId(0);
        let mut status = WidgetId(0);
        let mut log = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(1000.0);
        let shell_h = (height as f32 - 24.0).max(720.0);
        ui.set_viewport(width as f32, height as f32);

        let source: Vec<TransferItem> = (1..=12)
            .map(|i| TransferItem::new(i, format!("Point-{i:02}")))
            .collect();
        let target = vec![
            TransferItem::new(101, "Point-A"),
            TransferItem::new(102, "Point-B"),
        ];

        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 12.0,
                width: shell_w,
                height: shell_h,
                padding: 8.0,
                gap: 8.0,
                background: Some(Color::rgb(0x1e, 0x1e, 0x22)),
                ..ColumnStyle::from_theme(&theme)
            },
            |col| {
                col.toolbar(
                    ToolbarStyle {
                        width: shell_w - 16.0,
                        ..ToolbarStyle::from_theme(&theme)
                    },
                    |tb| {
                        tb.text_styled(
                            "Office P1 deferred — DateTime / Transfer / PropertyGrid / TreeTable",
                            Color::rgb(0xa0, 0xa8, 0xb4),
                            12.0,
                        );
                    },
                );

                col.row(
                    RowStyle {
                        width: shell_w - 16.0,
                        height: 300.0,
                        gap: 10.0,
                        cross_align: CrossAlign::Start,
                        ..RowStyle::from_theme(&theme)
                    },
                    |r| {
                        r.panel(
                            "DateTime + PropertyGrid",
                            PanelStyle {
                                width: 360.0,
                                height: 290.0,
                                ..PanelStyle::from_theme(&theme)
                            },
                            |p| {
                                p.text_styled("Log from", Color::rgb(0x90, 0x98, 0xa4), 12.0);
                                datetime = p.datetime(
                                    DateTimeValue::new(2026, 10, 18, 8, 30),
                                    DateTimeStyle {
                                        width: 220.0,
                                        ..DateTimeStyle::from_theme(&theme)
                                    },
                                );
                                p.spacer(4.0, 8.0);
                                props = p.property_grid(
                                    vec![
                                        PropertyRow::new(
                                            "Name",
                                            PropertyValue::Text("Bay-03".into()),
                                        ),
                                        PropertyRow::new("Voltage", PropertyValue::Number(10.5)),
                                        PropertyRow::new("Enabled", PropertyValue::Bool(true)),
                                        PropertyRow::new(
                                            "Mode",
                                            PropertyValue::Select {
                                                options: vec![
                                                    "Auto".into(),
                                                    "Manual".into(),
                                                    "Off".into(),
                                                ],
                                                selected: 0,
                                            },
                                        ),
                                    ],
                                    PropertyGridStyle {
                                        width: 320.0,
                                        height: 160.0,
                                        ..PropertyGridStyle::from_theme(&theme)
                                    },
                                );
                            },
                        );
                        r.panel(
                            "Transfer",
                            PanelStyle {
                                width: 560.0,
                                height: 290.0,
                                ..PanelStyle::from_theme(&theme)
                            },
                            |p| {
                                transfer = p.transfer(
                                    source.clone(),
                                    target.clone(),
                                    TransferStyle {
                                        width: 520.0,
                                        height: 240.0,
                                        ..TransferStyle::from_theme(&theme)
                                    },
                                );
                            },
                        );
                    },
                );

                col.panel(
                    "TreeTable (device points)",
                    PanelStyle {
                        width: shell_w - 16.0,
                        height: 320.0,
                        ..PanelStyle::from_theme(&theme)
                    },
                    |p| {
                        tree_table = p.tree_table(
                            vec![
                                TableColumn::new("Type", 100.0),
                                TableColumn::new("Value", 80.0),
                                TableColumn::new("Quality", 80.0),
                            ],
                            vec![TreeTableNode::new(
                                1,
                                "Plant North",
                                vec!["Plant".into(), "—".into(), "OK".into()],
                                vec![
                                    TreeTableNode::new(
                                        2,
                                        "Bay-01",
                                        vec!["Bay".into(), "—".into(), "OK".into()],
                                        vec![
                                            TreeTableNode::leaf(
                                                3,
                                                "Ua",
                                                vec!["AI".into(), "10.2".into(), "Good".into()],
                                            ),
                                            TreeTableNode::leaf(
                                                4,
                                                "Ub",
                                                vec!["AI".into(), "10.1".into(), "Good".into()],
                                            ),
                                        ],
                                    ),
                                    TreeTableNode::new(
                                        5,
                                        "Bay-02",
                                        vec!["Bay".into(), "—".into(), "OK".into()],
                                        vec![TreeTableNode::leaf(
                                            6,
                                            "CB-1",
                                            vec!["DI".into(), "1".into(), "Good".into()],
                                        )],
                                    ),
                                ],
                            )],
                            TreeTableStyle {
                                width: shell_w - 48.0,
                                height: 260.0,
                                tree_col_width: 180.0,
                                ..TreeTableStyle::from_theme(&theme)
                            },
                        );
                    },
                );

                log = col.text_styled(
                    "Click DateTime / transfer / props / tree · Esc closes picker",
                    Color::rgb(0xc0, 0xc8, 0xd0),
                    13.0,
                );

                col.status_bar(
                    StatusBarStyle {
                        width: shell_w - 16.0,
                        ..StatusBarStyle::from_theme(&theme)
                    },
                    |sb| {
                        status = sb.text_styled("Soft", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
                        sb.spacer(16.0, 1.0);
                        sb.text_styled(
                            "Space Soft↔GPU when idle",
                            Color::rgb(0x80, 0x88, 0x94),
                            12.0,
                        );
                    },
                );
            },
        );

        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);

        if std::env::var_os("NEXT_GUI_P1_DEFERRED_QA").is_some() {
            let _ = ui.set_datetime_open(datetime, true);
            ui.sync(&mut scene);
            scene.mark_full_dirty(width as f32, height as f32);
        }

        Self {
            scene,
            ui,
            backend: Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
            gpu_stash: None,
            surface_gpu: None,
            using_gpu: false,
            window_backend,
            datetime,
            transfer,
            props,
            tree_table,
            status,
            log,
            needs_paint: true,
            pending_native: matches!(window_backend, PresentBackend::GpuSurface),
            qa_dumps: 0,
        }
    }

    fn refresh_status(&mut self) {
        let dt = self
            .ui
            .datetime_value(self.datetime)
            .map(|v| v.format_display())
            .unwrap_or_else(|| "?".into());
        let open = self.ui.datetime_open(self.datetime).unwrap_or(false);
        let tgt = self
            .ui
            .transfer_target(self.transfer)
            .map(|t| t.len())
            .unwrap_or(0);
        let psel = self.ui.property_grid_selected(self.props);
        let tsel = self.ui.tree_table_selected(self.tree_table);
        self.ui.set_text(
            self.log,
            format!(
                "dt={dt} open={open} transfer_tgt={tgt} prop_sel={psel:?} tree_sel={tsel:?} [{}]",
                self.backend.name()
            ),
        );
        self.ui.set_text(
            self.status,
            format!("{} ({})", self.backend.name(), self.backend.detail()),
        );
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    fn toggle(&mut self, w: u32, h: u32) {
        if !self.using_gpu {
            match self.gpu_stash.take() {
                Some(mut gpu) => {
                    gpu.resize(w, h);
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                None => match GpuRenderer::try_new(w, h, CLEAR) {
                    Ok(gpu) => {
                        self.backend = Backend::Gpu(gpu);
                        self.using_gpu = true;
                    }
                    Err(e) => {
                        eprintln!("toggle: GPU unavailable: {e}");
                        return;
                    }
                },
            }
        } else if let Backend::Gpu(gpu) = std::mem::replace(
            &mut self.backend,
            Backend::Soft(SoftRenderer::new(w, h, CLEAR)),
        ) {
            self.gpu_stash = Some(gpu);
            self.using_gpu = false;
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
    }

    fn present_frame(&mut self, ctx: &mut WindowCtx<'_>) {
        match &mut self.backend {
            Backend::Gpu(g) => {
                if g.present_path() == GpuPresentPath::NativeSurface {
                    return;
                }
                if let Some((pixels, w, h)) = g.frame_pixels() {
                    let _ = ctx.present(w, h, pixels);
                }
            }
            Backend::Soft(s) => {
                let Some((pixels, w, h)) = s.frame_pixels() else {
                    return;
                };
                let pixels = pixels.to_vec();
                match self.window_backend {
                    PresentBackend::Softbuffer => {
                        let _ = ctx.present(w, h, &pixels);
                    }
                    PresentBackend::GpuSurface => {
                        if let Some(gpu) = self.surface_gpu.as_mut() {
                            let _ = gpu.present_xrgb_frame(w, h, &pixels);
                        }
                    }
                }
            }
        }
    }

    fn maybe_qa_dump(&mut self) {
        if std::env::var_os("NEXT_GUI_P1_DEFERRED_QA").is_none() {
            return;
        }
        if self.qa_dumps >= 1 {
            return;
        }
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tmp/demo-shots/22-office_p1_deferred.ppm");
            if let Err(e) = write_ppm(path, pixels, w, h) {
                eprintln!("QA dump failed: {e}");
            } else {
                eprintln!("QA wrote {path} ({w}x{h})");
                self.qa_dumps += 1;
            }
        }
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        if self.pending_native {
            self.pending_native = false;
            let window = ctx.winit_window();
            match GpuRenderer::try_new_native(window, w, h, CLEAR) {
                Ok(gpu) => {
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            eprintln!("e22_office_p1_deferred: Soft primary (Space toggles GPU if available)");
            let _ = GpuRenderer::try_new(w, h, CLEAR).map(|gpu| {
                self.gpu_stash = Some(gpu);
            });
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!(
            "next-gui Office P1 deferred — {}",
            self.backend.name()
        ));
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.backend.resize(width, height);
                if let Some(g) = self.gpu_stash.as_mut() {
                    g.resize(width, height);
                }
                if let Some(g) = self.surface_gpu.as_mut() {
                    g.resize(width, height);
                }
                self.ui.set_viewport(width as f32, height as f32);
                self.ui.sync(&mut self.scene);
                self.scene.mark_full_dirty(width as f32, height as f32);
                self.needs_paint = true;
                ctx.request_redraw();
            }
            Event::PointerButton {
                button: PointerButton::Left,
                pressed: true,
                x,
                y,
                ..
            } => {
                if self.ui.pointer_press(&mut self.scene, x as f32, y as f32, false) {
                    self.refresh_status();
                    ctx.request_redraw();
                }
            }
            Event::PointerButton {
                button: PointerButton::Left,
                pressed: false,
                ..
            } => {
                if self.ui.is_dragging() {
                    self.ui.pointer_released();
                    self.refresh_status();
                    ctx.request_redraw();
                }
            }
            Event::PointerMoved { x, y, .. } => {
                if self.ui.pointer_moved(x as f32, y as f32) {
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::PointerScroll {
                x,
                y,
                delta_x,
                delta_y,
                ..
            } => {
                if self.ui.pointer_scroll(
                    &self.scene,
                    x as f32,
                    y as f32,
                    delta_x as f32,
                    delta_y as f32,
                ) {
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                if let Some(ui_key) = map_key(key, modifiers) {
                    if self.ui.key_input(ui_key) {
                        self.refresh_status();
                        ctx.request_redraw();
                        return;
                    }
                    if matches!(ui_key, UiKey::Escape) {
                        ctx.request_exit();
                        return;
                    }
                    if matches!(ui_key, UiKey::Space) && self.ui.focused().is_none() {
                        let (w, h) = ctx.size();
                        self.toggle(w, h);
                        ctx.set_title(&format!(
                            "next-gui Office P1 deferred — {}",
                            self.backend.name()
                        ));
                        ctx.request_redraw();
                    }
                }
            }
            Event::RedrawRequested { .. } => {
                self.needs_paint = true;
            }
            _ => {}
        }
    }

    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
        if !self.needs_paint {
            return;
        }
        let dirty = self.scene.take_dirty();
        if dirty.is_empty() {
            self.needs_paint = false;
            return;
        }
        let cmds = encode_scene(&self.scene, &dirty);
        self.backend.render(&cmds, &dirty);
        self.present_frame(ctx);
        self.maybe_qa_dump();
        self.needs_paint = false;
    }
}

fn main() {
    let use_native = std::env::var_os("NEXT_GUI_GPU_SURFACE").is_some()
        && GpuRenderer::adapter_available();
    let attrs = if use_native {
        WindowAttrs::new("next-gui Office P1 deferred", 1120, 820).with_gpu_surface()
    } else {
        WindowAttrs::new("next-gui Office P1 deferred", 1120, 820)
    };
    let demo = Demo::new_soft(attrs.width, attrs.height, attrs.present_backend);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed: {e}");
        std::process::exit(1);
    }
}
