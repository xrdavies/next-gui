//! Office P0 input demo: ScrollArea, TextInput, Checkbox, Radio, Switch.
//!
//! Click fields to focus/toggle. Type ASCII or use IME for CJK into TextInput
//! (Ctrl/Cmd+A select-all, Ctrl/Cmd+C/V copy/paste via platform clipboard).
//! Wheel or drag inside the scroll list. Space toggles focused Checkbox/Switch/Radio,
//! or Soft↔GPU when nothing focused. Escape / close to quit.
//!
//! IME/CJK: focus a TextInput, enable a system IME (e.g. fcitx5/ibus), type to compose;
//! commit inserts CJK. Preedit shows underlined inline; candidate UI is OS-drawn.

use next_gui_layout::CrossAlign;
use next_gui_platform::{
    AppCallbacks, Event, ImeEvent, KeyCode, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{
    CheckboxStyle, ColumnStyle, PanelStyle, RadioStyle, ScrollAreaStyle, StatusBarStyle,
    SwitchStyle, TextInputStyle, ToolbarStyle, Ui, UiKey, WidgetId,
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
            match c {
                'a' | 'A' => return Some(UiKey::SelectAll),
                // Ctrl/Cmd+C/V handled in on_event via WindowCtx clipboard.
                'c' | 'C' | 'v' | 'V' | 'x' | 'X' => return None,
                _ => return None, // do not insert chars while a command modifier is held
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
        KeyCode::Other(_) => return None,
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
    name_input: WidgetId,
    tag_input: WidgetId,
    check: WidgetId,
    radio_a: WidgetId,
    radio_b: WidgetId,
    radio_c: WidgetId,
    sw: WidgetId,
    status: WidgetId,
    mode_label: WidgetId,
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
        let mut name_input = WidgetId(0);
        let mut tag_input = WidgetId(0);
        let mut check = WidgetId(0);
        let mut radio_a = WidgetId(0);
        let mut radio_b = WidgetId(0);
        let mut radio_c = WidgetId(0);
        let mut sw = WidgetId(0);
        let mut status = WidgetId(0);
        let mut mode_label = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(640.0);
        let shell_h = (height as f32 - 24.0).max(480.0);

        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 10.0,
                width: shell_w,
                height: shell_h,
                padding: 8.0,
                gap: 8.0,
                cross_align: CrossAlign::Start,
                ..ColumnStyle::from_theme(&theme)
            },
            |col| {
                col.toolbar(
                    ToolbarStyle {
                        width: shell_w - 16.0,
                        height: 36.0,
                        ..ToolbarStyle::from_theme(&theme)
                    },
                    |tb| {
                        tb.text_styled(
                            "Office inputs — ScrollArea / TextInput / Checkbox / Radio / Switch",
                            Color::rgb(0xe0, 0xe4, 0xec),
                            14.0,
                        );
                    },
                );

                col.panel(
                    "Form",
                    PanelStyle {
                        width: shell_w - 16.0,
                        height: shell_h - 80.0,
                        ..PanelStyle::from_theme(&theme)
                    },
                    |p| {
                        p.text_styled("Name", Color::rgb(0xa8, 0xb0, 0xbc), 13.0);
                        name_input = p.text_input_styled(
                            "",
                            "Plant name…",
                            TextInputStyle {
                                width: (shell_w - 60.0).min(420.0),
                                ..TextInputStyle::from_theme(&theme)
                            },
                        );
                        p.spacer(1.0, 6.0);
                        p.text_styled("Tag", Color::rgb(0xa8, 0xb0, 0xbc), 13.0);
                        tag_input = p.text_input_styled(
                            "BAY-01",
                            "Tag id",
                            TextInputStyle {
                                width: (shell_w - 60.0).min(420.0),
                                ..TextInputStyle::from_theme(&theme)
                            },
                        );
                        p.spacer(1.0, 10.0);
                        check = p.checkbox_styled(
                            "Alarm enabled",
                            true,
                            CheckboxStyle::from_theme(&theme),
                        );
                        p.spacer(1.0, 6.0);
                        p.text_styled("Priority", Color::rgb(0xa8, 0xb0, 0xbc), 13.0);
                        radio_a = p.radio_styled(7, "Low", false, RadioStyle::from_theme(&theme));
                        radio_b = p.radio_styled(7, "Medium", true, RadioStyle::from_theme(&theme));
                        radio_c = p.radio_styled(7, "High", false, RadioStyle::from_theme(&theme));
                        p.spacer(1.0, 8.0);
                        sw = p.switch_styled("Remote control", false, SwitchStyle::from_theme(&theme));
                        p.spacer(1.0, 12.0);
                        p.text_styled(
                            "Scroll list (wheel / drag)",
                            Color::rgb(0xa8, 0xb0, 0xbc),
                            13.0,
                        );
                        p.scroll_area(
                            ScrollAreaStyle {
                                width: (shell_w - 60.0).min(420.0),
                                height: 140.0,
                                ..ScrollAreaStyle::from_theme(&theme)
                            },
                            |sa| {
                                for i in 1..=24 {
                                    sa.text_styled(
                                        format!("Log entry #{i:02} — sample row for ScrollArea"),
                                        Color::rgb(0xc0, 0xc4, 0xcc),
                                        13.0,
                                    );
                                }
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
                        status = sb.text_styled("Ready", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
                        sb.spacer(16.0, 1.0);
                        mode_label = sb.text_styled("Soft", Color::rgb(0xa0, 0xa8, 0xb4), 12.0);
                        sb.spacer(16.0, 1.0);
                        sb.text_styled(
                            "IME on · focus field + system IME for CJK · Space=toggle / Soft↔GPU",
                            Color::rgb(0x88, 0x8e, 0x98),
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
            name_input,
            tag_input,
            check,
            radio_a,
            radio_b,
            radio_c,
            sw,
            status,
            mode_label,
            needs_paint: true,
            pending_native: window_backend == PresentBackend::GpuSurface,
        }
    }

    fn sync_ime(&mut self, ctx: &mut WindowCtx<'_>) {
        let want = self.ui.text_input_wants_ime();
        ctx.set_ime_allowed(want);
        if want {
            if let Some((x, y, w, h)) = self.ui.ime_cursor_area(&self.scene) {
                ctx.set_ime_cursor_area(x as f64, y as f64, w as f64, h as f64);
            }
        }
    }

    fn refresh_status(&mut self) {
        let name = self
            .ui
            .text_input_value(self.name_input)
            .unwrap_or("")
            .to_string();
        let tag = self
            .ui
            .text_input_value(self.tag_input)
            .unwrap_or("")
            .to_string();
        let alarm = self.ui.is_checked(self.check).unwrap_or(false);
        let prio = if self.ui.is_checked(self.radio_a) == Some(true) {
            "Low"
        } else if self.ui.is_checked(self.radio_b) == Some(true) {
            "Medium"
        } else if self.ui.is_checked(self.radio_c) == Some(true) {
            "High"
        } else {
            "?"
        };
        let remote = self.ui.is_checked(self.sw).unwrap_or(false);
        self.ui.set_text(
            self.status,
            format!("name={name:?} tag={tag:?} alarm={alarm} prio={prio} remote={remote}"),
        );
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
                } else if let Some(mut gpu) = self.surface_gpu.take() {
                    gpu.resize(w, h);
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                } else {
                    eprintln!("toggle: no native GPU available");
                    return;
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
                        "e12_office_inputs: native Surface ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("e12_office_inputs: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!(
                        "e12_office_inputs: GPU readback ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("e12_office_inputs: GPU unavailable ({e}); Soft only"),
            }
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!(
            "e12_office_inputs ready {}x{} backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        ctx.set_title(&format!(
            "next-gui Office inputs — {}",
            self.backend.name()
        ));
        // NEXT_GUI_IME_QA=1: seed CJK + preedit for visual self-test without a live IME.
        if std::env::var_os("NEXT_GUI_IME_QA").is_some() {
            self.ui.set_focus(Some(self.name_input));
            let _ = self.ui.ime_commit("你好电厂");
            let _ = self.ui.ime_preedit("hao", Some((3, 3)));
            self.ui.sync(&mut self.scene);
            self.needs_paint = true;
            self.refresh_status();
        }
        self.sync_ime(ctx);
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
                if self.ui.pointer_pressed(&mut self.scene, x as f32, y as f32) {
                    self.sync_ime(ctx);
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
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
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
                if modifiers.commandish() {
                    if let KeyCode::Char(c) = key {
                        match c {
                            'c' | 'C' => {
                                if let Some(t) = self.ui.focused_text_for_copy() {
                                    if let Err(e) = ctx.clipboard_set_text(&t) {
                                        eprintln!("clipboard set: {e}");
                                    } else {
                                        self.refresh_status();
                                        ctx.request_redraw();
                                    }
                                }
                                return;
                            }
                            'v' | 'V' => {
                                match ctx.clipboard_get_text() {
                                    Ok(Some(t)) if self.ui.paste_into_focused_text(&t) => {
                                        self.sync_ime(ctx);
                                        self.refresh_status();
                                        ctx.request_redraw();
                                    }
                                    Ok(_) => {}
                                    Err(e) => eprintln!("clipboard get: {e}"),
                                }
                                return;
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(ui_key) = map_key(key, modifiers) {
                    if self.ui.key_input(ui_key) {
                        self.sync_ime(ctx);
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
                            "next-gui Office inputs — {}",
                            self.backend.name()
                        ));
                        ctx.request_redraw();
                    }
                }
            }
            Event::Ime { ime, .. } => {
                let handled = match ime {
                    ImeEvent::Enabled => false,
                    ImeEvent::Preedit { text, cursor } => self.ui.ime_preedit(text, cursor),
                    ImeEvent::Commit { text } => self.ui.ime_commit(text),
                    ImeEvent::Disabled => self.ui.ime_clear_preedit(),
                };
                self.sync_ime(ctx);
                if handled {
                    self.refresh_status();
                    ctx.request_redraw();
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
        eprintln!("e12_office_inputs: NEXT_GUI_GPU_SURFACE → GpuSurface");
        WindowAttrs::new("next-gui Office inputs", 720, 640).with_gpu_surface()
    } else {
        eprintln!(
            "e12_office_inputs: Softbuffer window (set NEXT_GUI_GPU_SURFACE=1 for native Surface)"
        );
        WindowAttrs::new("next-gui Office inputs", 720, 640)
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
