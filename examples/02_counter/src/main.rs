//! P1 demo: compact declarative Column + Text + Button on Linux and macOS.
//!
//! Controls:
//! - Click **Increment** / **Reset** to update count (state → Text).
//! - Escape / close: quit.
//!
//! Stderr logs dirty area when a frame paints.

use next_gui_platform::{AppCallbacks, Event, KeyCode, PointerButton, WindowAttrs, WindowCtx};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_ui::{ColumnStyle, Ui, WidgetId};

const CLEAR: Color = Color {
    r: 0x20,
    g: 0x20,
    b: 0x20,
    a: 255,
};

struct Demo {
    scene: Scene,
    soft: SoftRenderer,
    ui: Ui,
    label: WidgetId,
    btn_inc: WidgetId,
    btn_reset: WidgetId,
    count: i32,
    needs_paint: bool,
    frame: u64,
}

impl Demo {
    fn new(width: u32, height: u32) -> Self {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut label = WidgetId(0);
        let mut btn_inc = WidgetId(0);
        let mut btn_reset = WidgetId(0);

        ui.column(
            ColumnStyle {
                x: 40.0,
                y: 40.0,
                width: 360.0,
                height: 520.0,
                padding: 20.0,
                gap: 14.0,
                background: Some(Color::rgb(0x2a, 0x2a, 0x2e)),
                            ..ColumnStyle::default()
            },
            |col| {
                label = col.text("Count: 0");
                btn_inc = col.button("Increment");
                btn_reset = col.button_styled(
                    "Reset",
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
            label,
            btn_inc,
            btn_reset,
            count: 0,
            needs_paint: true,
            frame: 0,
        }
    }

    fn refresh_label(&mut self) {
        self.ui
            .set_text(self.label, format!("Count: {}", self.count));
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        eprintln!(
            "next-gui P1 counter ready ({}x{}). Click Increment/Reset; Esc quits.",
            w, h
        );
        ctx.set_title(&format!("next-gui P1 counter — {w}x{h}"));
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
                        self.refresh_label();
                        ctx.request_redraw();
                    } else if id == self.btn_reset {
                        self.count = 0;
                        self.refresh_label();
                        ctx.request_redraw();
                    }
                }
            }
            Event::Key {
                key: KeyCode::Escape,
                pressed: true,
                ..
            } => {
                // platform handles CloseRequested via window; Escape mapped in linux backend may quit
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
                "next-gui P1 — count {} — frame {} — dirty {:.0}px²",
                self.count,
                self.frame,
                dirty.approx_area()
            ));
        }
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui P1 counter", 480, 420);
    let demo = Demo::new(attrs.width, attrs.height);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed to run P1 window: {e}");
        eprintln!("hint: need an active graphical desktop session");
        std::process::exit(1);
    }
}
