//! Office P0 shell demo: Toolbar + Icon + Splitter + Panel + StatusBar.
//!
//! Drag the splitter sash to resize panes. Buttons log clicks. Escape / close to quit.
//! Space toggles Soft ↔ GPU when available.

use next_gui_layout::{Axis, CrossAlign};
use next_gui_platform::{
    AppCallbacks, Event, KeyCode, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{
    ColumnStyle, IconName, PanelStyle, SplitterStyle, StatusBarStyle, ToolbarStyle, Ui, WidgetId,
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

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    splitter: WidgetId,
    status: WidgetId,
    mode_label: WidgetId,
    btn_open: WidgetId,
    btn_save: WidgetId,
    btn_inc: WidgetId,
    count: i32,
    needs_paint: bool,
    pending_native: bool,
}

impl Demo {
    fn new_soft(width: u32, height: u32, window_backend: PresentBackend) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });

        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font);
        let mut splitter = WidgetId(0);
        let mut status = WidgetId(0);
        let mut mode_label = WidgetId(0);
        let mut btn_open = WidgetId(0);
        let mut btn_save = WidgetId(0);
        let mut btn_inc = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(640.0);
        let shell_h = (height as f32 - 24.0).max(420.0);
        let split_h = (shell_h - 40.0 - 26.0 - 24.0).max(280.0);

        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 10.0,
                width: shell_w,
                height: shell_h,
                padding: 8.0,
                gap: 6.0,
                cross_align: CrossAlign::Start,
                background: Some(Color::rgb(0x20, 0x20, 0x24)),
            },
            |col| {
                col.toolbar(
                    ToolbarStyle {
                        width: shell_w - 16.0,
                        height: 40.0,
                        ..ToolbarStyle::default()
                    },
                    |tb| {
                        tb.icon(IconName::Menu);
                        tb.spacer(10.0, 1.0);
                        tb.icon(IconName::Folder);
                        btn_open = tb.tool_button("Open");
                        tb.spacer(4.0, 1.0);
                        tb.icon(IconName::Save);
                        btn_save = tb.tool_button("Save");
                        tb.spacer(12.0, 1.0);
                        tb.separator_vertical(22.0);
                        tb.spacer(12.0, 1.0);
                        btn_inc = tb.tool_button("Inc");
                        tb.spacer(8.0, 1.0);
                        status = tb.text_styled("count: 0", Color::rgb(0x8a, 0xe6, 0x9a), 13.0);
                    },
                );

                splitter = col.splitter(
                    SplitterStyle {
                        width: shell_w - 16.0,
                        height: split_h,
                        axis: Axis::Vertical,
                        ratio: 0.28,
                        sash_size: 5.0,
                        sash_color: Color::rgb(0x55, 0x55, 0x5e),
                        background: Some(Color::rgb(0x1c, 0x1c, 0x20)),
                        ..SplitterStyle::default()
                    },
                    |left| {
                        left.panel(
                            "Navigator",
                            PanelStyle {
                                width: 200.0,
                                height: split_h,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_styled(
                                    "厂站 / 间隔 (tree later)",
                                    Color::rgb(0xc0, 0xc4, 0xcc),
                                    14.0,
                                );
                                p.spacer(1.0, 8.0);
                                p.text_styled(
                                    "• Substation A",
                                    Color::rgb(0xa8, 0xb0, 0xbc),
                                    13.0,
                                );
                                p.text_styled(
                                    "• Bay 01",
                                    Color::rgb(0xa8, 0xb0, 0xbc),
                                    13.0,
                                );
                            },
                        );
                    },
                    |right| {
                        right.panel(
                            "Workspace",
                            PanelStyle {
                                width: 500.0,
                                height: split_h,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_styled(
                                    "Office shell — Icon / Toolbar / Panel / Splitter / StatusBar",
                                    Color::rgb(0xe0, 0xe4, 0xec),
                                    15.0,
                                );
                                p.spacer(1.0, 6.0);
                                p.text_styled(
                                    "Drag the sash between panes. Space toggles Soft↔GPU.",
                                    Color::rgb(0xa0, 0xa8, 0xb4),
                                    13.0,
                                );
                                p.spacer(1.0, 10.0);
                                p.icon_styled(IconName::Settings, 22.0, Color::rgb(0x7d, 0xd3, 0xfc));
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
                        sb.icon_styled(IconName::Info, 14.0, Color::rgb(0x8a, 0xe6, 0x9a));
                        sb.text_styled("Ready", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
                        sb.spacer(20.0, 1.0);
                        mode_label =
                            sb.text_styled("Soft", Color::rgb(0xa0, 0xa8, 0xb4), 12.0);
                        sb.spacer(20.0, 1.0);
                        sb.text_styled(
                            "next-gui Office P0 shell",
                            Color::rgb(0x88, 0x8e, 0x98),
                            12.0,
                        );
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
            splitter,
            status,
            mode_label,
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
        self.ui
            .set_text(self.mode_label, self.backend.name().to_string());
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    fn toggle(&mut self, w: u32, h: u32) {
        match self.window_backend {
            PresentBackend::GpuSurface => {
                if self.using_gpu {
                    if let Backend::Gpu(gpu) = std::mem::replace(
                        &mut self.backend,
                        Backend::Soft(SoftRenderer::new(w, h, CLEAR)),
                    ) {
                        self.surface_gpu = Some(gpu);
                    }
                    self.using_gpu = false;
                } else {
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
                        "office_shell: native Surface ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("office_shell: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!("office_shell: GPU readback ready ({})", gpu.backend_name());
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("office_shell: GPU unavailable ({e}); Soft only"),
            }
        }
        self.refresh();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!(
            "office_shell ready {}x{} backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        ctx.set_title(&format!("next-gui Office shell — {}", self.backend.name()));
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
                if self.ui.pointer_pressed(&self.scene, x as f32, y as f32) {
                    ctx.request_redraw();
                    return;
                }
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
            Event::PointerButton {
                button: PointerButton::Left,
                pressed: false,
                ..
            } => {
                if self.ui.is_dragging() {
                    self.ui.pointer_released();
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::PointerMoved { x, y, .. } => {
                if self.ui.pointer_moved(x as f32, y as f32) {
                    if let Some(r) = self.ui.splitter_ratio(self.splitter) {
                        eprintln!("splitter ratio={r:.2}");
                    }
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::Key {
                key: KeyCode::Space,
                pressed: true,
                ..
            } => {
                let (w, h) = ctx.size();
                self.toggle(w, h);
                ctx.set_title(&format!("next-gui Office shell — {}", self.backend.name()));
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
        self.present_frame(ctx);
        self.needs_paint = false;
    }
}

fn main() {
    // Prefer Softbuffer for reliable Linux headless/CI soft path; GPU still toggled when available.
    let use_native = std::env::var_os("NEXT_GUI_GPU_SURFACE").is_some()
        && GpuRenderer::adapter_available();
    let attrs = if use_native {
        eprintln!("office_shell: NEXT_GUI_GPU_SURFACE → GpuSurface");
        WindowAttrs::new("next-gui Office shell", 900, 600).with_gpu_surface()
    } else {
        eprintln!("office_shell: Softbuffer window (set NEXT_GUI_GPU_SURFACE=1 for native Surface)");
        WindowAttrs::new("next-gui Office shell", 900, 600)
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
