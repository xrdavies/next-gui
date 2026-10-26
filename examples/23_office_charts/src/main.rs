//! Office charts demo (`e23_office_charts`): multi-tile Chart + standalone Gauge/Trend.
//!
//! Chart hosts 2 gauges + 2 trends in one panel. Values animate on redraw.
//! Space toggles Soft↔GPU when idle. `NEXT_GUI_CHARTS_QA=1` dumps a PPM after paint.

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
    ChartStyle, ChartTile, ColumnStyle, GaugeStyle, PanelStyle, RowStyle, StatusBarStyle,
    ToolbarStyle, TrendStyle, Ui, UiKey, WidgetId,
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

fn sample_series(base: f32, t: f32, n: usize, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = i as f32 * 0.35 + t;
            base + amp * x.sin() + amp * 0.35 * (x * 1.7).cos()
        })
        .collect()
}

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    chart: WidgetId,
    gauge: WidgetId,
    trend: WidgetId,
    status: WidgetId,
    needs_paint: bool,
    pending_native: bool,
    started: Instant,
    last_tick: Instant,
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
        let mut chart = WidgetId(0);
        let mut gauge = WidgetId(0);
        let mut trend = WidgetId(0);
        let mut status = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(1000.0);
        let shell_h = (height as f32 - 24.0).max(700.0);
        ui.set_viewport(width as f32, height as f32);

        let tiles = vec![
            ChartTile::gauge(
                "Load",
                62.0,
                0.0,
                100.0,
                "%",
                "MW",
                GaugeStyle {
                    size: 110.0,
                    dial_fg: Color::rgb(0x3b, 0xc4, 0x8c),
                    ..GaugeStyle::from_theme(&theme)
                },
            ),
            ChartTile::gauge(
                "Temp",
                48.0,
                0.0,
                120.0,
                "°C",
                "Oil",
                GaugeStyle {
                    size: 110.0,
                    dial_fg: Color::rgb(0xf0, 0xa0, 0x4a),
                    ..GaugeStyle::from_theme(&theme)
                },
            ),
            ChartTile::trend(
                "Voltage A",
                sample_series(10.0, 0.0, 48, 0.8),
                TrendStyle {
                    width: 280.0,
                    height: 100.0,
                    line: Color::rgb(0x4a, 0xc0, 0xf0),
                    fill: Color::rgb(0x1a, 0x40, 0x55),
                    ..TrendStyle::from_theme(&theme)
                },
            ),
            ChartTile::trend(
                "Current B",
                sample_series(80.0, 1.0, 48, 18.0),
                TrendStyle {
                    width: 280.0,
                    height: 100.0,
                    line: Color::rgb(0xe0, 0x70, 0xb0),
                    fill: Color::rgb(0x40, 0x1a, 0x35),
                    ..TrendStyle::from_theme(&theme)
                },
            ),
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
                            "Office charts — Chart (2×Gauge + 2×Trend) + standalone",
                            Color::rgb(0xa0, 0xa8, 0xb4),
                            12.0,
                        );
                    },
                );

                col.panel(
                    "Chart panel (multi-tile)",
                    PanelStyle {
                        width: shell_w - 16.0,
                        height: 380.0,
                        ..PanelStyle::from_theme(&theme)
                    },
                    |p| {
                        chart = p.chart(
                            tiles,
                            ChartStyle {
                                width: shell_w - 48.0,
                                height: 330.0,
                                columns: 2,
                                ..ChartStyle::from_theme(&theme)
                            },
                        );
                    },
                );

                col.row(
                    RowStyle {
                        width: shell_w - 16.0,
                        height: 200.0,
                        gap: 12.0,
                        padding: 0.0,
                        cross_align: CrossAlign::Start,
                        background: None,
                        ..RowStyle::from_theme(&theme)
                    },
                    |r| {
                        r.panel(
                            "Standalone Gauge",
                            PanelStyle {
                                width: 200.0,
                                height: 190.0,
                                ..PanelStyle::from_theme(&theme)
                            },
                            |p| {
                                gauge = p.gauge(
                                    55.0,
                                    0.0,
                                    100.0,
                                    "%",
                                    "PF",
                                    GaugeStyle {
                                        size: 120.0,
                                        dial_fg: Color::rgb(0x6a, 0xb0, 0xf0),
                                        ..GaugeStyle::from_theme(&theme)
                                    },
                                );
                            },
                        );
                        r.panel(
                            "Standalone Trend",
                            PanelStyle {
                                width: (shell_w - 240.0).max(320.0),
                                height: 190.0,
                                ..PanelStyle::from_theme(&theme)
                            },
                            |p| {
                                trend = p.trend(
                                    sample_series(50.0, 0.5, 64, 12.0),
                                    TrendStyle {
                                        width: (shell_w - 280.0).max(280.0),
                                        height: 130.0,
                                        ..TrendStyle::from_theme(&theme)
                                    },
                                );
                            },
                        );
                    },
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
                            "Animated · Space Soft↔GPU",
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
            chart,
            gauge,
            trend,
            status,
            needs_paint: true,
            pending_native: matches!(window_backend, PresentBackend::GpuSurface),
            started: Instant::now(),
            last_tick: Instant::now(),
            qa_dumps: 0,
        }
    }

    fn refresh_status(&mut self) {
        self.ui.set_text(
            self.status,
            format!("{} ({})", self.backend.name(), self.backend.detail()),
        );
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    fn tick_values(&mut self) {
        let t = self.started.elapsed().as_secs_f32();
        let load = 55.0 + 20.0 * (t * 0.7).sin();
        let temp = 42.0 + 15.0 * (t * 0.45).cos();
        let _ = self.ui.set_chart_gauge_value(self.chart, 0, load);
        let _ = self.ui.set_chart_gauge_value(self.chart, 1, temp);
        let _ = self
            .ui
            .set_chart_trend_points(self.chart, 2, sample_series(10.0, t, 48, 0.8));
        let _ = self
            .ui
            .set_chart_trend_points(self.chart, 3, sample_series(80.0, t + 1.0, 48, 18.0));
        let pf = 50.0 + 25.0 * (t * 0.55).sin();
        let _ = self.ui.set_gauge_value(self.gauge, pf);
        let _ = self
            .ui
            .set_trend_points(self.trend, sample_series(50.0, t + 0.5, 64, 12.0));
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
        if std::env::var_os("NEXT_GUI_CHARTS_QA").is_none() {
            return;
        }
        if self.qa_dumps >= 1 {
            return;
        }
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tmp/demo-shots/23-office_charts.ppm");
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
            eprintln!("e23_office_charts: Soft primary (Space toggles GPU if available)");
            let _ = GpuRenderer::try_new(w, h, CLEAR).map(|gpu| {
                self.gpu_stash = Some(gpu);
            });
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!("next-gui Office charts — {}", self.backend.name()));
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
                            "next-gui Office charts — {}",
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
        if self.last_tick.elapsed().as_millis() >= 50 {
            self.last_tick = Instant::now();
            let (w, h) = ctx.size();
            self.tick_values();
            self.scene.mark_full_dirty(w as f32, h as f32);
            ctx.request_redraw();
        }
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
    let use_native =
        std::env::var_os("NEXT_GUI_GPU_SURFACE").is_some() && GpuRenderer::adapter_available();
    let attrs = if use_native {
        WindowAttrs::new("next-gui Office charts", 1120, 820).with_gpu_surface()
    } else {
        WindowAttrs::new("next-gui Office charts", 1120, 820)
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
