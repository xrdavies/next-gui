//! P0 demo: retained rect scene + dirty-region soft render on Linux and macOS.
//!
//! Controls:
//! - Left click a colored block to cycle its color (dirty region updates).
//! - Touch (primary finger) synthesizes the same left-click path (P6a).
//! - Space: cycle all blocks' colors.
//! - Escape / close: quit.
//!
//! Stderr logs dirty rect area each redraw that actually paints.

use next_gui_platform::{AppCallbacks, Event, KeyCode, PointerButton, WindowAttrs, WindowCtx};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, NodeId, Rect, Scene};

const CLEAR: Color = Color {
    r: 0x20,
    g: 0x20,
    b: 0x20,
    a: 255,
};

const PALETTE: [Color; 4] = [
    Color {
        r: 0x3b,
        g: 0x82,
        b: 0xf6,
        a: 255,
    },
    Color {
        r: 0x22,
        g: 0xc5,
        b: 0x5e,
        a: 255,
    },
    Color {
        r: 0xf5,
        g: 0x9e,
        b: 0x0b,
        a: 255,
    },
    Color {
        r: 0xef,
        g: 0x44,
        b: 0x44,
        a: 255,
    },
];

struct Demo {
    scene: Scene,
    soft: SoftRenderer,
    blocks: Vec<NodeId>,
    palette_idx: usize,
    needs_paint: bool,
    frame: u64,
}

impl Demo {
    fn new(width: u32, height: u32) -> Self {
        let mut scene = Scene::new();
        let root = scene.root();
        // Background panel
        let _bg = scene
            .insert(
                root,
                Rect::new(0.0, 0.0, width as f32, height as f32),
                Color::rgb(0x2a, 0x2a, 0x2e),
            )
            .expect("bg");
        let mut blocks = Vec::new();
        let specs = [
            (40.0, 80.0, 160.0, 64.0, 0usize),
            (240.0, 80.0, 160.0, 64.0, 1),
            (440.0, 80.0, 160.0, 64.0, 2),
            (40.0, 200.0, 360.0, 120.0, 3),
        ];
        for (x, y, w, h, pi) in specs {
            let id = scene
                .insert(root, Rect::new(x, y, w, h), PALETTE[pi])
                .expect("block");
            blocks.push(id);
        }
        scene.mark_full_dirty(width as f32, height as f32);
        Self {
            scene,
            soft: SoftRenderer::new(width, height, CLEAR),
            blocks,
            palette_idx: 0,
            needs_paint: true,
            frame: 0,
        }
    }

    fn cycle_node(&mut self, id: NodeId) {
        let Some(node) = self.scene.get(id) else {
            return;
        };
        // Skip full-window background (first child): only cycle known blocks
        if !self.blocks.contains(&id) {
            return;
        }
        let cur = node.fill;
        let next = PALETTE
            .iter()
            .position(|c| *c == cur)
            .map(|i| PALETTE[(i + 1) % PALETTE.len()])
            .unwrap_or(PALETTE[0]);
        self.scene.set_fill(id, next);
        self.needs_paint = true;
    }

    fn cycle_all(&mut self) {
        self.palette_idx = (self.palette_idx + 1) % PALETTE.len();
        for (i, id) in self.blocks.iter().copied().enumerate() {
            let color = PALETTE[(self.palette_idx + i) % PALETTE.len()];
            self.scene.set_fill(id, color);
        }
        self.needs_paint = true;
    }
}

impl AppCallbacks for Demo {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        eprintln!(
            "next-gui P0 soft window ready ({}x{}). Click blocks, Space cycles, Esc quits.",
            w, h
        );
        ctx.set_title(&format!("next-gui P0 soft — {w}x{h}"));
        self.needs_paint = true;
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::Resized { width, height, .. } => {
                self.soft.resize(width, height);
                // Resize background to cover window
                if let Some(bg) = self.scene.iter_paint_order().first().map(|n| n.id) {
                    self.scene
                        .set_bounds(bg, Rect::new(0.0, 0.0, width as f32, height as f32));
                }
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
                if let Some(id) = self.scene.hit_test(x as f32, y as f32) {
                    self.cycle_node(id);
                    ctx.request_redraw();
                }
            }
            Event::Key {
                key: KeyCode::Space,
                pressed: true,
                ..
            } => {
                self.cycle_all();
                ctx.request_redraw();
            }
            Event::Key {
                key: KeyCode::Escape,
                pressed: true,
                ..
            } => {
                ctx.request_exit();
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
            "frame {}: dirty_rects={} approx_area={:.0} cmds={}",
            self.frame,
            dirty.rects.len(),
            dirty.approx_area(),
            cmds.len()
        );
        if let Some((pixels, w, h)) = self.soft.frame_pixels() {
            if let Err(e) = ctx.present(w, h, pixels) {
                eprintln!("present error: {e}");
            }
            ctx.set_title(&format!(
                "next-gui P0 soft — {w}x{h} — frame {} — dirty area {:.0}px²",
                self.frame,
                dirty.approx_area()
            ));
        }
        self.needs_paint = false;
    }
}

fn main() {
    let attrs = WindowAttrs::new("next-gui P0 soft", 800, 600);
    let demo = Demo::new(attrs.width, attrs.height);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed to run P0 window: {e}");
        eprintln!("hint: need an active graphical desktop session");
        std::process::exit(1);
    }
}
