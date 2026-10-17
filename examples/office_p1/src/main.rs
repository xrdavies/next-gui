//! Office P1 demo: TreeSelect, Form validation, AlarmList, Gauge, Trend.
//!
//! Open TreeSelect to pick a bay; Validate form; click alarm then Enter/Space to ack;
//! Gauge/Trend animate on redraw. Space toggles Soft↔GPU when idle. Esc closes popup.

use std::time::Instant;

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
    AlarmEntry, AlarmListStyle, AlarmSeverity, ColumnStyle, FormRule, FormStyle, GaugeStyle,
    PanelStyle, RowStyle, StatusBarStyle, TextInputStyle, ToolbarStyle, TreeNode, TreeSelectStyle,
    TrendStyle, Ui, UiKey, WidgetId,
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
}

fn map_key(key: KeyCode, modifiers: next_gui_platform::Modifiers) -> Option<UiKey> {
    if modifiers.commandish() {
        if let KeyCode::Char(c) = key {
            if c == 'a' || c == 'A' {
                return Some(UiKey::SelectAll);
            }
        }
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
        KeyCode::Tab => UiKey::Tab,
        KeyCode::Space => UiKey::Space,
        KeyCode::Escape => UiKey::Escape,
        KeyCode::Char(c) => UiKey::Char(c),
        KeyCode::Other(_) => return None,
    })
}

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    tree_sel: WidgetId,
    form: WidgetId,
    validate_btn: WidgetId,
    alarms: WidgetId,
    gauge: WidgetId,
    trend: WidgetId,
    status: WidgetId,
    log: WidgetId,
    needs_paint: bool,
    pending_native: bool,
    started: Instant,
    last_tick: Instant,
    flash_on: bool,
}

impl Demo {
    fn new_soft(width: u32, height: u32, window_backend: PresentBackend) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });

        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font);
        let mut tree_sel = WidgetId(0);
        let mut form = WidgetId(0);
        let mut validate_btn = WidgetId(0);
        let mut alarms = WidgetId(0);
        let mut gauge = WidgetId(0);
        let mut trend = WidgetId(0);
        let mut status = WidgetId(0);
        let mut log = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(880.0);
        let shell_h = (height as f32 - 24.0).max(620.0);
        ui.set_viewport(width as f32, height as f32);

        let plant = vec![TreeNode::new(
            1,
            "Plant North",
            vec![
                TreeNode::new(
                    10,
                    "Substation A",
                    vec![
                        TreeNode::leaf(11, "Bay-01"),
                        TreeNode::leaf(12, "Bay-02"),
                        TreeNode::leaf(13, "Bay-03"),
                    ],
                ),
                TreeNode::new(
                    20,
                    "Substation B",
                    vec![TreeNode::leaf(21, "Bay-11"), TreeNode::leaf(22, "Bay-12")],
                ),
            ],
        )];

        let alarm_rows = vec![
            AlarmEntry::new(1, AlarmSeverity::Critical, "Breaker trip", "CB-31", "10:01:02"),
            AlarmEntry::new(2, AlarmSeverity::Major, "Overcurrent", "Feeder-2", "10:01:08"),
            AlarmEntry::new(3, AlarmSeverity::Warning, "High temp", "XFMR-1", "10:02:11"),
            AlarmEntry::new(4, AlarmSeverity::Info, "Comm restored", "GW-1", "10:03:00"),
            AlarmEntry::new(5, AlarmSeverity::Minor, "Tap change", "OLTC-A", "10:03:22"),
        ];

        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 10.0,
                width: shell_w,
                height: shell_h,
                padding: 8.0,
                gap: 8.0,
                cross_align: CrossAlign::Start,
                background: Some(Color::rgb(0x20, 0x20, 0x24)),
            },
            |col| {
                col.toolbar(
                    ToolbarStyle {
                        width: shell_w - 16.0,
                        height: 36.0,
                        ..ToolbarStyle::default()
                    },
                    |tb| {
                        tb.text_styled(
                            "Office P1 — TreeSelect / Form / AlarmList / Gauge / Trend",
                            Color::rgb(0xe0, 0xe4, 0xec),
                            14.0,
                        );
                    },
                );

                col.row(
                    RowStyle {
                        width: shell_w - 16.0,
                        height: shell_h - 90.0,
                        gap: 10.0,
                        padding: 0.0,
                        cross_align: CrossAlign::Start,
                        background: None,
                        ..RowStyle::default()
                    },
                    |row| {
                        row.panel(
                            "Hierarchy pick + Form",
                            PanelStyle {
                                width: 340.0,
                                height: shell_h - 100.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_styled(
                                    "TreeSelect",
                                    Color::rgb(0xa0, 0xa8, 0xb4),
                                    12.0,
                                );
                                tree_sel = p.tree_select(
                                    plant.clone(),
                                    None,
                                    "Select bay…",
                                    TreeSelectStyle {
                                        width: 300.0,
                                        popup_max_height: 180.0,
                                        ..TreeSelectStyle::default()
                                    },
                                );
                                p.spacer(1.0, 8.0);
                                form = p.form(
                                    FormStyle {
                                        width: 310.0,
                                        height: 220.0,
                                        ..FormStyle::default()
                                    },
                                    |f| {
                                        f.field(
                                            "name",
                                            "Device name",
                                            vec![FormRule::required("name is required")],
                                            |b| {
                                                b.text_input_styled(
                                                    "",
                                                    "e.g. Feeder-1",
                                                    TextInputStyle {
                                                        width: 280.0,
                                                        ..TextInputStyle::default()
                                                    },
                                                );
                                            },
                                        );
                                        f.field(
                                            "code",
                                            "Asset code (digits)",
                                            vec![
                                                FormRule::required("code is required"),
                                                FormRule::digits("digits only"),
                                                FormRule::min_length(4, "at least 4 digits"),
                                            ],
                                            |b| {
                                                b.text_input_styled(
                                                    "",
                                                    "e.g. 1001",
                                                    TextInputStyle {
                                                        width: 280.0,
                                                        ..TextInputStyle::default()
                                                    },
                                                );
                                            },
                                        );
                                    },
                                );
                                validate_btn = p.tool_button("Validate form");
                                log = p.text_styled(
                                    "Fill fields · Validate · pick a bay from TreeSelect.",
                                    Color::rgb(0xc0, 0xc8, 0xd0),
                                    12.0,
                                );
                            },
                        );

                        row.column(
                            ColumnStyle {
                                width: (shell_w - 380.0).max(420.0),
                                height: shell_h - 100.0,
                                padding: 0.0,
                                gap: 8.0,
                                background: None,
                                ..ColumnStyle::default()
                            },
                            |c| {
                                c.panel(
                                    "AlarmList",
                                    PanelStyle {
                                        width: (shell_w - 400.0).max(400.0),
                                        height: 240.0,
                                        ..PanelStyle::default()
                                    },
                                    |p| {
                                        p.text_styled(
                                            "Click row · Enter/Space ack · Critical/Major flash",
                                            Color::rgb(0xa0, 0xa8, 0xb4),
                                            12.0,
                                        );
                                        alarms = p.alarm_list(
                                            alarm_rows.clone(),
                                            AlarmListStyle {
                                                width: (shell_w - 440.0).max(360.0),
                                                height: 170.0,
                                                ..AlarmListStyle::default()
                                            },
                                        );
                                    },
                                );
                                c.row(
                                    RowStyle {
                                        width: (shell_w - 400.0).max(400.0),
                                        height: 180.0,
                                        gap: 12.0,
                                        padding: 0.0,
                                        cross_align: CrossAlign::Start,
                                        background: None,
                                        ..RowStyle::default()
                                    },
                                    |r| {
                                        r.panel(
                                            "Gauge",
                                            PanelStyle {
                                                width: 180.0,
                                                height: 170.0,
                                                ..PanelStyle::default()
                                            },
                                            |p| {
                                                gauge = p.gauge(
                                                    62.0,
                                                    0.0,
                                                    100.0,
                                                    "%",
                                                    "Load",
                                                    GaugeStyle::default(),
                                                );
                                            },
                                        );
                                        r.panel(
                                            "Trend",
                                            PanelStyle {
                                                width: (shell_w - 620.0).max(260.0),
                                                height: 170.0,
                                                ..PanelStyle::default()
                                            },
                                            |p| {
                                                trend = p.trend(
                                                    vec![
                                                        40.0, 42.0, 45.0, 44.0, 48.0, 52.0, 50.0,
                                                        55.0, 58.0, 60.0, 62.0, 61.0,
                                                    ],
                                                    TrendStyle {
                                                        width: (shell_w - 660.0).max(220.0),
                                                        height: 110.0,
                                                        ..TrendStyle::default()
                                                    },
                                                );
                                            },
                                        );
                                    },
                                );
                            },
                        );
                    },
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
                            "Esc closes popup · Space Soft↔GPU",
                            Color::rgb(0x80, 0x88, 0x94),
                            12.0,
                        );
                    },
                );
            },
        );

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
            tree_sel,
            form,
            validate_btn,
            alarms,
            gauge,
            trend,
            status,
            log,
            needs_paint: true,
            pending_native: matches!(window_backend, PresentBackend::GpuSurface),
            started: Instant::now(),
            last_tick: Instant::now(),
            flash_on: false,
        }
    }

    fn refresh_status(&mut self) {
        let bay = self
            .ui
            .tree_select_selected(self.tree_sel)
            .map(|id| format!("bay={id}"))
            .unwrap_or_else(|| "bay=—".into());
        let sel = self
            .ui
            .alarm_list_selected(self.alarms)
            .map(|i| format!("alarm#{i}"))
            .unwrap_or_else(|| "alarm=—".into());
        let g = self.ui.gauge_value(self.gauge).unwrap_or(0.0);
        self.ui.set_text(
            self.status,
            format!(
                "{} ({})  {bay}  {sel}  load={g:.0}%",
                self.backend.name(),
                self.backend.detail()
            ),
        );
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    fn tick_live(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_tick).as_millis() < 250 {
            return false;
        }
        self.last_tick = now;
        self.flash_on = !self.flash_on;
        let _ = self.ui.set_alarm_list_flash(self.alarms, self.flash_on);
        let t = now.duration_since(self.started).as_secs_f32();
        let load = 55.0 + 20.0 * (t * 0.7).sin();
        let _ = self.ui.set_gauge_value(self.gauge, load);
        let mut pts = Vec::with_capacity(24);
        for i in 0..24 {
            let x = t * 0.9 + i as f32 * 0.35;
            pts.push(50.0 + 15.0 * x.sin() + 8.0 * (x * 0.4).cos());
        }
        let _ = self.ui.set_trend_points(self.trend, pts);
        true
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
        eprintln!(
            "switched to {} ({})",
            self.backend.name(),
            self.backend.detail()
        );
    }

    fn present_frame(&mut self, ctx: &mut WindowCtx<'_>) {
        match &mut self.backend {
            Backend::Gpu(g) => {
                if g.present_path() == GpuPresentPath::NativeSurface {
                    return;
                }
                if let Some((pixels, w, h)) = g.frame_pixels() {
                    if let Err(e) = ctx.present(w, h, pixels) {
                        eprintln!("present error: {e}");
                    }
                }
            }
            Backend::Soft(s) => {
                let Some((pixels, w, h)) = s.frame_pixels() else {
                    return;
                };
                let pixels = pixels.to_vec();
                match self.window_backend {
                    PresentBackend::Softbuffer => {
                        if let Err(e) = ctx.present(w, h, &pixels) {
                            eprintln!("present error: {e}");
                        }
                    }
                    PresentBackend::GpuSurface => {
                        if let Some(gpu) = self.surface_gpu.as_mut() {
                            if let Err(e) = gpu.present_xrgb_frame(w, h, &pixels) {
                                eprintln!("surface soft blit error: {e}");
                            }
                        }
                    }
                }
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
                    eprintln!(
                        "office_p1: native Surface ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("office_p1: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!("office_p1: GPU readback ready ({})", gpu.backend_name());
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("office_p1: GPU unavailable ({e}); Soft only"),
            }
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!(
            "office_p1 ready {}x{} backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        ctx.set_title(&format!("next-gui Office P1 — {}", self.backend.name()));
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.ui.set_viewport(width as f32, height as f32);
                self.backend.resize(width, height);
                if let Some(g) = self.gpu_stash.as_mut() {
                    g.resize(width, height);
                }
                if let Some(g) = self.surface_gpu.as_mut() {
                    g.resize(width, height);
                }
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
                let handled = self.ui.pointer_press(&mut self.scene, x as f32, y as f32, false);
                if let Some(hit) = self.ui.hit_widget(&self.scene, x as f32, y as f32) {
                    if hit == self.validate_btn {
                        let ok = self.ui.validate_form(self.form);
                        let msg = if ok {
                            "Form OK — all rules passed."
                        } else {
                            "Form has errors — see red hints under fields."
                        };
                        self.ui.set_text(self.log, msg);
                        self.ui.sync(&mut self.scene);
                        self.needs_paint = true;
                        ctx.request_redraw();
                        return;
                    }
                }
                if handled {
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
                    if matches!(ui_key, UiKey::Space) && self.ui.focused().is_none() {
                        let (w, h) = ctx.size();
                        self.toggle(w, h);
                        ctx.set_title(&format!(
                            "next-gui Office P1 — {}",
                            self.backend.name()
                        ));
                        ctx.request_redraw();
                    }
                }
            }
            Event::RedrawRequested { .. } => {
                if self.tick_live() {
                    self.refresh_status();
                }
                self.needs_paint = true;
            }
            _ => {}
        }
    }

    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
        if !self.needs_paint {
            ctx.request_redraw();
            return;
        }
        let dirty = self.scene.take_dirty();
        if dirty.is_empty() {
            self.needs_paint = false;
            ctx.request_redraw();
            return;
        }
        let cmds = encode_scene(&self.scene, &dirty);
        self.backend.render(&cmds, &dirty);
        self.present_frame(ctx);
        self.needs_paint = false;
        // Keep animation / alarm flash alive.
        ctx.request_redraw();
    }
}

fn main() {
    let use_native = std::env::var_os("NEXT_GUI_GPU_SURFACE").is_some()
        && GpuRenderer::adapter_available();
    let attrs = if use_native {
        eprintln!("office_p1: NEXT_GUI_GPU_SURFACE → GpuSurface");
        WindowAttrs::new("next-gui Office P1", 980, 700).with_gpu_surface()
    } else {
        eprintln!(
            "office_p1: Softbuffer window (set NEXT_GUI_GPU_SURFACE=1 for native Surface)"
        );
        WindowAttrs::new("next-gui Office P1", 980, 700)
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
