//! P4 demo: same retained UI under Field ↔ Office profile.
//!
//! - `NEXT_GUI_PROFILE=field|office` selects the startup preset (default Office).
//! - Space or the toggle button flips presets: Field forces Soft; Office tries GPU
//!   (native Surface when the window was opened that way, else readback).
//! - `NEXT_GUI_PROFILE_QA=1` dumps a Soft-path PPM after first paint.
//!
//! Escape / close to quit.

use std::time::{Duration, Instant};

use next_gui_layout::CrossAlign;
use next_gui_platform::{
    AppCallbacks, Event, KeyCode, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_profile::{GuiProfile, ProfileConfig, SuggestedPresent};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{
    ColumnStyle, PanelStyle, RowStyle, StatusBarStyle, Ui, WidgetId,
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

fn write_ppm(path: &str, pixels: &[u32], w: u32, h: u32) -> std::io::Result<()> {
    use std::io::Write;
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

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    profile: ProfileConfig,
    title: WidgetId,
    budget: WidgetId,
    backend_label: WidgetId,
    hint: WidgetId,
    status: WidgetId,
    btn_toggle: WidgetId,
    btn_tick: WidgetId,
    tick: u32,
    needs_paint: bool,
    pending_native: bool,
    last_chart: Instant,
    qa_dumps: u32,
}

impl Demo {
    fn new_soft(width: u32, height: u32, window_backend: PresentBackend, profile: ProfileConfig) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });

        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font);
        let mut title = WidgetId(0);
        let mut budget = WidgetId(0);
        let mut backend_label = WidgetId(0);
        let mut hint = WidgetId(0);
        let mut status = WidgetId(0);
        let mut btn_toggle = WidgetId(0);
        let mut btn_tick = WidgetId(0);

        ui.column(
            ColumnStyle {
                x: 16.0,
                y: 12.0,
                width: (width as f32 - 32.0).max(400.0),
                height: (height as f32 - 24.0).max(360.0),
                padding: 12.0,
                gap: 10.0,
                cross_align: CrossAlign::Start,
                background: Some(Color::rgb(0x22, 0x22, 0x26)),
            },
            |col| {
                title = col.text_styled(
                    "Profile: …",
                    Color::rgb(0xff, 0xff, 0xff),
                    22.0,
                );
                col.text_styled(
                    "Same ui / scene — Field forces Soft; Office may use GPU.",
                    Color::rgb(0xa8, 0xb0, 0xbc),
                    13.0,
                );

                col.panel(
                    "Budgets",
                    PanelStyle {
                        width: (width as f32 - 64.0).max(360.0),
                        height: 200.0,
                        ..PanelStyle::default()
                    },
                    |p| {
                        budget = p.text_styled("…", Color::rgb(0xe0, 0xe4, 0xec), 14.0);
                        p.spacer(1.0, 8.0);
                        backend_label =
                            p.text_styled("Backend: …", Color::rgb(0x8a, 0xe6, 0x9a), 14.0);
                        p.spacer(1.0, 8.0);
                        hint = p.text_styled(
                            "Space / Toggle flips Field ↔ Office",
                            Color::rgb(0xa0, 0xa8, 0xb4),
                            13.0,
                        );
                    },
                );

                col.row(
                    RowStyle {
                        x: 0.0,
                        y: 0.0,
                        width: (width as f32 - 64.0).max(360.0),
                        height: 40.0,
                        gap: 10.0,
                        padding: 0.0,
                        cross_align: CrossAlign::Center,
                        background: None,
                    },
                    |row| {
                        btn_toggle = row.button("Toggle profile");
                        btn_tick = row.button("Manual tick");
                    },
                );

                col.status_bar(
                    StatusBarStyle {
                        width: (width as f32 - 64.0).max(360.0),
                        ..StatusBarStyle::default()
                    },
                    |sb| {
                        status = sb.text_styled("…", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
                    },
                );
            },
        );

        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);

        let pending_native = window_backend == PresentBackend::GpuSurface;
        Self {
            scene,
            ui,
            backend: Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
            gpu_stash: None,
            surface_gpu: None,
            using_gpu: false,
            window_backend,
            profile,
            title,
            budget,
            backend_label,
            hint,
            status,
            btn_toggle,
            btn_tick,
            tick: 0,
            needs_paint: true,
            pending_native,
            last_chart: Instant::now(),
            qa_dumps: 0,
        }
    }

    fn refresh_labels(&mut self) {
        let effects = if self.profile.effects_enabled {
            "on"
        } else {
            "off"
        };
        self.ui.set_text(
            self.title,
            format!("Profile: {}", self.profile.name()),
        );
        self.ui.set_text(
            self.budget,
            format!(
                "max_fps={}  chart={}Hz  effects={}  force_soft={}  allow_gpu={}\n\
                 prefer_surface={}  anim={:?}  suggested={:?}",
                self.profile.max_fps,
                if self.profile.chart_refresh_hz == 0 {
                    self.profile.max_fps
                } else {
                    self.profile.chart_refresh_hz
                },
                effects,
                self.profile.force_soft,
                self.profile.allow_gpu,
                self.profile.prefer_native_surface,
                self.profile.animation_budget,
                self.profile.suggested_present(),
            ),
        );
        self.ui.set_text(
            self.backend_label,
            format!(
                "Backend: {} ({}) · window={:?} · tick={}",
                self.backend.name(),
                self.backend.detail(),
                self.window_backend,
                self.tick
            ),
        );
        let hint = match self.profile.profile {
            GuiProfile::Field => {
                "Field: Soft only · Space → Office (GPU if available)"
            }
            GuiProfile::Office => {
                if self.using_gpu {
                    "Office: GPU active · Space → Field (force Soft)"
                } else {
                    "Office: Soft fallback · Space → Field"
                }
            }
        };
        self.ui.set_text(self.hint, hint);
        self.ui.set_text(self.status, self.profile.summary());
        self.ui.sync(&mut self.scene);
    }

    fn apply_profile_to_backend(&mut self, w: u32, h: u32) {
        if self.profile.should_try_gpu() {
            self.ensure_gpu(w, h);
        } else {
            self.ensure_soft(w, h);
        }
        self.refresh_labels();
        self.scene.mark_full_dirty(w as f32, h as f32);
        self.needs_paint = true;
    }

    fn ensure_soft(&mut self, w: u32, h: u32) {
        if !self.using_gpu {
            return;
        }
        match self.window_backend {
            PresentBackend::GpuSurface => {
                if let Backend::Gpu(gpu) = std::mem::replace(
                    &mut self.backend,
                    Backend::Soft(SoftRenderer::new(w, h, CLEAR)),
                ) {
                    self.surface_gpu = Some(gpu);
                }
            }
            PresentBackend::Softbuffer => {
                if let Backend::Gpu(gpu) = std::mem::replace(
                    &mut self.backend,
                    Backend::Soft(SoftRenderer::new(w, h, CLEAR)),
                ) {
                    self.gpu_stash = Some(gpu);
                }
            }
        }
        self.using_gpu = false;
    }

    fn ensure_gpu(&mut self, w: u32, h: u32) {
        if self.using_gpu {
            return;
        }
        match self.window_backend {
            PresentBackend::GpuSurface => match self.surface_gpu.take() {
                Some(mut gpu) => {
                    gpu.resize(w, h);
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                None => {
                    eprintln!("profile: GpuSurface but no surface GPU stashed — stay Soft");
                }
            },
            PresentBackend::Softbuffer => {
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
                            eprintln!("profile: GPU unavailable ({e}); Soft only");
                            self.profile.degrade(next_gui_profile::DegradationReason::NoGpu);
                        }
                    },
                }
            }
        }
    }

    fn toggle_profile(&mut self, w: u32, h: u32) {
        self.profile.toggle();
        eprintln!("profile → {}", self.profile.summary());
        self.apply_profile_to_backend(w, h);
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
                        } else {
                            eprintln!("Soft on GpuSurface but no surface_gpu for blit");
                        }
                    }
                }
            }
        }
    }

    fn maybe_qa_dump(&mut self) {
        if std::env::var_os("NEXT_GUI_PROFILE_QA").is_none() {
            return;
        }
        if self.qa_dumps >= 1 {
            return;
        }
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            let path = "/workspace/next-gui-demo-shots/24-profile_dual.ppm";
            if let Err(e) = write_ppm(path, pixels, w, h) {
                eprintln!("QA dump failed: {e}");
            } else {
                eprintln!("QA wrote {path} ({w}x{h})");
                self.qa_dumps += 1;
            }
        }
    }

    fn maybe_chart_tick(&mut self, ctx: &mut WindowCtx<'_>) {
        let interval = self.profile.chart_interval();
        if self.last_chart.elapsed() < interval {
            return;
        }
        self.last_chart = Instant::now();
        self.tick = self.tick.wrapping_add(1);
        self.refresh_labels();
        let (w, h) = ctx.size();
        self.scene.mark_full_dirty(w as f32, h as f32);
        self.needs_paint = true;
        ctx.request_redraw();
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
                        "e24: native Surface ready ({}, {:?})",
                        gpu.backend_name(),
                        gpu.present_path()
                    );
                    if self.profile.should_try_gpu() {
                        self.backend = Backend::Gpu(gpu);
                        self.using_gpu = true;
                    } else {
                        self.surface_gpu = Some(gpu);
                        self.using_gpu = false;
                    }
                }
                Err(e) => {
                    eprintln!("e24: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            if self.profile.should_try_gpu() {
                match GpuRenderer::try_new(w, h, CLEAR) {
                    Ok(gpu) => {
                        eprintln!(
                            "e24: GPU readback ready ({})",
                            gpu.backend_name()
                        );
                        self.backend = Backend::Gpu(gpu);
                        self.using_gpu = true;
                    }
                    Err(e) => {
                        eprintln!("e24: GPU unavailable ({e}); Soft only");
                        self.profile
                            .degrade(next_gui_profile::DegradationReason::NoGpu);
                    }
                }
            } else {
                eprintln!("e24: Field / force_soft — Soft only (GPU not attempted)");
            }
        }

        self.refresh_labels();
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!(
            "next-gui Profile — {} / {}",
            self.profile.name(),
            self.backend.name()
        ));
        eprintln!("e24 ready {}", self.profile.summary());
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
                if let Some(id) = self.ui.hit_widget(&self.scene, x as f32, y as f32) {
                    let (w, h) = ctx.size();
                    if id == self.btn_toggle {
                        self.toggle_profile(w, h);
                        ctx.set_title(&format!(
                            "next-gui Profile — {} / {}",
                            self.profile.name(),
                            self.backend.name()
                        ));
                        ctx.request_redraw();
                    } else if id == self.btn_tick {
                        self.tick = self.tick.wrapping_add(1);
                        self.refresh_labels();
                        self.scene.mark_full_dirty(w as f32, h as f32);
                        self.needs_paint = true;
                        ctx.request_redraw();
                    }
                }
            }
            Event::Key {
                key: KeyCode::Space,
                pressed: true,
                ..
            } => {
                let (w, h) = ctx.size();
                self.toggle_profile(w, h);
                ctx.set_title(&format!(
                    "next-gui Profile — {} / {}",
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
        self.maybe_chart_tick(ctx);
        if self.needs_paint {
            let dirty = self.scene.take_dirty();
            if !dirty.is_empty() {
                let cmds = encode_scene(&self.scene, &dirty);
                self.backend.render(&cmds, &dirty);
                self.present_frame(ctx);
                self.maybe_qa_dump();
            }
            self.needs_paint = false;
        }
        // Pace continuous ticks to profile.chart_interval (Field stays calm).
        let wait = self
            .profile
            .chart_interval()
            .saturating_sub(self.last_chart.elapsed())
            .min(Duration::from_millis(250));
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        ctx.request_redraw();
    }
}

fn window_attrs_for(profile: &ProfileConfig) -> WindowAttrs {
    let title = format!("next-gui Profile — {}", profile.name());
    let base = WindowAttrs::new(title, 880, 560);
    match profile.suggested_present() {
        SuggestedPresent::GpuSurface if GpuRenderer::adapter_available() => {
            eprintln!("e24: Office + adapter → PresentBackend::GpuSurface");
            base.with_gpu_surface()
        }
        SuggestedPresent::GpuSurface => {
            eprintln!("e24: Office wants GpuSurface but no adapter → Softbuffer");
            base
        }
        SuggestedPresent::Softbuffer => {
            eprintln!("e24: Field / Softbuffer window");
            base
        }
    }
}

fn main() {
    let mut profile = ProfileConfig::from_env();
    // QA seed: start Field so Soft path is guaranteed for PPM dump.
    if std::env::var_os("NEXT_GUI_PROFILE_QA").is_some()
        && std::env::var_os("NEXT_GUI_PROFILE").is_none()
    {
        profile = ProfileConfig::field();
        eprintln!("e24 QA: defaulting to Field for Soft screenshot");
    }

    let attrs = window_attrs_for(&profile);
    let demo = Demo::new_soft(attrs.width, attrs.height, attrs.present_backend, profile);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed: {e}");
        std::process::exit(1);
    }
}
