//! Office P2a demo: `button_group`, `link`, `space`.
//!
//! Tab focuses the group / links; ←/→ changes group selection; Enter/Space on a
//! link drains `take_link_activate`. Pointer uses the Button dual path for links
//! (`hit_widget`). Space Soft↔GPU when nothing focused.
//!
//! `NEXT_GUI_P2A_QA=1` dumps a Soft PPM after first paint (then exits via
//! request_exit after dump — no hang without interaction).

use std::io::Write;

use next_gui_layout::CrossAlign;
use next_gui_platform::{
    AppCallbacks, Event, KeyCode, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_ui::{
    ButtonGroupStyle, ColumnStyle, LinkStyle, PanelStyle, SpaceDirection, SpaceStyle,
    StatusBarStyle, ToolbarStyle, Ui, UiKey, WidgetId,
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
    if matches!(key, KeyCode::Tab) {
        return Some(if modifiers.shift {
            UiKey::BackTab
        } else {
            UiKey::Tab
        });
    }
    Some(match key {
        KeyCode::Left => UiKey::Left,
        KeyCode::Right => UiKey::Right,
        KeyCode::Up => UiKey::Up,
        KeyCode::Down => UiKey::Down,
        KeyCode::Enter => UiKey::Enter,
        KeyCode::Space => UiKey::Space,
        KeyCode::Escape => UiKey::Escape,
        _ => return None,
    })
}

fn write_ppm(path: &str, pixels: &[u32], w: u32, h: u32) -> std::io::Result<()> {
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
    using_gpu: bool,
    window_backend: PresentBackend,
    needs_paint: bool,
    status: WidgetId,
    group: WidgetId,
    link_docs: WidgetId,
    link_help: WidgetId,
    clicks: u32,
    qa_dumps: u32,
    qa_exit: bool,
}

impl Demo {
    fn new(width: u32, height: u32, window_backend: PresentBackend) -> Self {
        let mut ui = Ui::new();
        let mut status = WidgetId(0);
        let mut group = WidgetId(0);
        let mut link_docs = WidgetId(0);
        let mut link_help = WidgetId(0);

        ui.toolbar(
            ToolbarStyle {
                width: width as f32 - 16.0,
                height: 40.0,
                ..ToolbarStyle::default()
            },
            |bar| {
                bar.text("Office P2a");
                bar.spacer(12.0, 1.0);
                bar.text_styled(
                    "Tab · ←/→ group · Enter link · Space Soft↔GPU",
                    Color::rgb(0x90, 0x98, 0xa4),
                    12.0,
                );
            },
        );

        ui.panel(
            "P2a widgets",
            PanelStyle {
                x: 16.0,
                y: 56.0,
                width: width as f32 - 32.0,
                height: height as f32 - 100.0,
                ..PanelStyle::default()
            },
            |panel| {
                panel.text("button_group (exclusive, one focus)");
                group = panel.button_group(
                    ["Day", "Week", "Month"],
                    Some(0),
                    ButtonGroupStyle::default(),
                );
                panel.spacer(1.0, 12.0);
                panel.text("space (horizontal gap sugar over Row)");
                panel.space(
                    SpaceStyle {
                        direction: SpaceDirection::Horizontal,
                        gap: 20.0,
                        width: width as f32 - 64.0,
                        height: 36.0,
                        cross_align: CrossAlign::Center,
                        ..SpaceStyle::default()
                    },
                    |row| {
                        link_docs = row.link("Open docs");
                        link_help = row.link_styled(
                            "Help center",
                            LinkStyle {
                                underline: true,
                                fg: Color::rgb(0x90, 0xd0, 0xa0),
                                ..LinkStyle::default()
                            },
                        );
                        row.button("Solid CTA");
                    },
                );
                panel.spacer(1.0, 12.0);
                panel.text("space (vertical sugar over Column)");
                panel.space(
                    SpaceStyle {
                        direction: SpaceDirection::Vertical,
                        gap: 8.0,
                        width: width as f32 - 64.0,
                        height: 100.0,
                        cross_align: CrossAlign::Start,
                        ..SpaceStyle::default()
                    },
                    |col| {
                        col.text("Stacked with gap=8");
                        col.text("Second line");
                        col.link("Nested link");
                    },
                );
            },
        );

        ui.status_bar(
            StatusBarStyle {
                width: width as f32 - 16.0,
                height: 28.0,
                ..StatusBarStyle::default()
            },
            |bar| {
                status = bar.text_styled("ready", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
            },
        );

        let mut scene = Scene::new();
        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);

        let _ = ColumnStyle::default();

        Self {
            scene,
            ui,
            backend: Backend::Soft(SoftRenderer::new(width, height, CLEAR)),
            gpu_stash: None,
            using_gpu: false,
            window_backend,
            needs_paint: true,
            status,
            group,
            link_docs,
            link_help,
            clicks: 0,
            qa_dumps: 0,
            qa_exit: std::env::var_os("NEXT_GUI_P2A_QA").is_some(),
        }
    }

    fn refresh_status(&mut self) {
        let sel = self
            .ui
            .button_group_selected(self.group)
            .flatten()
            .map(|i| ["Day", "Week", "Month"][i])
            .unwrap_or("(none)");
        self.ui.set_text(
            self.status,
            format!(
                "range={sel} · links={} · {} ({})",
                self.clicks,
                self.backend.name(),
                self.backend.detail()
            ),
        );
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    fn apply_link_hit(&mut self, id: WidgetId) -> bool {
        if id == self.link_docs || id == self.link_help {
            self.clicks = self.clicks.saturating_add(1);
            eprintln!("link pointer → {}", id.raw());
            true
        } else {
            false
        }
    }

    fn drain_actions(&mut self) {
        if let Some((id, sel)) = self.ui.take_button_group_change() {
            if id == self.group {
                eprintln!("button_group → {sel:?}");
            }
        }
        if let Some(id) = self.ui.take_link_activate() {
            self.clicks = self.clicks.saturating_add(1);
            eprintln!("link activate → {}", id.raw());
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
                let _ = ctx.present(w, h, pixels);
            }
        }
    }

    fn maybe_qa_dump(&mut self, ctx: &mut WindowCtx<'_>) {
        if !self.qa_exit || self.qa_dumps >= 1 {
            return;
        }
        if let Some((pixels, w, h)) = self.backend.frame_pixels() {
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tmp/demo-shots/26-office_p2.ppm"
            );
            match write_ppm(path, pixels, w, h) {
                Ok(()) => {
                    eprintln!("QA wrote {path} ({w}x{h})");
                    self.qa_dumps += 1;
                    ctx.request_exit();
                }
                Err(e) => eprintln!("QA dump failed: {e}"),
            }
        }
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        let _ = self.window_backend;
        let _ = GpuRenderer::try_new(w, h, CLEAR).map(|gpu| {
            self.gpu_stash = Some(gpu);
        });
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!("next-gui Office P2a — {}", self.backend.name()));
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
                let xf = x as f32;
                let yf = y as f32;
                let mut handled = self.ui.pointer_pressed(&mut self.scene, xf, yf);
                if let Some(hit) = self.ui.hit_widget(&self.scene, xf, yf) {
                    if self.apply_link_hit(hit) {
                        handled = true;
                    }
                }
                self.drain_actions();
                if handled {
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
                        self.drain_actions();
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
                            "next-gui Office P2a — {}",
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
        self.maybe_qa_dump(ctx);
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui Office P2a", 720, 520);
    let demo = Demo::new(attrs.width, attrs.height, attrs.present_backend);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed: {e}");
        std::process::exit(1);
    }
}
