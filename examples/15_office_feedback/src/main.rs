//! Office P0 feedback demo: Tag, Badge, Tooltip, Modal, Progress, Alert.
//!
//! Hover icons for tooltips; click Alert × to dismiss; Open Modal / Escape / ×;
//! Progress advances on Open Modal. Space toggles Soft↔GPU when idle.

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
    AlertStyle, AlertTone, BadgeKind, BadgeStyle, ColumnStyle, IconName, ModalAction, ModalStyle,
    PanelStyle, ProgressStyle, RowStyle, StatusBarStyle, TagStyle, ToolbarStyle, TooltipStyle, Ui,
    UiKey, WidgetId,
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
        KeyCode::Insert => UiKey::Insert,
        KeyCode::Left => UiKey::Left,
        KeyCode::Right => UiKey::Right,
        KeyCode::Up => UiKey::Up,
        KeyCode::Down => UiKey::Down,
        KeyCode::Home => UiKey::Home,
        KeyCode::End => UiKey::End,
        KeyCode::PageUp => UiKey::PageUp,
        KeyCode::PageDown => UiKey::PageDown,
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
    tag: WidgetId, // closable demo
    badge: WidgetId,
    tip_save: WidgetId,
    progress: WidgetId,
    alert: WidgetId,
    modal: WidgetId,
    open_btn: WidgetId,
    bump_btn: WidgetId,
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
        let mut tag = WidgetId(0);
        let mut badge = WidgetId(0);
        let mut tip_save = WidgetId(0);
        let mut progress = WidgetId(0);
        let mut alert = WidgetId(0);
        let mut open_btn = WidgetId(0);
        let mut bump_btn = WidgetId(0);
        let mut status = WidgetId(0);
        let mut log = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(720.0);
        let shell_h = (height as f32 - 24.0).max(520.0);
        ui.set_viewport(width as f32, height as f32);

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
                        tip_save = tb.tooltip_styled(
                            "Save project (tooltip)",
                            TooltipStyle::from_theme(&theme),
                            |t| {
                                t.icon(IconName::Save);
                            },
                        );
                        tb.spacer(8.0, 1.0);
                        tb.tooltip("Open settings", |t| {
                            t.icon(IconName::Settings);
                        });
                        tb.spacer(12.0, 1.0);
                        tb.text_styled(
                            "Office feedback — Tag / Badge / Tooltip / Modal / Progress / Alert",
                            Color::rgb(0xa0, 0xa8, 0xb4),
                            12.0,
                        );
                    },
                );

                alert = col.alert_styled(
                    AlertTone::Warning,
                    "Bay-03 voltage near threshold — acknowledge or dismiss.",
                    true,
                    AlertStyle {
                        width: shell_w - 16.0,
                        ..AlertStyle::from_theme(&theme)
                    },
                );

                col.panel(
                    "Status chips",
                    PanelStyle {
                        width: shell_w - 16.0,
                        height: 100.0,
                        ..PanelStyle::from_theme(&theme)
                    },
                    |p| {
                        p.row(
                            RowStyle {
                                width: shell_w - 48.0,
                                height: 36.0,
                                gap: 10.0,
                                cross_align: CrossAlign::Center,
                                ..RowStyle::from_theme(&theme)
                            },
                            |r| {
                                tag = r.tag_styled(
                                    "Running",
                                    true,
                                    TagStyle {
                                        bg: Color::rgb(0x1e, 0x3d, 0x2a),
                                        fg: Color::rgb(0x8a, 0xe6, 0x9a),
                                        ..TagStyle::from_theme(&theme)
                                    },
                                );
                                r.tag_styled(
                                    "Bypass",
                                    false,
                                    TagStyle {
                                        bg: Color::rgb(0x4a, 0x3a, 0x18),
                                        fg: Color::rgb(0xf0, 0xd0, 0x80),
                                        ..TagStyle::from_theme(&theme)
                                    },
                                );
                                r.tag_styled(
                                    "Fault",
                                    false,
                                    TagStyle {
                                        bg: Color::rgb(0x4a, 0x22, 0x28),
                                        fg: Color::rgb(0xf0, 0xa0, 0xa8),
                                        ..TagStyle::from_theme(&theme)
                                    },
                                );
                                r.spacer(16.0, 1.0);
                                r.badge_host(
                                    BadgeKind::Count(7),
                                    BadgeStyle::from_theme(&theme),
                                    |h| {
                                        h.icon(IconName::Warning);
                                    },
                                );
                                badge = r.badge_count(3);
                                r.badge_dot();
                            },
                        );
                    },
                );

                col.panel(
                    "Progress & dialog",
                    PanelStyle {
                        width: shell_w - 16.0,
                        height: 160.0,
                        ..PanelStyle::from_theme(&theme)
                    },
                    |p| {
                        p.text_styled("Download / command progress", Color::rgb(0xa0, 0xa8, 0xb4), 12.0);
                        progress = p.progress_styled(
                            0.35,
                            false,
                            ProgressStyle {
                                width: shell_w - 64.0,
                                ..ProgressStyle::from_theme(&theme)
                            },
                        );
                        p.row(
                            RowStyle {
                                width: shell_w - 48.0,
                                height: 40.0,
                                gap: 10.0,
                                cross_align: CrossAlign::Center,
                                ..RowStyle::from_theme(&theme)
                            },
                            |r| {
                                open_btn = r.tool_button("Open Modal");
                                bump_btn = r.tool_button("+10%");
                            },
                        );
                        log = p.text_styled(
                            "Hover toolbar icons · dismiss alert · open modal · bump progress.",
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
                            "Esc closes modal · Space Soft↔GPU",
                            Color::rgb(0x80, 0x88, 0x94),
                            12.0,
                        );
                    },
                );
            },
        );

        let modal = ui.modal(
            "Confirm delete",
            "Remove tag Running from the monitoring list?",
            "Delete",
            Some("Cancel".into()),
            ModalStyle::from_theme(&theme),
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
            tag,
            badge,
            tip_save,
            progress,
            alert,
            modal,
            open_btn,
            bump_btn,
            status,
            log,
            needs_paint: true,
            pending_native: matches!(window_backend, PresentBackend::GpuSurface),
        }
    }

    fn refresh_status(&mut self) {
        let prog = self.ui.progress_value(self.progress).unwrap_or(0.0);
        let alert_gone = self.ui.alert_dismissed(self.alert).unwrap_or(false);
        let modal_open = self.ui.modal_open(self.modal).unwrap_or(false);
        let _ = (self.tag, self.tip_save); // retained for hit / tooltip demos
        let msg = format!(
            "progress={:.0}%  alert={}  modal={}  backend={}",
            prog * 100.0,
            if alert_gone { "dismissed" } else { "visible" },
            if modal_open { "open" } else { "closed" },
            self.backend.name()
        );
        self.ui.set_text(self.log, msg);
        self.ui.set_text(
            self.status,
            format!("{} ({})", self.backend.name(), self.backend.detail()),
        );
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }


    fn apply_button(&mut self, id: next_gui_ui::WidgetId) -> bool {
        if id == self.open_btn {
            let _ = self.ui.set_modal_open(self.modal, true);
            true
        } else if id == self.bump_btn {
            let v = self.ui.progress_value(self.progress).unwrap_or(0.0);
            let _ = self.ui.set_progress_value(self.progress, (v + 0.1).min(1.0));
            let _ = self.ui.set_badge_kind(
                self.badge,
                next_gui_ui::BadgeKind::Count(((v + 0.1) * 10.0) as u32),
            );
            true
        } else {
            false
        }
    }

    fn handle_feedback_actions(&mut self) {
        if let Some(id) = self.ui.take_alert_dismiss() {
            self.ui.set_text(self.log, format!("Alert dismissed ({})", id.raw()));
        }
        if let Some(id) = self.ui.take_tag_close() {
            self.ui.set_text(self.log, format!("Tag close clicked ({})", id.raw()));
        }
        if let Some((id, action)) = self.ui.take_modal_action() {
            let label = match action {
                ModalAction::Primary => "Delete",
                ModalAction::Secondary => "Cancel",
                ModalAction::Tertiary => "Third action",
                ModalAction::Dismiss => "Dismiss",
            };
            self.ui.set_text(
                self.log,
                format!("Modal {} → {label}", id.raw()),
            );
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
                        "e15_office_feedback: native Surface ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("e15_office_feedback: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!(
                        "e15_office_feedback: GPU readback ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("e15_office_feedback: GPU unavailable ({e}); Soft only"),
            }
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!(
            "e15_office_feedback ready {}x{} backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        ctx.set_title(&format!("next-gui Office feedback — {}", self.backend.name()));
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
                let xf = x as f32;
                let yf = y as f32;
                let mut handled = self.ui.pointer_press(&mut self.scene, xf, yf, false);
                if let Some(hit) = self.ui.hit_widget(&self.scene, xf, yf) {
                    if self.apply_button(hit) {
                        handled = true;
                    }
                }
                if handled {
                    self.handle_feedback_actions();
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
                    self.handle_feedback_actions();
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
                        if let Some(id) = self.ui.take_button_activate() {
                            let _ = self.apply_button(id);
                        }
                        self.handle_feedback_actions();
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
                            "next-gui Office feedback — {}",
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
        eprintln!("e15_office_feedback: NEXT_GUI_GPU_SURFACE → GpuSurface");
        WindowAttrs::new("next-gui Office feedback", 900, 640).with_gpu_surface()
    } else {
        eprintln!(
            "e15_office_feedback: Softbuffer window (set NEXT_GUI_GPU_SURFACE=1 for native Surface)"
        );
        WindowAttrs::new("next-gui Office feedback", 900, 640)
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
