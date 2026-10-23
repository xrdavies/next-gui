//! Office-line demo: toolbar Row + content panel; native GPU Surface when available.
//!
//! Space toggles Soft ↔ GPU when no focus; Tab + Space/Enter activate buttons.
//! Escape / close to quit.

use next_gui_layout::CrossAlign;
use next_gui_platform::{
    AppCallbacks, Event, KeyCode, Modifiers, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{ColumnStyle, RowStyle, Ui, UiKey, WidgetId};

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
            Backend::Soft(_) => "CPU soft / softbuffer-or-surface-blit".into(),
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

struct Demo {
    scene: Scene,
    ui: Ui,
    /// Active Soft or GPU backend (GPU may be created in `on_start` for native surface).
    backend: Backend,
    /// Stashed GPU when toggling to Soft on a Softbuffer window (readback path).
    gpu_stash: Option<GpuRenderer>,
    /// On GpuSurface windows, Soft still needs the GPU for surface blit present.
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    status: WidgetId,
    hint: WidgetId,
    btn_open: WidgetId,
    btn_save: WidgetId,
    btn_inc: WidgetId,
    count: i32,
    needs_paint: bool,
    pending_native: bool,
}


fn map_key(key: KeyCode, modifiers: Modifiers) -> Option<UiKey> {
    if matches!(key, KeyCode::Tab) {
        return Some(if modifiers.shift {
            UiKey::BackTab
        } else {
            UiKey::Tab
        });
    }
    match key {
        KeyCode::Enter => Some(UiKey::Enter),
        KeyCode::Space => Some(UiKey::Space),
        KeyCode::Escape => Some(UiKey::Escape),
        _ => None,
    }
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
                ..ColumnStyle::from_theme(&theme)
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
                        ..RowStyle::from_theme(&theme)
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
                    "starting…",
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

        let pending_native = window_backend == PresentBackend::GpuSurface;
        Self {
            scene,
            ui,
            backend: Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
            gpu_stash: None,
            surface_gpu: None,
            using_gpu: false,
            window_backend,
            status,
            hint,
            btn_open,
            btn_save,
            btn_inc,
            count: 0,
            needs_paint: true,
            pending_native,
        }
    }

    fn refresh(&mut self) {
        self.ui
            .set_text(self.status, format!("count: {}", self.count));
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    fn apply_button(&mut self, id: WidgetId) -> bool {
        if id == self.btn_inc {
            self.count = self.count.saturating_add(1);
            self.refresh();
            true
        } else if id == self.btn_open {
            eprintln!("Open clicked (stub)");
            true
        } else if id == self.btn_save {
            eprintln!("Save clicked (stub)");
            true
        } else {
            false
        }
    }

    fn set_hint_for_mode(&mut self) {
        let hint = match (self.window_backend, self.using_gpu) {
            (PresentBackend::GpuSurface, true) => {
                "Space: Soft ↔ GPU · present = native wgpu Surface"
            }
            (PresentBackend::GpuSurface, false) => {
                "Space: Soft ↔ GPU · Soft blits via Surface (no softbuffer)"
            }
            (PresentBackend::Softbuffer, true) => {
                "Space: Soft ↔ GPU · present = readback→softbuffer"
            }
            (PresentBackend::Softbuffer, false) => {
                if self.gpu_stash.is_some() {
                    "Space: Soft ↔ GPU · Soft softbuffer"
                } else {
                    "GPU unavailable — Soft only"
                }
            }
        };
        self.ui.set_text(self.hint, hint);
    }

    fn toggle(&mut self, w: u32, h: u32) {
        match self.window_backend {
            PresentBackend::GpuSurface => {
                if self.using_gpu {
                    // GPU → Soft (keep surface_gpu for blit present)
                    if let Backend::Gpu(gpu) = std::mem::replace(
                        &mut self.backend,
                        Backend::Soft(SoftRenderer::new(w, h, CLEAR)),
                    ) {
                        self.surface_gpu = Some(gpu);
                    }
                    self.using_gpu = false;
                } else {
                    // Soft → GPU
                    match self.surface_gpu.take() {
                        Some(mut gpu) => {
                            gpu.resize(w, h);
                            self.backend = Backend::Gpu(gpu);
                            self.using_gpu = true;
                        }
                        None => {
                            eprintln!("toggle: no native GPU available");
                            return;
                        }
                    }
                }
            }
            PresentBackend::Softbuffer => {
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
            }
        }
        self.set_hint_for_mode();
        self.refresh();
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
                    // Already presented inside GpuRenderer::render.
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
                        "e10_office_panel: native Surface ready ({}, present={:?})",
                        gpu.backend_name(),
                        gpu.present_path()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!(
                        "e10_office_panel: native Surface failed ({e}); Soft needs softbuffer — exit"
                    );
                    eprintln!(
                        "hint: start with Softbuffer window if GPU surface init fails"
                    );
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!(
                        "e10_office_panel: GPU readback ready ({}, present={:?})",
                        gpu.backend_name(),
                        gpu.present_path()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("e10_office_panel: GPU unavailable ({e}); Soft only");
                }
            }
        }

        self.set_hint_for_mode();
        self.refresh();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!(
            "e10_office_panel ready {}x{} backend={} ({}) window={:?}",
            w,
            h,
            self.backend.name(),
            self.backend.detail(),
            self.window_backend
        );
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
                let xf = x as f32;
                let yf = y as f32;
                let _ = self.ui.pointer_pressed(&mut self.scene, xf, yf);
                if let Some(id) = self.ui.hit_widget(&self.scene, xf, yf) {
                    if self.apply_button(id) {
                        ctx.request_redraw();
                    }
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
                        if let Some(id) = self.ui.take_button_activate() {
                            let _ = self.apply_button(id);
                        }
                        self.ui.sync(&mut self.scene);
                        self.needs_paint = true;
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
                        ctx.set_title(&format!("next-gui Office — {}", self.backend.name()));
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
        self.needs_paint = false;
    }
}

fn main() {
    // Prefer GpuSurface when an adapter exists; else Softbuffer.
    let use_native = GpuRenderer::adapter_available();
    let attrs = if use_native {
        eprintln!("e10_office_panel: adapter found → PresentBackend::GpuSurface");
        WindowAttrs::new("next-gui Office panel", 820, 560).with_gpu_surface()
    } else {
        eprintln!("e10_office_panel: no adapter → PresentBackend::Softbuffer");
        WindowAttrs::new("next-gui Office panel", 820, 560)
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
