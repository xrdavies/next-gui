//! Office P2a–P2e demo: through `grid` / `timeline` / `calendar` / `image` /
//! `config_provider` (Ui API).
//!
//! Tab focuses group / links / steps / result / popover / popconfirm /
//! upload / color / autocomplete / timeline / calendar; ←/→ changes group,
//! steps, color, or calendar day; Enter/Space on a link or result drains
//! activate/action; Enter/Space on popover/popconfirm toggles open;
//! Enter/Space on upload → Browse (demo stubs a path). Pointer uses the
//! Button dual path for links (`hit_widget`). Space Soft↔GPU when nothing
//! focused.
//!
//! `NEXT_GUI_P2A_QA=1` … `NEXT_GUI_P2E_QA=1` dumps a Soft PPM after first
//! paint (then exits — no hang without interaction).

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
    AutocompleteStyle, ButtonGroupStyle, CalendarStyle, ColorPickerStyle, GridStyle,
    ImageStyle, LinkStyle, PanelStyle, PopconfirmAction, PopconfirmStyle, PopoverStyle,
    ResultAction, ResultStatus, ResultStyle, SpaceDirection, SpaceStyle, StatisticStyle, StepItem,
    StepsStyle, StatusBarStyle, TimelineItem, TimelineStyle, ToolbarStyle, Ui, UiConfig, UiDensity,
    UiKey, UploadStyle, WidgetId,
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
    if let KeyCode::Char(_c) = key {
        if modifiers.ctrl || modifiers.alt || modifiers.meta {
            return None;
        }
    }
    Some(match key {
        KeyCode::Left => UiKey::Left,
        KeyCode::Right => UiKey::Right,
        KeyCode::Up => UiKey::Up,
        KeyCode::Down => UiKey::Down,
        KeyCode::Enter => UiKey::Enter,
        KeyCode::Space => UiKey::Space,
        KeyCode::Escape => UiKey::Escape,
        KeyCode::Backspace => UiKey::Backspace,
        KeyCode::Delete => UiKey::Delete,
        KeyCode::Char(c) => UiKey::Char(c),
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
    steps: WidgetId,
    result: WidgetId,
    popover: WidgetId,
    popconfirm: WidgetId,
    upload: WidgetId,
    color: WidgetId,
    autocomplete: WidgetId,
    timeline: WidgetId,
    calendar: WidgetId,
    clicks: u32,
    result_actions: u32,
    popconfirm_actions: u32,
    upload_stubs: u32,
    qa_dumps: u32,
    qa_exit: bool,
}

impl Demo {
    fn new(width: u32, height: u32, window_backend: PresentBackend) -> Self {
        let mut ui = Ui::new();
        let theme = *ui.theme();
        let mut status = WidgetId(0);
        let mut group = WidgetId(0);
        let mut link_docs = WidgetId(0);
        let mut link_help = WidgetId(0);
        let mut steps = WidgetId(0);
        let mut result = WidgetId(0);
        let mut popover = WidgetId(0);
        let mut popconfirm = WidgetId(0);
        let mut upload = WidgetId(0);
        let mut color = WidgetId(0);
        let mut autocomplete = WidgetId(0);
        let mut timeline = WidgetId(0);
        let mut calendar = WidgetId(0);

        ui.set_config(UiConfig {
            locale: "zh-CN".into(),
            density: UiDensity::Default,
            accent: Some(Color::rgb(0x6a, 0xb0, 0xf0)),
        });

        ui.toolbar(
            ToolbarStyle {
                width: width as f32 - 16.0,
                height: 40.0,
                ..ToolbarStyle::from_theme(&theme)
            },
            |bar| {
                bar.text("Office P2a–P2e");
                bar.spacer(12.0, 1.0);
                bar.text_styled(
                    "Tab · ←/→ · Enter · Esc closes pop · Space Soft↔GPU",
                    Color::rgb(0x90, 0x98, 0xa4),
                    12.0,
                );
            },
        );

        ui.panel(
            "P2 widgets",
            PanelStyle {
                x: 16.0,
                y: 56.0,
                width: width as f32 - 32.0,
                height: height as f32 - 100.0,
                ..PanelStyle::from_theme(&theme)
            },
            |panel| {
                panel.text("P2a — button_group / link / space");
                group = panel.button_group(
                    ["Day", "Week", "Month"],
                    Some(0),
                    ButtonGroupStyle::from_theme(&theme),
                );
                panel.spacer(1.0, 8.0);
                panel.space(
                    SpaceStyle {
                        direction: SpaceDirection::Horizontal,
                        gap: 20.0,
                        width: width as f32 - 64.0,
                        height: 36.0,
                        cross_align: CrossAlign::Center,
                        ..SpaceStyle::from_theme(&theme)
                    },
                    |row| {
                        link_docs = row.link("Open docs");
                        link_help = row.link_styled(
                            "Help center",
                            LinkStyle {
                                underline: true,
                                fg: Color::rgb(0x90, 0xd0, 0xa0),
                                ..LinkStyle::from_theme(&theme)
                            },
                        );
                        row.button("Solid CTA");
                    },
                );
                panel.spacer(1.0, 12.0);
                panel.text("P2b — steps");
                steps = panel.steps(
                    [
                        StepItem::new("Plan"),
                        StepItem::with_description("Build", "implement"),
                        StepItem::new("Ship"),
                    ],
                    1,
                    StepsStyle {
                        width: width as f32 - 64.0,
                        height: 64.0,
                        clickable: true,
                        ..StepsStyle::from_theme(&theme)
                    },
                );
                panel.spacer(1.0, 12.0);
                panel.text("P2b — statistic (not focusable)");
                panel.space(
                    SpaceStyle {
                        direction: SpaceDirection::Horizontal,
                        gap: 12.0,
                        width: width as f32 - 64.0,
                        height: 96.0,
                        cross_align: CrossAlign::Start,
                        ..SpaceStyle::from_theme(&theme)
                    },
                    |row| {
                        row.statistic_full(
                            "Users",
                            "1,280",
                            None::<&str>,
                            None::<&str>,
                            Some("+12% MoM"),
                            StatisticStyle {
                                width: 140.0,
                                height: 88.0,
                                ..StatisticStyle::from_theme(&theme)
                            },
                        );
                        row.statistic_full(
                            "Revenue",
                            "42",
                            Some("¥"),
                            Some("k"),
                            Some("+3.1%"),
                            StatisticStyle {
                                width: 140.0,
                                height: 88.0,
                                ..StatisticStyle::from_theme(&theme)
                            },
                        );
                        row.statistic(
                            "Latency",
                            "18ms",
                            StatisticStyle {
                                width: 140.0,
                                height: 88.0,
                                ..StatisticStyle::from_theme(&theme)
                            },
                        );
                    },
                );
                panel.spacer(1.0, 12.0);
                panel.text("P2b — result");
                result = panel.result(
                    ResultStatus::Success,
                    "Order placed",
                    Some("Confirmation sent to your inbox."),
                    Some("Back home"),
                    Some("View order"),
                    ResultStyle {
                        width: width as f32 - 64.0,
                        height: 140.0,
                        ..ResultStyle::from_theme(&theme)
                    },
                );
                panel.spacer(1.0, 12.0);
                panel.text("P2c — popover / popconfirm (click trigger; Esc closes)");
                panel.space(
                    SpaceStyle {
                        direction: SpaceDirection::Horizontal,
                        gap: 16.0,
                        width: width as f32 - 64.0,
                        height: 40.0,
                        cross_align: CrossAlign::Center,
                        ..SpaceStyle::from_theme(&theme)
                    },
                    |row| {
                        popover = row.popover_titled(
                            "Details",
                            Some("Popover"),
                            "Click-toggle bubble · below host",
                            PopoverStyle::from_theme(&theme),
                        );
                        popconfirm = row.popconfirm(
                            "Delete",
                            "Remove this row permanently?",
                            PopconfirmStyle::from_theme(&theme),
                        );
                    },
                );
                panel.spacer(1.0, 12.0);
                panel.text("P2d — upload (Browse stubs path) / color_picker / autocomplete");
                upload = panel.upload(
                    "Choose a config file…",
                    UploadStyle {
                        width: width as f32 - 64.0,
                        ..UploadStyle::from_theme(&theme)
                    },
                );
                panel.spacer(1.0, 8.0);
                color = panel.color_picker(
                    Color::rgb(0x6a, 0xb0, 0xf0),
                    ColorPickerStyle {
                        width: width as f32 - 64.0,
                        ..ColorPickerStyle::from_theme(&theme)
                    },
                );
                panel.spacer(1.0, 8.0);
                autocomplete = panel.autocomplete_placeholder(
                    [
                        "Shanghai",
                        "Shenzhen",
                        "Beijing",
                        "Hangzhou",
                        "Guangzhou",
                        "Chengdu",
                    ],
                    "",
                    "City…",
                    AutocompleteStyle {
                        width: 280.0,
                        ..AutocompleteStyle::from_theme(&theme)
                    },
                );
                panel.spacer(1.0, 12.0);
                panel.text("P2e — grid / timeline / calendar / image (config via Ui::set_config)");
                panel.grid(
                    2,
                    GridStyle {
                        width: width as f32 - 64.0,
                        height: 72.0,
                        padding: 4.0,
                        gap: 8.0,
                        background: Some(Color::rgb(0x22, 0x22, 0x28)),
                        ..GridStyle::from_theme(&theme)
                    },
                    |g| {
                        g.text_styled("Grid A", Color::rgb(0xe0, 0xe4, 0xec), 13.0);
                        g.text_styled("Grid B", Color::rgb(0xe0, 0xe4, 0xec), 13.0);
                        g.text_styled("Grid C", Color::rgb(0xe0, 0xe4, 0xec), 13.0);
                        g.text_styled("Grid D", Color::rgb(0xe0, 0xe4, 0xec), 13.0);
                    },
                );
                panel.spacer(1.0, 8.0);
                panel.space(
                    SpaceStyle {
                        direction: SpaceDirection::Horizontal,
                        gap: 12.0,
                        width: width as f32 - 64.0,
                        height: 200.0,
                        cross_align: CrossAlign::Start,
                        ..SpaceStyle::from_theme(&theme)
                    },
                    |row| {
                        timeline = row.timeline(
                            [
                                TimelineItem::new("09:00", "Boot"),
                                TimelineItem::new("09:05", "Ready").with_color(0x6ac08a),
                                TimelineItem::new("09:10", "Sync"),
                            ],
                            TimelineStyle {
                                width: 220.0,
                                height: 140.0,
                                ..TimelineStyle::from_theme(&theme)
                            },
                        );
                        calendar = row.calendar(
                            2026,
                            10,
                            19,
                            CalendarStyle {
                                width: 260.0,
                                height: 240.0,
                                cell: 28.0,
                                ..CalendarStyle::from_theme(&theme)
                            },
                        );
                    },
                );
                panel.spacer(1.0, 8.0);
                // 4x3 RGBA gradient preview
                let mut px = Vec::with_capacity(4 * 3 * 4);
                for y in 0..3u8 {
                    for x in 0..4u8 {
                        px.extend_from_slice(&[x * 60, y * 80, 180, 255]);
                    }
                }
                panel.space(
                    SpaceStyle {
                        direction: SpaceDirection::Horizontal,
                        gap: 12.0,
                        width: width as f32 - 64.0,
                        height: 80.0,
                        ..SpaceStyle::from_theme(&theme)
                    },
                    |row| {
                        row.image_rgba_caption(
                            4,
                            3,
                            &px,
                            "RGBA mosaic",
                            ImageStyle {
                                width: 96.0,
                                height: 72.0,
                                mosaic_max: 4,
                                ..ImageStyle::from_theme(&theme)
                            },
                        );
                        row.image_placeholder(
                            "placeholder",
                            ImageStyle {
                                width: 96.0,
                                height: 72.0,
                                ..ImageStyle::from_theme(&theme)
                            },
                        );
                    },
                );
            },
        );

        ui.status_bar(
            StatusBarStyle {
                width: width as f32 - 16.0,
                height: 28.0,
                ..StatusBarStyle::from_theme(&theme)
            },
            |bar| {
                status = bar.text_styled("ready", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
            },
        );

        let mut scene = Scene::new();
        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);


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
            steps,
            result,
            popover,
            popconfirm,
            upload,
            color,
            autocomplete,
            timeline,
            calendar,
            clicks: 0,
            result_actions: 0,
            popconfirm_actions: 0,
            upload_stubs: 0,
            qa_dumps: 0,
            qa_exit: std::env::var_os("NEXT_GUI_P2A_QA").is_some()
                || std::env::var_os("NEXT_GUI_P2B_QA").is_some()
                || std::env::var_os("NEXT_GUI_P2C_QA").is_some()
                || std::env::var_os("NEXT_GUI_P2D_QA").is_some()
                || std::env::var_os("NEXT_GUI_P2E_QA").is_some(),
        }
    }

    fn refresh_status(&mut self) {
        let sel = self
            .ui
            .button_group_selected(self.group)
            .flatten()
            .map(|i| ["Day", "Week", "Month"][i])
            .unwrap_or("(none)");
        let step = self.ui.steps_current(self.steps).unwrap_or(0);
        self.ui.set_text(
            self.status,
            format!(
                "range={sel} · step={step} · links={} · result={} · pc={} · up={} · {} ({})",
                self.clicks,
                self.result_actions,
                self.popconfirm_actions,
                self.upload_stubs,
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
        if let Some((id, cur)) = self.ui.take_steps_change() {
            if id == self.steps {
                eprintln!("steps → {cur}");
            }
        }
        if let Some((id, action)) = self.ui.take_result_action() {
            if id == self.result {
                self.result_actions = self.result_actions.saturating_add(1);
                let name = match action {
                    ResultAction::Primary => "primary",
                    ResultAction::Secondary => "secondary",
                };
                eprintln!("result → {name}");
            }
        }
        if let Some((id, open)) = self.ui.take_popover_open_change() {
            if id == self.popover {
                eprintln!("popover open → {open}");
            }
        }
        if let Some((id, action)) = self.ui.take_popconfirm_action() {
            if id == self.popconfirm {
                self.popconfirm_actions = self.popconfirm_actions.saturating_add(1);
                let name = match action {
                    PopconfirmAction::Confirm => "confirm",
                    PopconfirmAction::Cancel => "cancel",
                    PopconfirmAction::Dismiss => "dismiss",
                };
                eprintln!("popconfirm → {name}");
            }
        }
        if let Some(id) = self.ui.take_upload_browse() {
            if id == self.upload {
                self.upload_stubs = self.upload_stubs.saturating_add(1);
                let stub = format!("/tmp/office-p2d-stub-{}.csv", self.upload_stubs);
                let _ = self.ui.set_upload_path(id, Some(stub.clone()));
                eprintln!("upload browse → stub {stub}");
            }
        }
        if let Some(id) = self.ui.take_upload_clear() {
            if id == self.upload {
                let _ = self.ui.set_upload_path(id, None::<String>);
                eprintln!("upload clear");
            }
        }
        if let Some((id, c)) = self.ui.take_color_change() {
            if id == self.color {
                eprintln!("color → #{:02X}{:02X}{:02X}", c.r, c.g, c.b);
            }
        }
        if let Some((id, v)) = self.ui.take_autocomplete_change() {
            if id == self.autocomplete {
                eprintln!("autocomplete → {v}");
            }
        }
        if let Some((id, idx)) = self.ui.take_timeline_select() {
            if id == self.timeline {
                eprintln!("timeline → {idx}");
            }
        }
        if let Some((id, date)) = self.ui.take_calendar_change() {
            if id == self.calendar {
                eprintln!(
                    "calendar → {:04}-{:02}-{:02}",
                    date.year, date.month, date.day
                );
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
        self.ui.set_viewport(w as f32, h as f32);
        let _ = self.window_backend;
        let _ = GpuRenderer::try_new(w, h, CLEAR).map(|gpu| {
            self.gpu_stash = Some(gpu);
        });
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!("next-gui Office P2 — {}", self.backend.name()));
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.ui.set_viewport(width as f32, height as f32);
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
                            "next-gui Office P2 — {}",
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
    let attrs = WindowAttrs::new("next-gui Office P2", 720, 1480);
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
