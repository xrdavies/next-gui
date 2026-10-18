//! P5 demo: retained shell hosts a local Immediate tool panel.
//!
//! - Left: retained Panel / buttons / status (dirty-region path).
//! - Right: `ImmediateViewport` rebuilt each frame (counters, sparkline, buttons).
//! - Clicks outside the viewport never fire Immediate widgets; Esc blurs Immediate
//!   focus first (second Esc / close quits via window chrome).
//! - Field profile keeps Immediate **off** unless `NEXT_GUI_IMMEDIATE=1` or the
//!   retained "Enable Immediate" button is used. Office defaults Immediate on.
//! - Soft path by default; Space toggles Soft↔GPU when GPU is allowed.
//! - `NEXT_GUI_IMMEDIATE_QA=1` dumps a Soft PPM after paint (forces Immediate on).

use std::time::{Duration, Instant};

use next_gui_immediate::{ImmediateId, ImmediateViewport, ImmediateViewportStyle};
use next_gui_layout::CrossAlign;
use next_gui_platform::{
    AppCallbacks, Event, KeyCode, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_profile::ProfileConfig;
use next_gui_render::{encode_scene, DrawCommand, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Rect, Scene};
use next_gui_text::{load_default_cjk_font, Font};
use next_gui_ui::{
    ColumnStyle, PanelStyle, RowStyle, StatusBarStyle, ToolbarStyle, Ui, WidgetId,
};

const CLEAR: Color = Color {
    r: 0x1a,
    g: 0x1a,
    b: 0x1c,
    a: 255,
};

const WIN_W: u32 = 960;
const WIN_H: u32 = 560;
const IMM_W: f32 = 340.0;

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

    fn render(&mut self, cmds: &[DrawCommand], dirty: &next_gui_scene::DirtyRegion) {
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

fn write_ppm(path: &str, pixels: &[u32], w: u32, h: u32) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "P6\n{w} {h}\n255")?;
    for &px in pixels {
        let r = ((px >> 16) & 0xff) as u8;
        let g = ((px >> 8) & 0xff) as u8;
        let b = (px & 0xff) as u8;
        f.write_all(&[r, g, b])?;
    }
    Ok(())
}

fn imm_bounds(win_w: f32, win_h: f32) -> Rect {
    let margin = 12.0;
    let x = win_w - IMM_W - margin;
    Rect::new(x, 48.0, IMM_W, (win_h - 48.0 - 36.0 - margin).max(120.0))
}

struct Demo {
    scene: Scene,
    ui: Ui,
    font: Font,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    using_gpu: bool,
    profile: ProfileConfig,
    viewport: ImmediateViewport,
    // retained widgets
    title: WidgetId,
    hint: WidgetId,
    status: WidgetId,
    retained_count_label: WidgetId,
    btn_retained: WidgetId,
    btn_enable_imm: WidgetId,
    retained_clicks: u32,
    imm_ticks: u32,
    imm_btn_tick: Option<ImmediateId>,
    imm_btn_reset: Option<ImmediateId>,
    spark: Vec<f32>,
    needs_paint: bool,
    last_tick: Instant,
    qa_dumps: u32,
    win_w: f32,
    win_h: f32,
}

impl Demo {
    fn new(width: u32, height: u32, profile: ProfileConfig) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });

        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font.clone());
        let mut title = WidgetId(0);
        let mut hint = WidgetId(0);
        let mut status = WidgetId(0);
        let mut retained_count_label = WidgetId(0);
        let mut btn_retained = WidgetId(0);
        let mut btn_enable_imm = WidgetId(0);

        let left_w = (width as f32 - IMM_W - 40.0).max(360.0);

        ui.toolbar(
            ToolbarStyle {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: 40.0,
                ..ToolbarStyle::default()
            },
            |tb| {
                tb.text_styled(
                    "P5 Immediate viewport",
                    Color::rgb(0xf0, 0xf4, 0xfa),
                    15.0,
                );
            },
        );

        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 48.0,
                width: left_w,
                height: (height as f32 - 48.0 - 40.0).max(200.0),
                padding: 12.0,
                gap: 10.0,
                cross_align: CrossAlign::Start,
                background: Some(Color::rgb(0x22, 0x22, 0x26)),
            },
            |col| {
                title = col.text_styled("Retained shell", Color::rgb(0xff, 0xff, 0xff), 20.0);
                hint = col.text_styled(
                    "Clicks here stay retained; right panel is Immediate.",
                    Color::rgb(0xa8, 0xb0, 0xbc),
                    13.0,
                );

                col.panel(
                    "Retained controls",
                    PanelStyle {
                        width: (left_w - 24.0).max(300.0),
                        height: 180.0,
                        ..PanelStyle::default()
                    },
                    |p| {
                        retained_count_label =
                            p.text_styled("Retained clicks: 0", Color::rgb(0xe0, 0xe4, 0xec), 14.0);
                        p.spacer(1.0, 8.0);
                        p.row(
                            RowStyle {
                                width: (left_w - 48.0).max(260.0),
                                height: 36.0,
                                gap: 10.0,
                                padding: 0.0,
                                background: None,
                                ..RowStyle::default()
                            },
                            |row| {
                                btn_retained = row.button("Retained +1");
                                btn_enable_imm = row.button("Toggle Immediate");
                            },
                        );
                        p.spacer(1.0, 8.0);
                        p.text_styled(
                            "Space: Soft↔GPU (when allowed) · Esc: blur Immediate",
                            Color::rgb(0x90, 0x98, 0xa4),
                            12.0,
                        );
                    },
                );
            },
        );

        ui.status_bar(
            StatusBarStyle {
                x: 0.0,
                y: height as f32 - 28.0,
                width: width as f32,
                height: 28.0,
                ..StatusBarStyle::default()
            },
            |sb| {
                status = sb.text_styled("…", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
            },
        );

        ui.sync(&mut scene);

        let bounds = imm_bounds(width as f32, height as f32);
        let mut viewport = ImmediateViewport::new(
            bounds,
            ImmediateViewportStyle {
                background: Color::rgb(0x1e, 0x24, 0x2c),
                ..ImmediateViewportStyle::default()
            },
        );
        viewport.set_title("Immediate tool panel");
        // Field default off; Office / explicit enable on.
        let enable = profile.allow_immediate
            || std::env::var_os("NEXT_GUI_IMMEDIATE").is_some()
            || std::env::var_os("NEXT_GUI_IMMEDIATE_QA").is_some();
        viewport.set_enabled(enable);
        let root = scene.root();
        viewport.mount(&mut scene, root);

        Self {
            scene,
            ui,
            font,
            backend: Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
            gpu_stash: None,
            using_gpu: false,
            profile,
            viewport,
            title,
            hint,
            status,
            retained_count_label,
            btn_retained,
            btn_enable_imm,
            retained_clicks: 0,
            imm_ticks: 0,
            imm_btn_tick: None,
            imm_btn_reset: None,
            spark: vec![0.2, 0.4, 0.35, 0.6, 0.55, 0.7, 0.65, 0.8],
            needs_paint: true,
            last_tick: Instant::now(),
            qa_dumps: 0,
            win_w: width as f32,
            win_h: height as f32,
        }
    }

    fn refresh_retained_labels(&mut self) {
        let imm_state = if self.viewport.is_enabled() {
            if self.viewport.is_focused() {
                "Immediate ON (focused)"
            } else {
                "Immediate ON"
            }
        } else {
            "Immediate OFF (Field default)"
        };
        self.ui.set_text(
            self.title,
            format!("Retained shell · {}", self.profile.name()),
        );
        self.ui.set_text(
            self.hint,
            format!(
                "{imm_state} · backend {} ({})",
                self.backend.name(),
                self.backend.detail()
            ),
        );
        self.ui.set_text(
            self.retained_count_label,
            format!("Retained clicks: {}", self.retained_clicks),
        );
        self.ui.set_text(
            self.status,
            format!(
                "{} · imm={} · retained={} · imm_ticks={}",
                self.profile.summary(),
                self.viewport.is_enabled(),
                self.retained_clicks,
                self.imm_ticks
            ),
        );
        self.ui.sync(&mut self.scene);
        // Keep Immediate host on top after retained sync.
        let root = self.scene.root();
        self.viewport.mount(&mut self.scene, root);
    }

    fn rebuild_immediate(&mut self) {
        if !self.viewport.is_enabled() {
            self.imm_btn_tick = None;
            self.imm_btn_reset = None;
            return;
        }
        self.viewport.set_bounds(imm_bounds(self.win_w, self.win_h));
        let root = self.scene.root();
        self.viewport.mount(&mut self.scene, root);
        let content;
        let btn_tick;
        let btn_reset;
        let frame_n = self.viewport.frame_index().wrapping_add(1);
        let focused = self.viewport.is_focused();
        let ticks = self.imm_ticks;
        {
            let mut ctx = self.viewport.begin_frame(&self.font);
            ctx.paint_title();
            ctx.label(
                format!("frame #{frame_n}  ticks={ticks}"),
                14.0,
                Color::rgb(0xe8, 0xee, 0xf6),
            );
            ctx.label(
                if focused {
                    "focus: inside viewport"
                } else {
                    "focus: not captured (click panel)"
                },
                12.0,
                Color::rgb(0x9a, 0xb0, 0xc4),
            );
            ctx.spacer(4.0);
            btn_tick = ctx.button("Immediate +1");
            btn_reset = ctx.button("Reset ticks");
            ctx.spacer(6.0);
            ctx.label("Live sparkline (per-frame)", 12.0, Color::rgb(0xb0, 0xb8, 0xc4));
            let (cx, cy) = ctx.cursor();
            content = ctx.content_bounds();
            let spark_rect = Rect::new(
                content.x + 10.0,
                cy,
                (content.width - 20.0).max(40.0),
                72.0,
            );
            let _ = cx;
            ctx.sparkline(
                spark_rect,
                &self.spark,
                Color::rgb(0x4f, 0xc3, 0xf7),
                Color::rgb(0x16, 0x1c, 0x24),
            );
            ctx.set_cursor(content.x + 10.0, spark_rect.y + spark_rect.height + 10.0);
            ctx.label(
                "Outside clicks never reach these buttons.",
                11.0,
                Color::rgb(0x80, 0x88, 0x94),
            );
        }
        self.imm_btn_tick = Some(btn_tick);
        self.imm_btn_reset = Some(btn_reset);
        let _ = content;
    }

    fn animate_spark(&mut self) {
        let t = self.imm_ticks as f32 * 0.15;
        let v = 0.45 + 0.35 * (t.sin()) + 0.1 * ((t * 2.3).cos());
        self.spark.push(v.clamp(0.05, 0.95));
        if self.spark.len() > 48 {
            self.spark.remove(0);
        }
    }

    fn toggle_immediate(&mut self) {
        let next = !self.viewport.is_enabled();
        self.viewport.set_enabled(next);
        eprintln!("immediate → {}", if next { "ON" } else { "OFF" });
        self.refresh_retained_labels();
        self.scene.mark_full_dirty(self.win_w, self.win_h);
        self.needs_paint = true;
    }

    fn toggle_backend(&mut self) {
        if !self.profile.should_try_gpu() && !self.using_gpu {
            eprintln!("e25: GPU disallowed by profile — stay Soft");
            return;
        }
        let (w, h) = (self.win_w as u32, self.win_h as u32);
        if self.using_gpu {
            if let Backend::Gpu(gpu) = std::mem::replace(
                &mut self.backend,
                Backend::Soft(SoftRenderer::new(w, h, CLEAR)),
            ) {
                self.gpu_stash = Some(gpu);
            }
            self.using_gpu = false;
        } else {
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
                        eprintln!("e25: GPU unavailable ({e})");
                        return;
                    }
                },
            }
        }
        self.refresh_retained_labels();
        self.scene.mark_full_dirty(self.win_w, self.win_h);
        self.needs_paint = true;
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
                if let Some((pixels, w, h)) = s.frame_pixels() {
                    let pixels = pixels.to_vec();
                    let _ = ctx.present(w, h, &pixels);
                }
            }
        }
    }

    fn maybe_qa_dump(&mut self) {
        if std::env::var_os("NEXT_GUI_IMMEDIATE_QA").is_none() {
            return;
        }
        if self.qa_dumps >= 1 {
            return;
        }
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tmp/demo-shots/25-immediate_viewport.ppm");
            match write_ppm(path, pixels, w, h) {
                Ok(()) => {
                    eprintln!("QA wrote {path} ({w}x{h})");
                    self.qa_dumps += 1;
                    // Headless/CI: exit after one Soft frame dump.
                    std::process::exit(0);
                }
                Err(e) => eprintln!("QA dump failed: {e}"),
            }
        }
    }

    fn paint(&mut self, ctx: &mut WindowCtx<'_>) {
        self.rebuild_immediate();
        let mut dirty = self.scene.take_dirty();
        if dirty.is_empty() {
            // Still need to refresh Immediate overlay when enabled.
            if self.viewport.is_enabled() {
                dirty.add(self.viewport.bounds());
            } else {
                return;
            }
        }
        let mut cmds = encode_scene(&self.scene, &dirty);
        if self.viewport.is_enabled() {
            cmds.extend_from_slice(self.viewport.draw_commands());
        }
        self.backend.render(&cmds, &dirty);
        self.present_frame(ctx);
        self.maybe_qa_dump();
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        self.win_w = w as f32;
        self.win_h = h as f32;
        if self.profile.should_try_gpu() {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!("e25: GPU ready ({})", gpu.backend_name());
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("e25: Soft only ({e})"),
            }
        } else {
            eprintln!("e25: profile disallows GPU — Soft");
        }
        self.refresh_retained_labels();
        self.scene.mark_full_dirty(self.win_w, self.win_h);
        ctx.set_title(&format!(
            "next-gui Immediate — {} / {}",
            self.profile.name(),
            self.backend.name()
        ));
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.win_w = width as f32;
                self.win_h = height as f32;
                self.backend.resize(width, height);
                if let Some(g) = self.gpu_stash.as_mut() {
                    g.resize(width, height);
                }
                self.viewport.set_bounds(imm_bounds(self.win_w, self.win_h));
                self.refresh_retained_labels();
                self.scene.mark_full_dirty(self.win_w, self.win_h);
                self.needs_paint = true;
                ctx.request_redraw();
            }
            Event::PointerButton {
                button: PointerButton::Left,
                pressed,
                x,
                y,
                ..
            } => {
                let x = x as f32;
                let y = y as f32;
                if pressed {
                    // Immediate boundary first — retained must not steal inside events.
                    if self.viewport.pointer_pressed(x, y) {
                        self.refresh_retained_labels();
                        self.scene.mark_dirty(self.viewport.bounds());
                        self.needs_paint = true;
                        ctx.request_redraw();
                        return;
                    }
                    // Outside Immediate: blur + retained hit.
                    self.viewport.set_focused(false);
                    if let Some(id) = self.ui.hit_widget(&self.scene, x, y) {
                        if id == self.btn_retained {
                            self.retained_clicks = self.retained_clicks.wrapping_add(1);
                            eprintln!("retained click → {}", self.retained_clicks);
                            self.refresh_retained_labels();
                            self.scene.mark_full_dirty(self.win_w, self.win_h);
                            self.needs_paint = true;
                            ctx.request_redraw();
                        } else if id == self.btn_enable_imm {
                            self.toggle_immediate();
                            ctx.set_title(&format!(
                                "next-gui Immediate — {} / {} / imm={}",
                                self.profile.name(),
                                self.backend.name(),
                                self.viewport.is_enabled()
                            ));
                            ctx.request_redraw();
                        }
                    }
                } else if self.viewport.pointer_released(x, y) {
                    if let Some(id) = self.viewport.take_clicked() {
                        if Some(id) == self.imm_btn_tick {
                            self.imm_ticks = self.imm_ticks.wrapping_add(1);
                            self.animate_spark();
                            eprintln!("immediate +1 → {}", self.imm_ticks);
                        } else if Some(id) == self.imm_btn_reset {
                            self.imm_ticks = 0;
                            self.spark = vec![0.2, 0.4, 0.35, 0.6];
                            eprintln!("immediate reset");
                        }
                    }
                    self.scene.mark_dirty(self.viewport.bounds());
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::Key {
                key: KeyCode::Escape,
                pressed: true,
                ..
            } => {
                if self.viewport.handle_escape() {
                    eprintln!("immediate blur (Esc)");
                    self.refresh_retained_labels();
                    self.scene.mark_dirty(self.viewport.bounds());
                    self.needs_paint = true;
                    ctx.request_redraw();
                } else {
                    ctx.request_exit();
                }
            }
            Event::Key {
                key: KeyCode::Space,
                pressed: true,
                ..
            } => {
                self.toggle_backend();
                ctx.set_title(&format!(
                    "next-gui Immediate — {} / {}",
                    self.profile.name(),
                    self.backend.name()
                ));
                ctx.request_redraw();
            }
            Event::RedrawRequested { .. } => {
                self.needs_paint = true;
            }
            _ => {}
        }
    }

    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
        // Frame-refresh Immediate only (outer retained stays calm unless dirty).
        if self.viewport.is_enabled() && self.last_tick.elapsed() >= Duration::from_millis(50) {
            self.last_tick = Instant::now();
            self.animate_spark();
            self.scene.mark_dirty(self.viewport.bounds());
            self.needs_paint = true;
        }
        if self.needs_paint {
            self.paint(ctx);
            self.needs_paint = false;
        }
        if self.viewport.is_enabled() {
            ctx.request_redraw();
        }
    }
}

fn main() {
    let mut profile = ProfileConfig::from_env();
    // QA: Soft + Immediate on for screenshot.
    if std::env::var_os("NEXT_GUI_IMMEDIATE_QA").is_some() {
        if std::env::var_os("NEXT_GUI_PROFILE").is_none() {
            profile = ProfileConfig::field();
            profile.allow_immediate = true;
            eprintln!("e25 QA: Field Soft + Immediate forced on");
        } else {
            profile.allow_immediate = true;
        }
    }

    let attrs = WindowAttrs::new(
        format!("next-gui Immediate — {}", profile.name()),
        WIN_W,
        WIN_H,
    );
    // Always Softbuffer window for QA reliability; GPU is readback when toggled.
    let _ = PresentBackend::Softbuffer;
    let demo = Demo::new(attrs.width, attrs.height, profile);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed: {e}");
        std::process::exit(1);
    }
}
