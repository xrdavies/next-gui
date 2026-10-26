//! Office P0 nav demo: Menu, ContextMenu, Tabs, Select, Slider.
//!
//! Left-click Menu / Select to open; right-click ContextMenu host; drag Slider;
//! click Tabs to switch. Escape closes popup. Space toggles Soft↔GPU when idle.
//! Escape / close to quit.

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
    ColumnStyle, ContextMenuStyle, MenuItem, MenuStyle, PanelStyle, RowStyle, SelectStyle,
    SliderStyle, StatusBarStyle, TabsStyle, ToolbarStyle, Ui, UiKey, WidgetId,
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

struct Demo {
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    menu: WidgetId,
    ctx: WidgetId,
    tabs: WidgetId,
    select: WidgetId,
    slider: WidgetId,
    status: WidgetId,
    log: WidgetId,
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
        let theme = *ui.theme();
        let mut menu = WidgetId(0);
        let mut ctx = WidgetId(0);
        let mut tabs = WidgetId(0);
        let mut select = WidgetId(0);
        let mut slider = WidgetId(0);
        let mut status = WidgetId(0);
        let mut log = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(720.0);
        let shell_h = (height as f32 - 24.0).max(520.0);

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
                        menu = tb.menu(
                            "File",
                            vec![
                                MenuItem::new(1, "Open Project"),
                                MenuItem::new(2, "Save"),
                                MenuItem::disabled(3, "Export…"),
                                MenuItem::new(4, "Quit"),
                            ],
                            MenuStyle::from_theme(&theme),
                        );
                        tb.menu(
                            "Edit",
                            vec![
                                MenuItem::new(20, "Undo"),
                                MenuItem::new(21, "Redo"),
                                MenuItem::new(22, "Preferences"),
                            ],
                            MenuStyle::from_theme(&theme),
                        );
                        tb.spacer(12.0, 1.0);
                        tb.text_styled(
                            "Office nav — Menu / Context / Tabs / Select / Slider",
                            Color::rgb(0xa0, 0xa8, 0xb4),
                            12.0,
                        );
                    },
                );

                col.row(
                    RowStyle {
                        width: shell_w - 16.0,
                        height: 200.0,
                        gap: 10.0,
                        padding: 0.0,
                        background: None,
                        cross_align: CrossAlign::Start,
                        ..RowStyle::from_theme(&theme)
                    },
                    |row| {
                        ctx = row.context_menu(
                            "Right-click inside",
                            vec![
                                MenuItem::new(100, "Inspect Tag"),
                                MenuItem::new(101, "Acknowledge"),
                                MenuItem::disabled(102, "Lock (admin)"),
                                MenuItem::new(103, "Copy Path"),
                            ],
                            ContextMenuStyle {
                                width: 280.0,
                                height: 180.0,
                                ..ContextMenuStyle::from_theme(&theme)
                            },
                            |c| {
                                c.panel(
                                    "Context host",
                                    PanelStyle {
                                        width: 260.0,
                                        height: 160.0,
                                        ..PanelStyle::from_theme(&theme)
                                    },
                                    |p| {
                                        p.text_styled(
                                            "Bay-03 / CB-31",
                                            Color::rgb(0xe0, 0xe4, 0xec),
                                            14.0,
                                        );
                                        p.text_styled(
                                            "Secondary-click for actions",
                                            Color::rgb(0x90, 0x98, 0xa4),
                                            12.0,
                                        );
                                    },
                                );
                            },
                        );

                        tabs = row.tabs(
                            TabsStyle {
                                width: (shell_w - 16.0 - 300.0).max(360.0),
                                height: 180.0,
                                ..TabsStyle::from_theme(&theme)
                            },
                            |tb| {
                                tb.page("General", |p| {
                                    p.text_styled(
                                        "Plant: North  ·  Profile: Office",
                                        Color::rgb(0xd0, 0xd4, 0xdc),
                                        13.0,
                                    );
                                    p.text_styled(
                                        "Use ←→ when focused to switch tabs.",
                                        Color::rgb(0x90, 0x98, 0xa4),
                                        12.0,
                                    );
                                });
                                tb.page("Alarms", |p| {
                                    p.text_styled(
                                        "No active alarms (demo).",
                                        Color::rgb(0x8a, 0xe6, 0x9a),
                                        13.0,
                                    );
                                });
                                tb.page("About", |p| {
                                    p.text_styled(
                                        "next-gui Office P0 navigation slice",
                                        Color::rgb(0xd0, 0xd4, 0xdc),
                                        13.0,
                                    );
                                });
                            },
                        );
                    },
                );

                col.panel(
                    "Selection & threshold",
                    PanelStyle {
                        width: shell_w - 16.0,
                        height: 160.0,
                        ..PanelStyle::from_theme(&theme)
                    },
                    |p| {
                        p.row(
                            RowStyle {
                                width: shell_w - 48.0,
                                height: 36.0,
                                gap: 12.0,
                                padding: 0.0,
                                background: None,
                                cross_align: CrossAlign::Center,
                                ..RowStyle::from_theme(&theme)
                            },
                            |r| {
                                r.text_styled("Mode", Color::rgb(0xa0, 0xa8, 0xb4), 13.0);
                                select = r.select(
                                    ["Auto", "Manual", "Off"],
                                    Some(0),
                                    "Choose…",
                                    SelectStyle {
                                        width: 160.0,
                                        ..SelectStyle::from_theme(&theme)
                                    },
                                );
                                r.spacer(24.0, 1.0);
                                r.text_styled("Threshold", Color::rgb(0xa0, 0xa8, 0xb4), 13.0);
                                slider = r.slider(
                                    0.0,
                                    100.0,
                                    42.0,
                                    SliderStyle {
                                        width: 220.0,
                                        ..SliderStyle::from_theme(&theme)
                                    },
                                );
                            },
                        );
                        log = p.text_styled(
                            "Ready — open File menu, right-click host, switch tabs, pick mode, drag slider.",
                            Color::rgb(0xc0, 0xc8, 0xd0),
                            13.0,
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
            menu,
            ctx,
            tabs,
            select,
            slider,
            status,
            log,
            needs_paint: true,
            pending_native: matches!(window_backend, PresentBackend::GpuSurface),
        }
    }

    fn refresh_status(&mut self) {
        let mode = match self.ui.select_selected(self.select) {
            Some(0) => "Auto",
            Some(1) => "Manual",
            Some(2) => "Off",
            _ => "—",
        };
        let tab = self.ui.tabs_selected(self.tabs).unwrap_or(0);
        let tab_name = ["General", "Alarms", "About"].get(tab).copied().unwrap_or("?");
        let thr = self.ui.slider_value(self.slider).unwrap_or(0.0);
        let msg = format!(
            "tab={tab_name}  mode={mode}  threshold={thr:.1}  backend={}",
            self.backend.name()
        );
        self.ui.set_text(self.log, msg);
        self.ui.set_text(
            self.status,
            format!("{} ({})", self.backend.name(), self.backend.detail()),
        );
        self.ui.sync(&mut self.scene);
        let (w, h) = (
            self.scene
                .get(self.scene.root())
                .map(|n| n.bounds.width)
                .unwrap_or(800.0),
            600.0,
        );
        let _ = (w, h);
        self.needs_paint = true;
    }

    fn handle_menu_action(&mut self) {
        if let Some((owner, item)) = self.ui.take_menu_action() {
            let src = if owner == self.menu {
                "File"
            } else if owner == self.ctx {
                "Context"
            } else {
                "Menu"
            };
            let label = match item {
                1 => "Open Project",
                2 => "Save",
                3 => "Export",
                4 => "Quit",
                20 => "Undo",
                21 => "Redo",
                22 => "Preferences",
                100 => "Inspect Tag",
                101 => "Acknowledge",
                102 => "Lock",
                103 => "Copy Path",
                _ => "?",
            };
            self.ui.set_text(
                self.log,
                format!("Activated {src} → {label} (id={item})"),
            );
            if item == 4 {
                eprintln!("Quit selected (demo does not exit automatically)");
            }
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
                        "e14_office_nav: native Surface ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("e14_office_nav: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!(
                        "e14_office_nav: GPU readback ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("e14_office_nav: GPU unavailable ({e}); Soft only"),
            }
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!(
            "e14_office_nav ready {}x{} backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        ctx.set_title(&format!("next-gui Office nav — {}", self.backend.name()));
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
                if self.ui.pointer_press(&mut self.scene, x as f32, y as f32, false) {
                    self.handle_menu_action();
                    self.refresh_status();
                    ctx.request_redraw();
                }
            }
            Event::PointerButton {
                button: PointerButton::Right,
                pressed: true,
                x,
                y,
                ..
            } => {
                if self.ui.pointer_press(&mut self.scene, x as f32, y as f32, true) {
                    self.handle_menu_action();
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
                        self.handle_menu_action();
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
                            "next-gui Office nav — {}",
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
        self.needs_paint = false;
    }
}

fn main() {
    let use_native = std::env::var_os("NEXT_GUI_GPU_SURFACE").is_some()
        && GpuRenderer::adapter_available();
    let attrs = if use_native {
        eprintln!("e14_office_nav: NEXT_GUI_GPU_SURFACE → GpuSurface");
        WindowAttrs::new("next-gui Office nav", 900, 640).with_gpu_surface()
    } else {
        eprintln!(
            "e14_office_nav: Softbuffer window (set NEXT_GUI_GPU_SURFACE=1 for native Surface)"
        );
        WindowAttrs::new("next-gui Office nav", 900, 640)
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
