//! Soft demo: Theme::light / Theme::dark via `set_theme` + full Ui/Scene rebuild.
//!
//! Controls:
//! - Click **切换到亮色** / **切换到暗色**, or Space (when nothing consumes it): toggle theme.
//! - Tab / Shift+Tab: focus; Space / Enter: activate focused button.
//! - Escape / close: quit.
//!
//! ```bash
//! cargo run -p e27_theme_tokens
//! NEXT_GUI_THEME_QA=1 cargo run -p e27_theme_tokens   # dump /tmp/next-gui-theme.ppm then exit
//! ```
//!
//! `set_theme` does not rebuild existing widgets — this example replaces Ui + Scene
//! on every toggle so chrome picks up the new tokens.

use next_gui_platform::{
    AppCallbacks, Event, KeyCode, Modifiers, PointerButton, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::{load_default_cjk_font, Font};
use next_gui_ui::{
    Theme, ThemeMode, Ui, UiKey, WidgetId,
};

fn clear_for(theme: &Theme) -> Color {
    match theme.mode {
        ThemeMode::Dark => Color::rgb(0x1a, 0x1a, 0x1c),
        ThemeMode::Light => theme.colors.content_bg,
    }
}

fn mode_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Dark => "暗色",
        ThemeMode::Light => "亮色",
    }
}

fn toggle_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Dark => "切换到亮色",
        ThemeMode::Light => "切换到暗色",
    }
}

fn write_ppm(path: &str, pixels: &[u32], w: u32, h: u32) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "P6\n{w} {h}\n255")?;
    for &px in pixels {
        let r = ((px >> 16) & 0xff) as u8;
        let g = ((px >> 8) & 0xff) as u8;
        let b = (px & 0xff) as u8;
        f.write_all(&[r, g, b])?;
    }
    Ok(())
}

struct BuiltUi {
    ui: Ui,
    scene: Scene,
    btn_toggle: WidgetId,
}

fn build_ui(font: Font, theme: Theme, width: u32, height: u32) -> BuiltUi {
    let mut scene = Scene::new();
    let mut ui = Ui::with_font(font);
    ui.set_theme(theme);

    let mut btn_toggle = WidgetId(0);

    let win_w = width as f32;
    let win_h = height as f32;
    let content_h = (win_h - 36.0).max(200.0);

    let mut col_style = ui.column_style();
    col_style.x = 16.0;
    col_style.y = 12.0;
    col_style.width = (win_w - 32.0).max(320.0);
    col_style.height = content_h - 12.0;

    let mut toolbar_style = ui.toolbar_style();
    toolbar_style.width = col_style.width - col_style.padding * 2.0;
    toolbar_style.height = 40.0;

    let mut panel_style = ui.panel_style();
    panel_style.width = col_style.width - col_style.padding * 2.0;
    panel_style.height = 200.0;

    let mode = theme.mode;
    ui.column(col_style, |col| {
        col.text(format!("当前主题：{}", mode_label(mode)));
        col.text("样式来自 ui.*_style() / 未指定 Style 的控件构造；切换时整树重建。");
        btn_toggle = col.button(toggle_label(mode));

        col.toolbar(toolbar_style, |tb| {
            tb.text("工具条");
            tb.tag("Theme");
            tb.link("文档");
        });

        col.panel("示例面板", panel_style, |p| {
            p.text_input("在此输入…");
            p.checkbox("复选框", true);
            p.switch("开关", false);
            p.link("主题 tokens 链接");
        });
    });

    let mut status_style = ui.status_bar_style();
    status_style.x = 0.0;
    status_style.y = win_h - 32.0;
    status_style.width = win_w;
    status_style.height = 32.0;
    ui.status_bar(status_style, |bar| {
        bar.text(format!(
            "模式 {} · 空格切换 · Tab+空格/回车激活 · Esc 退出",
            mode_label(mode)
        ));
    });

    ui.sync(&mut scene);
    scene.mark_full_dirty(win_w, win_h);

    BuiltUi {
        ui,
        scene,
        btn_toggle,
    }
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

struct Demo {
    font: Font,
    theme: Theme,
    scene: Scene,
    soft: SoftRenderer,
    ui: Ui,
    btn_toggle: WidgetId,
    win_w: u32,
    win_h: u32,
    needs_paint: bool,
    frame: u64,
    qa_exit: bool,
    qa_done: bool,
}

impl Demo {
    fn new(width: u32, height: u32) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });
        let theme = Theme::dark();
        let clear = clear_for(&theme);
        let built = build_ui(font.clone(), theme, width, height);
        let qa_exit = std::env::var_os("NEXT_GUI_THEME_QA").is_some();

        Self {
            font,
            theme,
            scene: built.scene,
            soft: SoftRenderer::new(width, height, clear),
            ui: built.ui,
            btn_toggle: built.btn_toggle,
            win_w: width,
            win_h: height,
            needs_paint: true,
            frame: 0,
            qa_exit,
            qa_done: false,
        }
    }

    fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        let built = build_ui(self.font.clone(), theme, self.win_w, self.win_h);
        self.ui = built.ui;
        self.scene = built.scene;
        self.btn_toggle = built.btn_toggle;
        self.soft.set_clear_color(clear_for(&theme));
        self.needs_paint = true;
    }

    fn toggle_theme(&mut self) {
        let next = match self.theme.mode {
            ThemeMode::Dark => Theme::light(),
            ThemeMode::Light => Theme::dark(),
        };
        eprintln!(
            "theme → {} (full Ui + Scene rebuild)",
            mode_label(next.mode)
        );
        self.apply_theme(next);
    }

    fn apply_button(&mut self, id: WidgetId) -> bool {
        if id == self.btn_toggle {
            self.toggle_theme();
            true
        } else {
            false
        }
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        eprintln!(
            "next-gui theme tokens ready ({}x{}, mode={}). Space toggles; Esc quits.",
            w,
            h,
            mode_label(self.theme.mode)
        );
        ctx.set_title(&format!(
            "next-gui Theme tokens — {} — {w}x{h}",
            mode_label(self.theme.mode)
        ));
        self.needs_paint = true;
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.win_w = width;
                self.win_h = height;
                self.soft.resize(width, height);
                // Rebuild so column/status geometry matches the new size.
                self.apply_theme(self.theme);
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
                    } else {
                        self.ui.sync(&mut self.scene);
                        self.needs_paint = true;
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
                        } else {
                            self.ui.sync(&mut self.scene);
                            self.needs_paint = true;
                        }
                        ctx.request_redraw();
                        return;
                    }
                    if matches!(ui_key, UiKey::Space) {
                        self.toggle_theme();
                        ctx.request_redraw();
                        return;
                    }
                    if matches!(ui_key, UiKey::Escape) {
                        ctx.request_exit();
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
        self.soft.render(&cmds, &dirty);
        self.frame += 1;
        eprintln!(
            "frame {}: mode={} dirty_rects={} cmds={}",
            self.frame,
            mode_label(self.theme.mode),
            dirty.rects.len(),
            cmds.len()
        );
        if let Some((pixels, w, h)) = self.soft.frame_pixels() {
            if let Err(e) = ctx.present(w, h, pixels) {
                eprintln!("present error: {e}");
            }
            ctx.set_title(&format!(
                "next-gui Theme tokens — {} — frame {}",
                mode_label(self.theme.mode),
                self.frame
            ));

            if self.qa_exit && !self.qa_done {
                self.qa_done = true;
                let path = "/tmp/next-gui-theme.ppm";
                match write_ppm(path, pixels, w, h) {
                    Ok(()) => eprintln!("NEXT_GUI_THEME_QA: wrote {path}"),
                    Err(e) => eprintln!("NEXT_GUI_THEME_QA: write {path} failed: {e}"),
                }
                ctx.request_exit();
            }
        }
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui Theme tokens", 720, 520);
    let demo = Demo::new(attrs.width, attrs.height);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed to run theme tokens window: {e}");
        eprintln!("hint: need an active graphical desktop session");
        std::process::exit(1);
    }
}
