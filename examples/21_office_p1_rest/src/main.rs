//! Office P1 rest demo: Drawer, VirtualList/Table, TextArea, InputNumber,
//! Notification, Spin, Empty, Collapse, Descriptions, Card, Segmented,
//! Breadcrumb, Pagination (+ Backdrop via Modal/Drawer scrim).
//!
//! D = open Drawer; N = toast; Space Soft↔GPU when idle. Esc closes Drawer.
//! `NEXT_GUI_P1_REST_QA=1` seeds Drawer open + toast and dumps a PPM after paint.

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
    BreadcrumbStyle, CardStyle, CollapsePanel, CollapseStyle, ColumnStyle, DescriptionItem,
    DescriptionsStyle, DrawerPlacement, DrawerStyle, EmptyStyle, InputNumberStyle, ListStyle,
    NotificationTone, PaginationStyle, PanelStyle, RowStyle, SegmentedStyle, SpinStyle,
    StatusBarStyle, TableColumn, TableStyle, TextAreaStyle, ToolbarStyle, Ui, UiKey, WidgetId,
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
        KeyCode::Left => UiKey::Left,
        KeyCode::Right => UiKey::Right,
        KeyCode::Up => UiKey::Up,
        KeyCode::Down => UiKey::Down,
        KeyCode::Home => UiKey::Home,
        KeyCode::End => UiKey::End,
        KeyCode::Enter => UiKey::Enter,
        KeyCode::Space => UiKey::Space,
        KeyCode::Escape => UiKey::Escape,
        KeyCode::Char(c) => UiKey::Char(c),
        KeyCode::Other(_) => return None,
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
    drawer: WidgetId,
    open_btn: WidgetId,
    toast_btn: WidgetId,
    vlist: WidgetId,
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
        let mut open_btn = WidgetId(0);
        let mut toast_btn = WidgetId(0);
        let mut vlist = WidgetId(0);
        let mut status = WidgetId(0);
        let mut log = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(960.0);
        let shell_h = (height as f32 - 24.0).max(680.0);
        ui.set_viewport(width as f32, height as f32);

        let points: Vec<String> = (0..8000).map(|i| format!("Point-{i:04}")).collect();
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("{i}"), format!("Tag-{i}"), "OK".into()])
            .collect();

        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 12.0,
                width: shell_w,
                height: shell_h,
                padding: 8.0,
                gap: 8.0,
                background: Some(Color::rgb(0x1e, 0x1e, 0x22)),
                ..ColumnStyle::default()
            },
            |col| {
                col.toolbar(
                    ToolbarStyle {
                        width: shell_w - 16.0,
                        ..ToolbarStyle::default()
                    },
                    |tb| {
                        tb.text_styled(
                            "Office P1 rest — Drawer / Virtual / Entry / Feedback / Structure",
                            Color::rgb(0xa0, 0xa8, 0xb4),
                            12.0,
                        );
                        tb.spacer(12.0, 1.0);
                        open_btn = tb.tool_button("Open Drawer");
                        toast_btn = tb.tool_button("Notify");
                    },
                );

                col.breadcrumb(
                    ["Plant North", "Bay-03", "Meter-12"],
                    BreadcrumbStyle {
                        width: shell_w - 16.0,
                        ..BreadcrumbStyle::default()
                    },
                );

                col.segmented(
                    ["Table", "Chart", "Log"],
                    0,
                    SegmentedStyle {
                        width: 280.0,
                        ..SegmentedStyle::default()
                    },
                );

                col.row(
                    RowStyle {
                        width: shell_w - 16.0,
                        height: 150.0,
                        gap: 10.0,
                        cross_align: CrossAlign::Start,
                        ..RowStyle::default()
                    },
                    |r| {
                        r.panel(
                            "VirtualList (8k)",
                            PanelStyle {
                                width: 280.0,
                                height: 148.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                vlist = p.virtual_list(
                                    points.clone(),
                                    ListStyle {
                                        width: 256.0,
                                        height: 110.0,
                                        ..ListStyle::default()
                                    },
                                );
                            },
                        );
                        r.panel(
                            "VirtualTable (3k)",
                            PanelStyle {
                                width: 320.0,
                                height: 148.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.virtual_table(
                                    vec![
                                        TableColumn::new("Id", 48.0),
                                        TableColumn::new("Name", 120.0),
                                        TableColumn::new("St", 40.0),
                                    ],
                                    rows.clone(),
                                    TableStyle {
                                        width: 292.0,
                                        height: 110.0,
                                        ..TableStyle::default()
                                    },
                                );
                            },
                        );
                        r.panel(
                            "Entry",
                            PanelStyle {
                                width: 300.0,
                                height: 148.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_area(
                                    "line1\nline2",
                                    "notes…",
                                    TextAreaStyle {
                                        width: 270.0,
                                        height: 64.0,
                                        ..TextAreaStyle::default()
                                    },
                                );
                                p.input_number(36.5, 0.0, 100.0, 0.5, InputNumberStyle::default());
                            },
                        );
                    },
                );

                col.row(
                    RowStyle {
                        width: shell_w - 16.0,
                        height: 170.0,
                        gap: 10.0,
                        ..RowStyle::default()
                    },
                    |r| {
                        r.card(
                            "Card",
                            "Bay voltage within band.",
                            CardStyle {
                                width: 220.0,
                                height: 120.0,
                                ..CardStyle::default()
                            },
                        );
                        r.empty(
                            "No selection",
                            EmptyStyle {
                                width: 160.0,
                                height: 120.0,
                                ..EmptyStyle::default()
                            },
                        );
                        r.collapse(
                            vec![
                                CollapsePanel::new("Limits", "Hi=80 Lo=20", true),
                                CollapsePanel::new("Comm", "Modbus RTU", false),
                            ],
                            CollapseStyle {
                                width: 220.0,
                                ..CollapseStyle::default()
                            },
                        );
                        r.descriptions(
                            vec![
                                DescriptionItem::new("Plant", "North"),
                                DescriptionItem::new("Bay", "03"),
                                DescriptionItem::new("Mode", "Auto"),
                            ],
                            DescriptionsStyle {
                                width: 220.0,
                                ..DescriptionsStyle::default()
                            },
                        );
                        r.spin(
                            true,
                            SpinStyle {
                                width: 140.0,
                                height: 120.0,
                                ..SpinStyle::default()
                            },
                            |s| {
                                s.text_styled("Panel", Color::rgb(0xa0, 0xa8, 0xb4), 12.0);
                            },
                        );
                    },
                );

                col.pagination(
                    1,
                    50,
                    8000,
                    PaginationStyle {
                        width: 360.0,
                        ..PaginationStyle::default()
                    },
                );

                log = col.text_styled(
                    "D Drawer · N Notify · scroll virtual lists · Esc closes drawer",
                    Color::rgb(0xc0, 0xc8, 0xd0),
                    13.0,
                );

                col.status_bar(
                    StatusBarStyle {
                        width: shell_w - 16.0,
                        ..StatusBarStyle::default()
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

        let drawer = ui.drawer(
            "Bay config",
            "Edit thresholds, mapping, and notes for the selected point.",
            DrawerStyle {
                placement: DrawerPlacement::Right,
                width: 340.0,
                ..DrawerStyle::default()
            },
        );

        if std::env::var_os("NEXT_GUI_P1_REST_QA").is_some() {
            let _ = ui.set_drawer_open(drawer, true);
            ui.notify("QA: drawer + toast seeded", NotificationTone::Info);
            ui.notify("Saved thresholds", NotificationTone::Success);
        }

        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);

        Self {
            scene,
            ui,
            backend: Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
            gpu_stash: None,
            surface_gpu: None,
            using_gpu: false,
            window_backend,
            drawer,
            open_btn,
            toast_btn,
            vlist,
            status,
            log,
            needs_paint: true,
            pending_native: matches!(window_backend, PresentBackend::GpuSurface),
            qa_dumps: 0,
        }
    }

    fn refresh_status(&mut self) {
        let open = self.ui.drawer_open(self.drawer).unwrap_or(false);
        let sel = self.ui.list_selected(self.vlist);
        self.ui.set_text(
            self.log,
            format!(
                "drawer={}  vlist_sel={:?}  backend={}",
                if open { "open" } else { "closed" },
                sel,
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


    fn apply_button(&mut self, id: next_gui_ui::WidgetId) -> bool {
        if id == self.open_btn {
            let _ = self.ui.set_drawer_open(self.drawer, true);
            true
        } else if id == self.toast_btn {
            self.ui.notify("Thresholds saved", next_gui_ui::NotificationTone::Success);
            true
        } else {
            false
        }
    }

    fn handle_actions(&mut self) {
        if let Some((id, _)) = self.ui.take_drawer_action() {
            self.ui
                .set_text(self.log, format!("Drawer dismissed ({})", id.raw()));
        }
        if let Some((id, idx)) = self.ui.take_breadcrumb_click() {
            self.ui
                .set_text(self.log, format!("Breadcrumb {idx} on {}", id.raw()));
        }
        if let Some((id, page)) = self.ui.take_pagination_change() {
            self.ui
                .set_text(self.log, format!("Page {page} on {}", id.raw()));
        }
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
        if std::env::var_os("NEXT_GUI_P1_REST_QA").is_none() {
            return;
        }
        if self.qa_dumps >= 1 {
            return;
        }
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tmp/demo-shots/21-office_p1_rest.ppm");
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
            // Stay Soft for stable QA screenshots unless user toggles.
            eprintln!("e21_office_p1_rest: Soft primary (Space toggles GPU if available)");
            let _ = GpuRenderer::try_new(w, h, CLEAR).map(|gpu| {
                self.gpu_stash = Some(gpu);
            });
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!("next-gui Office P1 rest — {}", self.backend.name()));
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
                let xf = x as f32;
                let yf = y as f32;
                let mut handled = self.ui.pointer_press(&mut self.scene, xf, yf, false);
                if let Some(hit) = self.ui.hit_widget(&self.scene, xf, yf) {
                    if self.apply_button(hit) {
                        handled = true;
                    }
                }
                if handled {
                    self.handle_actions();
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
                    // Char shortcuts when no text focus
                    if self.ui.focused().is_none() {
                        if matches!(ui_key, UiKey::Char('d') | UiKey::Char('D')) {
                            let _ = self.ui.set_drawer_open(self.drawer, true);
                            self.refresh_status();
                            ctx.request_redraw();
                            return;
                        }
                        if matches!(ui_key, UiKey::Char('n') | UiKey::Char('N')) {
                            self.ui.notify("Manual toast", NotificationTone::Warning);
                            self.refresh_status();
                            ctx.request_redraw();
                            return;
                        }
                    }
                    if self.ui.key_input(ui_key) {
                        if let Some(id) = self.ui.take_button_activate() {
                            let _ = self.apply_button(id);
                        }
                        self.handle_actions();
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
                            "next-gui Office P1 rest — {}",
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
        WindowAttrs::new("next-gui Office P1 rest", 1100, 780).with_gpu_surface()
    } else {
        WindowAttrs::new("next-gui Office P1 rest", 1100, 780)
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
