//! P2 demo: Latin + CJK text via `next-gui-text` on soft render.
//!
//! Shows mixed strings and a simple counter label with Chinese UI chrome.
//! Escape / close to quit.

use next_gui_platform::{AppCallbacks, Event, KeyCode, PointerButton, WindowAttrs, WindowCtx};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::load_default_cjk_font;
use next_gui_ui::{ColumnStyle, Ui, WidgetId};

const CLEAR: Color = Color {
    r: 0x1c,
    g: 0x1c,
    b: 0x1e,
    a: 255,
};

struct Demo {
    scene: Scene,
    soft: SoftRenderer,
    ui: Ui,
    title: WidgetId,
    mixed: WidgetId,
    status: WidgetId,
    btn_inc: WidgetId,
    btn_reset: WidgetId,
    count: i32,
    needs_paint: bool,
    frame: u64,
}

impl Demo {
    fn new(width: u32, height: u32) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });
        let mut scene = Scene::new();
        let mut ui = Ui::with_font(font);
        let mut title = WidgetId(0);
        let mut mixed = WidgetId(0);
        let mut status = WidgetId(0);
        let mut btn_inc = WidgetId(0);
        let mut btn_reset = WidgetId(0);

        ui.column(
            ColumnStyle {
                x: 32.0,
                y: 28.0,
                width: 520.0,
                height: 440.0,
                padding: 20.0,
                gap: 12.0,
                background: Some(Color::rgb(0x2c, 0x2c, 0x2e)),
            },
            |col| {
                title = col.text_styled(
                    "next-gui P2 — 文本 / CJK",
                    Color::rgb(0xff, 0xff, 0xff),
                    26.0,
                );
                mixed = col.text_styled(
                    "Hello 工业界面 · 你好世界 · Latin+CJK",
                    Color::rgb(0xc7, 0xcd, 0xd8),
                    22.0,
                );
                status = col.text_styled("计数 Count: 0", Color::rgb(0x8a, 0xe6, 0x9a), 22.0);
                btn_inc = col.button("增加 Increment");
                btn_reset = col.button_styled(
                    "重置 Reset",
                    Color::rgb(0x6b, 0x72, 0x80),
                    Color::rgb(0xff, 0xff, 0xff),
                );
            },
        );

        ui.sync(&mut scene);
        scene.mark_full_dirty(width as f32, height as f32);

        Self {
            scene,
            soft: SoftRenderer::new(width, height, CLEAR),
            ui,
            title,
            mixed,
            status,
            btn_inc,
            btn_reset,
            count: 0,
            needs_paint: true,
            frame: 0,
        }
    }

    fn refresh_status(&mut self) {
        self.ui.set_text(
            self.status,
            format!("计数 Count: {}", self.count),
        );
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        eprintln!(
            "next-gui P2 CJK text ready ({}x{}). Click 增加/重置; Esc quits.",
            w, h
        );
        eprintln!(
            "labels: title={:?} mixed={:?} (font ok)",
            self.ui.text_content(self.title),
            self.ui.text_content(self.mixed)
        );
        ctx.set_title(&format!("next-gui P2 CJK — {w}x{h}"));
        self.needs_paint = true;
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.soft.resize(width, height);
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
                        self.refresh_status();
                        ctx.request_redraw();
                    } else if id == self.btn_reset {
                        self.count = 0;
                        self.refresh_status();
                        ctx.request_redraw();
                    }
                }
            }
            Event::Key {
                key: KeyCode::Escape,
                pressed: true,
                ..
            } => {}
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
            "frame {}: dirty_rects={} approx_area={:.0} cmds={} count={}",
            self.frame,
            dirty.rects.len(),
            dirty.approx_area(),
            cmds.len(),
            self.count
        );
        if let Some((pixels, w, h)) = self.soft.frame_pixels() {
            if let Err(e) = ctx.present(w, h, pixels) {
                eprintln!("present error: {e}");
            }
            ctx.set_title(&format!(
                "next-gui P2 CJK — count {} — frame {}",
                self.count, self.frame
            ));
        }
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui P2 CJK text", 600, 480);
    let demo = Demo::new(attrs.width, attrs.height);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed to run P2 window: {e}");
        eprintln!("hint: need graphical session + CJK font (e.g. fonts-noto-cjk)");
        std::process::exit(1);
    }
}
