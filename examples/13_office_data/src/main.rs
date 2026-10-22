//! Office P0 data demo: List, Tree, Table.
//!
//! Click to select; ↑↓ navigate (Tree also ←→ expand/collapse). Wheel / drag to scroll.
//! Space toggles Soft↔GPU when nothing focused. Escape / close to quit.

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
    ColumnStyle, ListStyle, PanelStyle, SplitterStyle, StatusBarStyle, TableColumn, TableStyle,
    ToolbarStyle, TreeNode, TreeStyle, Ui, UiKey, WidgetId,
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
    list: WidgetId,
    tree: WidgetId,
    table: WidgetId,
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
        let mut list = WidgetId(0);
        let mut tree = WidgetId(0);
        let mut table = WidgetId(0);
        let mut status = WidgetId(0);
        let mut mode_label = WidgetId(0);

        let shell_w = (width as f32 - 24.0).max(720.0);
        let shell_h = (height as f32 - 24.0).max(520.0);

        let plant = vec![TreeNode::new(
            1,
            "Plant North",
            vec![
                TreeNode::new(
                    10,
                    "Substation A",
                    vec![
                        TreeNode::leaf(11, "Bay-01"),
                        TreeNode::leaf(12, "Bay-02"),
                        TreeNode::new(
                            13,
                            "Bay-03",
                            vec![TreeNode::leaf(131, "CB-31"), TreeNode::leaf(132, "DS-31")],
                        ),
                    ],
                ),
                TreeNode::new(
                    20,
                    "Substation B",
                    vec![TreeNode::leaf(21, "Bay-11"), TreeNode::leaf(22, "Bay-12")],
                ),
            ],
        )];

        let tags: Vec<String> = (1..=20).map(|i| format!("TAG-{i:03} · feeder")).collect();

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
                            "Office data — List / Tree / Table",
                            Color::rgb(0xe0, 0xe4, 0xec),
                            14.0,
                        );
                    },
                );

                col.splitter(
                    SplitterStyle {
                        width: shell_w - 16.0,
                        height: shell_h - 80.0,
                        axis: Axis::Vertical,
                        ratio: 0.32,
                        ..SplitterStyle::from_theme(&theme)
                    },
                    |left| {
                        left.panel(
                            "Hierarchy",
                            PanelStyle {
                                width: 240.0,
                                height: shell_h - 90.0,
                                ..PanelStyle::from_theme(&theme)
                            },
                            |p| {
                                tree = p.tree(
                                    plant.clone(),
                                    TreeStyle {
                                        width: 220.0,
                                        height: (shell_h - 160.0).max(200.0),
                                        ..TreeStyle::from_theme(&theme)
                                    },
                                );
                            },
                        );
                    },
                    |right| {
                        right.column(
                            ColumnStyle {
                                width: 480.0,
                                height: shell_h - 90.0,
                                padding: 0.0,
                                gap: 8.0,
                                background: None,
                                ..ColumnStyle::from_theme(&theme)
                            },
                            |c| {
                                c.panel(
                                    "Tag list",
                                    PanelStyle {
                                        width: 460.0,
                                        height: 200.0,
                                        ..PanelStyle::from_theme(&theme)
                                    },
                                    |p| {
                                        list = p.list(
                                            tags.clone(),
                                            ListStyle {
                                                width: 430.0,
                                                height: 150.0,
                                                ..ListStyle::from_theme(&theme)
                                            },
                                        );
                                    },
                                );
                                c.panel(
                                    "Measurements",
                                    PanelStyle {
                                        width: 460.0,
                                        height: (shell_h - 320.0).max(180.0),
                                        ..PanelStyle::from_theme(&theme)
                                    },
                                    |p| {
                                        table = p.table(
                                            vec![
                                                TableColumn::new("Tag", 110.0),
                                                TableColumn::new("Status", 90.0),
                                                TableColumn::new("kV", 70.0),
                                                TableColumn::new("A", 70.0),
                                            ],
                                            vec![
                                                vec![
                                                    "CB-31".into(),
                                                    "Closed".into(),
                                                    "110.2".into(),
                                                    "320".into(),
                                                ],
                                                vec![
                                                    "DS-31".into(),
                                                    "Open".into(),
                                                    "0.0".into(),
                                                    "0".into(),
                                                ],
                                                vec![
                                                    "CB-11".into(),
                                                    "Closed".into(),
                                                    "109.8".into(),
                                                    "280".into(),
                                                ],
                                                vec![
                                                    "VT-01".into(),
                                                    "OK".into(),
                                                    "110.0".into(),
                                                    "—".into(),
                                                ],
                                                vec![
                                                    "CT-01".into(),
                                                    "OK".into(),
                                                    "—".into(),
                                                    "315".into(),
                                                ],
                                                vec![
                                                    "CB-12".into(),
                                                    "Fault".into(),
                                                    "0.0".into(),
                                                    "0".into(),
                                                ],
                                                vec![
                                                    "DS-12".into(),
                                                    "Closed".into(),
                                                    "110.1".into(),
                                                    "0".into(),
                                                ],
                                                vec![
                                                    "Bus-A".into(),
                                                    "Live".into(),
                                                    "110.0".into(),
                                                    "—".into(),
                                                ],
                                            ],
                                            TableStyle {
                                                width: 430.0,
                                                height: (shell_h - 380.0).max(140.0),
                                                ..TableStyle::from_theme(&theme)
                                            },
                                        );
                                    },
                                );
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
                            "↑↓ select · Tree ←→ expand · wheel/drag scroll · Space Soft↔GPU",
                            Color::rgb(0x88, 0x8e, 0x98),
                            12.0,
                        );
                    },
                );
            },
        );

        ui.set_list_selected(list, Some(0));
        ui.set_tree_selected(tree, Some(1));
        ui.set_table_selected_row(table, Some(0));
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
            list,
            tree,
            table,
            status,
            mode_label,
            needs_paint: true,
            pending_native: window_backend == PresentBackend::GpuSurface,
        }
    }

    fn refresh_status(&mut self) {
        let li = self.ui.list_selected(self.list);
        let ti = self.ui.tree_selected(self.tree);
        let tr = self.ui.table_selected_row(self.table);
        self.ui.set_text(
            self.status,
            format!("list={li:?} tree={ti:?} table_row={tr:?}"),
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
                        "e13_office_data: native Surface ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("e13_office_data: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, CLEAR) {
                Ok(gpu) => {
                    eprintln!(
                        "e13_office_data: GPU readback ready ({})",
                        gpu.backend_name()
                    );
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("e13_office_data: GPU unavailable ({e}); Soft only"),
            }
        }
        self.refresh_status();
        self.scene.mark_full_dirty(w as f32, h as f32);
        eprintln!(
            "e13_office_data ready {}x{} backend={} ({})",
            w,
            h,
            self.backend.name(),
            self.backend.detail()
        );
        ctx.set_title(&format!("next-gui Office data — {}", self.backend.name()));
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
                if let Some(ui_key) = map_key(key, modifiers) {
                    if self.ui.key_input(ui_key) {
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
                            "next-gui Office data — {}",
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
        eprintln!("e13_office_data: NEXT_GUI_GPU_SURFACE → GpuSurface");
        WindowAttrs::new("next-gui Office data", 900, 640).with_gpu_surface()
    } else {
        eprintln!(
            "e13_office_data: Softbuffer window (set NEXT_GUI_GPU_SURFACE=1 for native Surface)"
        );
        WindowAttrs::new("next-gui Office data", 900, 640)
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
