//! P1 demo: UI described in a `.ngui` file, loaded into retained `Ui`.
//!
//! Controls:
//! - Click **Increment** / **Reset** (wired by `@inc` / `@reset` names).
//! - Escape / close: quit.

use next_gui_platform::{
    AppCallbacks, Event, KeyCode, PointerButton, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_ui::{load_ngui, Ui, WidgetId};

const CLEAR: Color = Color {
    r: 0x20,
    g: 0x20,
    b: 0x20,
    a: 255,
};

const NGUI: &str = include_str!("../ui/counter.ngui");

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
        let loaded = load_ngui(NGUI).unwrap_or_else(|e| {
            eprintln!("failed to parse counter.ngui: {e}");
            std::process::exit(1);
        });
        let label = loaded.require("label").unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        });
        let btn_inc = loaded.require("inc").unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        });
        let btn_reset = loaded.require("reset").unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        });

        let mut ui = loaded.ui;
        let mut scene = Scene::new();
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
            "next-gui P1 .ngui counter ready ({}x{}). UI from counter.ngui; Esc quits.",
            w, h
        );
        ctx.set_title(&format!("next-gui P1 .ngui — {w}x{h}"));
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
                "next-gui P1 .ngui — count {} — frame {} — dirty {:.0}px²",
                self.count,
                self.frame,
                dirty.approx_area()
            ));
        }
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui P1 .ngui counter", 480, 420);
    let demo = Demo::new(attrs.width, attrs.height);
    if let Err(e) = next_gui_platform::run(attrs, demo) {
        eprintln!("failed to run P1 .ngui window: {e}");
        eprintln!("hint: need a graphical DISPLAY (X11/Wayland) on Linux");
        std::process::exit(1);
    }
}
