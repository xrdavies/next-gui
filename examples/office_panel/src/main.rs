//! Office-line demo: toolbar Row + content panel; GPU preferred, Soft fallback.
//!
//! Space toggles Soft ↔ GPU when available. Escape / close to quit.

use next_gui_layout::CrossAlign;
use next_gui_platform::{AppCallbacks, Event, KeyCode, PointerButton, WindowAttrs, WindowCtx};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{ColumnStyle, RowStyle, Ui, WidgetId};

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
            Backend::Soft(_) => "CPU softbuffer".into(),
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

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    using_gpu: bool,
    status: WidgetId,
    hint: WidgetId,
    btn_open: WidgetId,
    btn_save: WidgetId,
    btn_inc: WidgetId,
    count: i32,
    needs_paint: bool,
}

impl Demo {
    fn new(width: u32, height: u32) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });

        let (backend, using_gpu) = match GpuRenderer::try_new(width, height, CLEAR) {
            Ok(gpu) => {
                eprintln!(
                    "office_panel: GPU ready ({}, present={:?}); Soft fallback via Space",
                    gpu.backend_name(),
                    gpu.present_path()
                );
                (Backend::Gpu(gpu), true)
            }
            Err(e) => {
                eprintln!("office_panel: GPU unavailable ({e}); Soft only");
                (Backend::Soft(SoftRenderer::new(width, height, CLEAR)), false)
            }
        };

        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font);
        let mut status = WidgetId(0);
        let mut hint = WidgetId(0);
        let mut btn_open = WidgetId(0);
        let mut btn_save = WidgetId(0);
        let mut btn_inc = WidgetId(0);

        ui.column(
            ColumnStyle {
                x: 20.0,
                y: 16.0,
                width: 760.0,
                height: 520.0,
                padding: 14.0,
                gap: 12.0,
                cross_align: CrossAlign::Start,
                background: Some(Color::rgb(0x24, 0x24, 0x28)),
            },
            |col| {
                col.text_styled(
                    "next-gui Office panel",
                    Color::rgb(0xff, 0xff, 0xff),
                    22.0,
                );
                col.row(
                    RowStyle {
                        width: 720.0,
                        height: 48.0,
                        padding: 8.0,
                        gap: 8.0,
                        cross_align: CrossAlign::Center,
                        background: Some(Color::rgb(0x32, 0x32, 0x38)),
                        ..RowStyle::default()
                    },
                    |row| {
                        btn_open = row.button("Open");
                        btn_save = row.button_styled(
                            "Save",
                            Color::rgb(0x22, 0xc5, 0x5e),
                            Color::rgb(0xff, 0xff, 0xff),
                        );
                        row.spacer(16.0, 1.0);
                        row.separator_vertical(28.0);
                        row.spacer(16.0, 1.0);
                        btn_inc = row.button("Increment");
                        row.spacer(8.0, 1.0);
                        status = row.text_styled("count: 0", Color::rgb(0x8a, 0xe6, 0x9a), 16.0);
                    },
                );
                col.separator();
                hint = col.text_styled(
                    if using_gpu {
                        "Space: Soft ↔ GPU · present still readback→softbuffer (see docs/office-line.md)"
                    } else {
                        "GPU unavailable — Soft only"
                    },
                    Color::rgb(0xa0, 0xa8, 0xb4),
                    14.0,
                );
                col.text_styled(
                    "工具条 = Row + Spacer + Separator；内容区 retained + 脏区。",
                    Color::rgb(0xc7, 0xcd, 0xd8),
                    16.0,
                );
            },
        );

        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);

        Self {
            scene,
            ui,
            backend,
            gpu_stash: None,
            using_gpu,
            status,
            hint,
            btn_open,
            btn_save,
            btn_inc,
            count: 0,
            needs_paint: true,
        }
    }

    fn refresh(&mut self) {
        self.ui
            .set_text(self.status, format!("count: {}", self.count));
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
        let hint = if self.using_gpu || self.gpu_stash.is_some() {
            "Space: Soft ↔ GPU · present still readback→softbuffer (see docs/office-line.md)"
        } else {
            "GPU unavailable — Soft only"
        };
        self.ui.set_text(self.hint, hint);
        self.refresh();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!("switched to {} ({})", self.backend.name(), self.backend.detail());
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        eprintln!(
            "office_panel ready {}x{} backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        self.refresh();
        ctx.set_title(&format!("next-gui Office — {}", self.backend.name()));
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.backend.resize(width, height);
                if let Some(g) = self.gpu_stash.as_mut() {
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
                    if id == self.btn_inc {
                        self.count = self.count.saturating_add(1);
                        self.refresh();
                        ctx.request_redraw();
                    } else if id == self.btn_open {
                        eprintln!("Open clicked (stub)");
                    } else if id == self.btn_save {
                        eprintln!("Save clicked (stub)");
                    }
                }
            }
            Event::Key {
                key: KeyCode::Space,
                pressed: true,
                ..
            } => {
                let (w, h) = ctx.size();
                self.toggle(w, h);
                ctx.set_title(&format!("next-gui Office — {}", self.backend.name()));
                ctx.request_redraw();
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
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            if let Err(e) = ctx.present(w, h, pixels) {
                eprintln!("present error: {e}");
            }
        }
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui Office panel", 820, 560);
    let demo = Demo::new(attrs.width, attrs.height);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed: {e}");
        std::process::exit(1);
    }
}
