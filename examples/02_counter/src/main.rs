//! P1 demo: compact declarative Column + Text + Button on Linux and macOS.
//!
//! Controls:
//! - Click **Increment** / **Reset** to update count (state → Text).
//! - Tab / Shift+Tab: focus order (buttons included); Space / Enter: activate focused button.
//! - Escape / close: quit.
//!
//! Stderr logs dirty area when a frame paints.
//!
//! # Soak (P6e)
//!
//! ```bash
//! NEXT_GUI_SOAK=1 cargo run -p e02_counter
//! # optional interval (ms, default 1000):
//! NEXT_GUI_SOAK=1 NEXT_GUI_SOAK_MS=2000 cargo run -p e02_counter
//! ```
//!
//! When set, the platform wakes on a timer and this example auto-increments
//! the counter each soak tick (exercises dirty + Soft present). Linux logs
//! `VmRSS` from `/proc/self/status`. See `docs/soak.md`. Env unset → identical
//! to the interactive demo.

use next_gui_platform::{
    soak, AppCallbacks, Event, KeyCode, Modifiers, PointerButton, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_ui::{ColumnStyle, Ui, UiKey, WidgetId};
use std::time::{Duration, Instant};

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
    /// P6e: when `NEXT_GUI_SOAK` is set at start.
    soak: bool,
    soak_interval: Duration,
    last_soak_tick: Instant,
    last_rss_log: Instant,
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

impl Demo {
    fn new(width: u32, height: u32) -> Self {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let theme = *ui.theme();
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
                ..ColumnStyle::from_theme(&theme)
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

        let soak = soak::enabled();
        let soak_interval = soak::interval();
        let now = Instant::now();

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
            soak,
            soak_interval,
            last_soak_tick: now,
            last_rss_log: now,
        }
    }

    fn refresh_label(&mut self) {
        self.ui
            .set_text(self.label, format!("Count: {}", self.count));
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
    }

    /// Shared by pointer `hit_widget` and keyboard `take_button_activate`.
    fn apply_button(&mut self, id: WidgetId) -> bool {
        if id == self.btn_inc {
            self.count = self.count.saturating_add(1);
            self.refresh_label();
            true
        } else if id == self.btn_reset {
            self.count = 0;
            self.refresh_label();
            true
        } else {
            false
        }
    }

    fn soak_tick(&mut self) {
        if !self.soak {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_soak_tick) < self.soak_interval {
            return;
        }
        self.last_soak_tick = now;
        // Cheap retained mutation so dirty + Soft present stay warm for hours.
        self.count = self.count.wrapping_add(1);
        self.refresh_label();

        // Heartbeat RSS log (~every 5s or each tick if interval is slower).
        let log_every = self.soak_interval.max(Duration::from_secs(5));
        if now.duration_since(self.last_rss_log) >= log_every {
            self.last_rss_log = now;
            match soak::linux_vm_rss_kb() {
                Some(kb) => eprintln!(
                    "soak: count={} frame={} VmRSS={} kB (~{:.1} MiB)",
                    self.count,
                    self.frame,
                    kb,
                    kb as f64 / 1024.0
                ),
                None => eprintln!(
                    "soak: count={} frame={} (VmRSS n/a on this OS)",
                    self.count, self.frame
                ),
            }
        }
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        if self.soak {
            eprintln!(
                "next-gui P1 counter SOAK mode ({}x{}, interval {} ms). Esc quits.",
                w,
                h,
                self.soak_interval.as_millis()
            );
            ctx.set_title(&format!(
                "next-gui soak — {w}x{h} — {}ms",
                self.soak_interval.as_millis()
            ));
        } else {
            eprintln!(
                "next-gui P1 counter ready ({}x{}). Click or Tab+Space/Enter; Esc quits.",
                w, h
            );
            ctx.set_title(&format!("next-gui P1 counter — {w}x{h}"));
        }
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
                let xf = x as f32;
                let yf = y as f32;
                // Focus button on press (returns false); click semantics stay on hit_widget.
                let _ = self.ui.pointer_pressed(&mut self.scene, xf, yf);
                if let Some(id) = self.ui.hit_widget(&self.scene, xf, yf) {
                    if self.apply_button(id) {
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
                        }
                        self.ui.sync(&mut self.scene);
                        self.needs_paint = true;
                        ctx.request_redraw();
                        return;
                    }
                    if matches!(ui_key, UiKey::Escape) {
                        ctx.request_exit();
                    }
                }
            }
            Event::RedrawRequested { .. } => {
                self.soak_tick();
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
        if !self.soak {
            eprintln!(
                "frame {}: dirty_rects={} approx_area={:.0} cmds={} count={}",
                self.frame,
                dirty.rects.len(),
                dirty.approx_area(),
                cmds.len(),
                self.count
            );
        }
        if let Some((pixels, w, h)) = self.soft.frame_pixels() {
            if let Err(e) = ctx.present(w, h, pixels) {
                eprintln!("present error: {e}");
            }
            if self.soak {
                ctx.set_title(&format!(
                    "next-gui soak — count {} — frame {}",
                    self.count, self.frame
                ));
            } else {
                ctx.set_title(&format!(
                    "next-gui P1 — count {} — frame {} — dirty {:.0}px²",
                    self.count,
                    self.frame,
                    dirty.approx_area()
                ));
            }
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
