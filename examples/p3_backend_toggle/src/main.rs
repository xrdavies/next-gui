//! P3 demo: same retained UI on Soft or GPU; Space toggles when GPU is available.
//!
//! Escape / close to quit.

use next_gui_platform::{AppCallbacks, Event, KeyCode, PointerButton, WindowAttrs, WindowCtx};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::GpuRenderer;
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{ColumnStyle, Ui, WidgetId};

const CLEAR: Color = Color {
    r: 0x1c,
    g: 0x1c,
    b: 0x1e,
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
            Backend::Soft(_) => "CPU softbuffer".into(),
            Backend::Gpu(g) => format!("wgpu {}", g.backend_name()),
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        match self {
            Backend::Soft(s) => s.resize(w, h),
            Backend::Gpu(g) => g.resize(w, h),
        }
    }

    fn render(&mut self, cmds: &[next_gui_render::DrawCommand], dirty: &next_gui_scene::DirtyRegion) {
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

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_available: Option<GpuRenderer>,
    using_gpu: bool,
    title: WidgetId,
    status: WidgetId,
    hint: WidgetId,
    btn_inc: WidgetId,
    btn_reset: WidgetId,
    count: i32,
    needs_paint: bool,
    frame: u64,
}

impl Demo {
    fn new(width: u32, height: u32) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e} (continuing may show empty glyphs)");
            // Fall through: try again would fail; exit for clarity
            std::process::exit(1);
        });

        let (backend, gpu_stash, using_gpu) = match GpuRenderer::try_new(width, height, CLEAR) {
            Ok(gpu) => {
                eprintln!(
                    "P3: GPU ready ({}); Soft available as fallback. Space toggles.",
                    gpu.backend_name()
                );
                // Start on GPU; keep a soft renderer for toggle by recreating soft when switching.
                (Backend::Gpu(gpu), None, true)
            }
            Err(e) => {
                eprintln!("P3: GPU unavailable ({e}); using Soft only.",);
                (
                    Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
                    None,
                    false,
                )
            }
        };

        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font);
        let mut title = WidgetId(0);
        let mut status = WidgetId(0);
        let mut hint = WidgetId(0);
        let mut btn_inc = WidgetId(0);
        let mut btn_reset = WidgetId(0);

        ui.column(
            ColumnStyle {
                x: 28.0,
                y: 24.0,
                width: 520.0,
                height: 400.0,
                padding: 18.0,
                gap: 12.0,
                background: Some(Color::rgb(0x2c, 0x2c, 0x2e)),
                            ..ColumnStyle::default()
            },
            |col| {
                title = col.text_styled(
                    "next-gui P3 — Soft / GPU",
                    Color::rgb(0xff, 0xff, 0xff),
                    24.0,
                );
                status = col.text_styled("count: 0", Color::rgb(0x8a, 0xe6, 0x9a), 20.0);
                hint = col.text_styled(
                    if using_gpu {
                        "Space: toggle Soft ↔ GPU"
                    } else {
                        "GPU unavailable — Soft only"
                    },
                    Color::rgb(0xc7, 0xcd, 0xd8),
                    16.0,
                );
                btn_inc = col.button("Increment");
                btn_reset = col.button_styled(
                    "Reset",
                    Color::rgb(0x6b, 0x72, 0x80),
                    Color::rgb(0xff, 0xff, 0xff),
                );
            },
        );

        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);

        Self {
            scene,
            ui,
            backend,
            gpu_available: gpu_stash,
            using_gpu,
            title,
            status,
            hint,
            btn_inc,
            btn_reset,
            count: 0,
            needs_paint: true,
            frame: 0,
        }
    }

    fn refresh_labels(&mut self) {
        let backend_label = match &self.backend {
            Backend::Soft(_) => "Soft".to_string(),
            Backend::Gpu(g) => format!("GPU ({})", g.backend_name()),
        };
        self.ui.set_text(
            self.title,
            format!("next-gui P3 — {backend_label}"),
        );
        self.ui
            .set_text(self.status, format!("count: {}", self.count));
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    fn toggle_backend(&mut self, width: u32, height: u32) {
        if !self.using_gpu {
            // Currently soft — switch to GPU if we can create/reuse one.
            match self.gpu_available.take() {
                Some(mut gpu) => {
                    gpu.resize(width, height);
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                None => match GpuRenderer::try_new(width, height, CLEAR) {
                    Ok(gpu) => {
                        self.backend = Backend::Gpu(gpu);
                        self.using_gpu = true;
                    }
                    Err(e) => {
                        eprintln!("toggle: GPU still unavailable: {e}");
                        return;
                    }
                },
            }
        } else {
            // Currently GPU — stash it and go Soft.
            if let Backend::Gpu(gpu) = std::mem::replace(
                &mut self.backend,
                Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
            ) {
                self.gpu_available = Some(gpu);
            }
            self.using_gpu = false;
        }

        let hint = if self.gpu_available.is_some() || self.using_gpu {
            "Space: toggle Soft ↔ GPU"
        } else {
            "GPU unavailable — Soft only"
        };
        self.ui.set_text(self.hint, hint);
        self.refresh_labels();
        self.scene.mark_full_dirty(width as f32, height as f32);
        eprintln!(
            "switched to {} ({})",
            self.backend.name(),
            self.backend.detail()
        );
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        eprintln!(
            "next-gui P3 ready ({}x{}). backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        self.refresh_labels();
        ctx.set_title(&format!(
            "next-gui P3 — {} — {}x{}",
            self.backend.name(),
            w,
            h
        ));
        self.needs_paint = true;
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.backend.resize(width, height);
                if let Some(gpu) = self.gpu_available.as_mut() {
                    gpu.resize(width, height);
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
                    if id == self.btn_inc {
                        self.count = self.count.saturating_add(1);
                        self.refresh_labels();
                        ctx.request_redraw();
                    } else if id == self.btn_reset {
                        self.count = 0;
                        self.refresh_labels();
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
                self.toggle_backend(w, h);
                ctx.set_title(&format!(
                    "next-gui P3 — {} — count {}",
                    self.backend.name(),
                    self.count
                ));
                ctx.request_redraw();
            }
            Event::Key {
                key: KeyCode::Escape,
                pressed: true,
                ..
            } => {}
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
        self.frame += 1;
        eprintln!(
            "frame {}: backend={} dirty={} cmds={} count={}",
            self.frame,
            self.backend.name(),
            dirty.rects.len(),
            cmds.len(),
            self.count
        );
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            if let Err(e) = ctx.present(w, h, pixels) {
                eprintln!("present error: {e}");
            }
            ctx.set_title(&format!(
                "next-gui P3 — {} — count {} — frame {}",
                self.backend.name(),
                self.count,
                self.frame
            ));
        }
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui P3 Soft/GPU", 580, 460);
    let demo = Demo::new(attrs.width, attrs.height);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed to run P3 window: {e}");
        std::process::exit(1);
    }
}
