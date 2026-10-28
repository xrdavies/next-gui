//! Built-in icon glyphs for toolbars / status (simple alpha masks).

use next_gui_scene::AlphaMask;

/// Named built-in icon for [`crate::Ui`] toolbars and panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconName {
    /// Empty placeholder.
    Blank,
    /// Folder / project.
    Folder,
    /// Generic file.
    File,
    /// Save disk.
    Save,
    /// Open / folder-out.
    Open,
    /// Gear / settings.
    Settings,
    /// Play triangle.
    Play,
    /// Stop square.
    Stop,
    /// Plus.
    Plus,
    /// Minus.
    Minus,
    /// Close / X.
    Close,
    /// Check mark.
    Check,
    /// Warning triangle.
    Warning,
    /// Info circle (i).
    Info,
    /// Magnifier.
    Search,
    /// Circular arrows (refresh).
    Refresh,
    /// Hamburger menu.
    Menu,
    /// Home house.
    Home,
    /// Database cylinder.
    Database,
    /// Return arrow (undo/revert).
    Undo,
    /// Execute one step / line.
    Step,
    /// Code / comment delimiters.
    Code,
}

impl IconName {
    /// Parse a stable id string (`"save"`, `"folder"`, …). Unknown → [`IconName::Blank`].
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "blank" | "" => Self::Blank,
            "folder" => Self::Folder,
            "file" => Self::File,
            "save" => Self::Save,
            "open" => Self::Open,
            "settings" | "gear" => Self::Settings,
            "play" => Self::Play,
            "stop" => Self::Stop,
            "plus" | "add" => Self::Plus,
            "minus" | "remove" => Self::Minus,
            "close" | "x" => Self::Close,
            "check" | "ok" => Self::Check,
            "warning" | "warn" => Self::Warning,
            "info" => Self::Info,
            "search" => Self::Search,
            "refresh" | "reload" => Self::Refresh,
            "menu" | "hamburger" => Self::Menu,
            "home" => Self::Home,
            "database" | "db" => Self::Database,
            "undo" | "revert" => Self::Undo,
            "step" => Self::Step,
            "code" | "comment" => Self::Code,
            _ => Self::Blank,
        }
    }

    /// Stable id used in `.ngui` / docs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blank => "blank",
            Self::Folder => "folder",
            Self::File => "file",
            Self::Save => "save",
            Self::Open => "open",
            Self::Settings => "settings",
            Self::Play => "play",
            Self::Stop => "stop",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Close => "close",
            Self::Check => "check",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Search => "search",
            Self::Refresh => "refresh",
            Self::Menu => "menu",
            Self::Home => "home",
            Self::Database => "database",
            Self::Undo => "undo",
            Self::Step => "step",
            Self::Code => "code",
        }
    }
}

struct Pix<'a> {
    s: u32,
    px: &'a mut [u8],
}

impl Pix<'_> {
    fn set(&mut self, x: i32, y: i32, a: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.s && (y as u32) < self.s {
            let i = (y as u32 * self.s + x as u32) as usize;
            self.px[i] = self.px[i].max(a);
        }
    }

    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, a: u8) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        let mut x = x0;
        let mut y = y0;
        loop {
            self.set(x, y, a);
            self.set(x + 1, y, a / 2);
            self.set(x, y + 1, a / 2);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, a: u8) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, a);
            }
        }
    }

    fn stroke_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, a: u8) {
        self.line(x0, y0, x1, y0, a);
        self.line(x1, y0, x1, y1, a);
        self.line(x1, y1, x0, y1, a);
        self.line(x0, y1, x0, y0, a);
    }

    fn stroke_ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, a: u8) {
        let mut last = (cx + rx, cy);
        for step in 1..=64 {
            let angle = step as f32 * std::f32::consts::TAU / 64.0;
            let next = (cx + (rx as f32 * angle.cos()).round() as i32, cy + (ry as f32 * angle.sin()).round() as i32);
            self.line(last.0, last.1, next.0, next.1, a);
            last = next;
        }
    }

    fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, a: u8) {
        for y in (cy - r)..=(cy + r) {
            for x in (cx - r)..=(cx + r) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r * r {
                    self.set(x, y, a);
                }
            }
        }
    }
}

/// Rasterize a built-in icon into an alpha mask of roughly `size`×`size` pixels.
pub fn rasterize_icon(name: IconName, size: f32) -> AlphaMask {
    let s = size.round().clamp(8.0, 64.0) as u32;
    let mut px = vec![0u8; (s * s) as usize];
    let mut p = Pix {
        s,
        px: &mut px,
    };

    let m = (s as i32) - 1;
    let inset = (s as i32 / 6).max(1);
    let a = 240u8;

    match name {
        IconName::Blank => {}
        IconName::Database => {
            let (cx, rx, ry) = (m / 2, ((m - inset * 2) / 2).max(1), (s as i32 / 8).max(1));
            let top = inset + ry; let bottom = m - inset - ry;
            for cy in [top, (top + bottom) / 2, bottom] { p.stroke_ellipse(cx, cy, rx, ry, a); }
            p.line(cx - rx, top, cx - rx, bottom, a);
            p.line(cx + rx, top, cx + rx, bottom, a);
        }
        IconName::Undo => {
            let cy = s as i32 / 2; let arm = (s as i32 / 4).max(1);
            p.line(inset, cy, inset + arm, cy - arm, a);
            p.line(inset, cy, inset + arm, cy + arm, a);
            p.line(inset, cy, m - inset, cy, a);
            p.line(m - inset, cy, m - inset, m - inset, a);
        }
        IconName::Step => {
            p.line(inset, inset, inset, m - inset, a);
            p.line(inset, inset, m - inset - 2, s as i32 / 2, a);
            p.line(inset, m - inset, m - inset - 2, s as i32 / 2, a);
            p.line(m - inset, inset, m - inset, m - inset, a);
        }
        IconName::Code => {
            let c = s as i32 / 2; let arm = (s as i32 / 6).max(1);
            p.line(inset + arm, inset + 1, inset, c, a);
            p.line(inset, c, inset + arm, m - inset - 1, a);
            p.line(m - inset - arm, inset + 1, m - inset, c, a);
            p.line(m - inset, c, m - inset - arm, m - inset - 1, a);
            p.line(c + 1, inset, c - 1, m - inset, a);
        }
        IconName::Folder => {
            let top = inset;
            let mid = s as i32 / 3;
            p.fill_rect(inset, top, inset + s as i32 / 3, mid, a);
            p.stroke_rect(inset, mid, m - inset, m - inset, a);
            p.fill_rect(inset + 1, mid + 1, m - inset - 1, m - inset - 1, a / 3);
        }
        IconName::File => {
            p.stroke_rect(inset + 1, inset, m - inset, m - inset, a);
            p.line(
                m - inset - s as i32 / 4,
                inset,
                m - inset,
                inset + s as i32 / 4,
                a,
            );
            p.line(
                m - inset - s as i32 / 4,
                inset,
                m - inset - s as i32 / 4,
                inset + s as i32 / 4,
                a,
            );
            p.line(
                m - inset - s as i32 / 4,
                inset + s as i32 / 4,
                m - inset,
                inset + s as i32 / 4,
                a,
            );
        }
        IconName::Save => {
            p.stroke_rect(inset, inset, m - inset, m - inset, a);
            p.fill_rect(inset + 2, inset + 2, m - inset - 2, inset + s as i32 / 3, a);
            p.fill_rect(
                inset + s as i32 / 4,
                m - inset - s as i32 / 3,
                m - inset - s as i32 / 4,
                m - inset - 1,
                a / 2,
            );
        }
        IconName::Open => {
            p.stroke_rect(inset, inset + 2, m - inset - 2, m - inset, a);
            p.line(inset + 2, inset + s as i32 / 3, m - inset, inset, a);
            p.line(m - inset, inset, m - inset, inset + s as i32 / 3, a);
        }
        IconName::Settings => {
            let cx = s as i32 / 2;
            let cy = s as i32 / 2;
            p.fill_circle(cx, cy, s as i32 / 5, a);
            for i in 0..8 {
                let ang = (i as f32) * std::f32::consts::PI / 4.0;
                let x0 = cx + ((s as f32 / 4.0) * ang.cos()) as i32;
                let y0 = cy + ((s as f32 / 4.0) * ang.sin()) as i32;
                let x1 = cx + ((s as f32 / 2.2) * ang.cos()) as i32;
                let y1 = cy + ((s as f32 / 2.2) * ang.sin()) as i32;
                p.line(x0, y0, x1, y1, a);
            }
        }
        IconName::Play => {
            let x0 = inset + 1;
            let x1 = m - inset;
            let y0 = inset;
            let y1 = m - inset;
            let ym = (y0 + y1) / 2;
            p.line(x0, y0, x0, y1, a);
            p.line(x0, y0, x1, ym, a);
            p.line(x0, y1, x1, ym, a);
            for y in y0..=y1 {
                let t = (y - y0) as f32 / (y1 - y0).max(1) as f32;
                let edge = if t <= 0.5 {
                    x0 + ((x1 - x0) as f32 * (t * 2.0)) as i32
                } else {
                    x0 + ((x1 - x0) as f32 * ((1.0 - t) * 2.0)) as i32
                };
                for x in x0..=edge {
                    p.set(x, y, a / 3);
                }
            }
        }
        IconName::Stop => {
            p.fill_rect(inset + 1, inset + 1, m - inset - 1, m - inset - 1, a);
        }
        IconName::Plus => {
            let c = s as i32 / 2;
            let arm = s as i32 / 3;
            p.fill_rect(c - 1, c - arm, c + 1, c + arm, a);
            p.fill_rect(c - arm, c - 1, c + arm, c + 1, a);
        }
        IconName::Minus => {
            let c = s as i32 / 2;
            let arm = s as i32 / 3;
            p.fill_rect(c - arm, c - 1, c + arm, c + 1, a);
        }
        IconName::Close => {
            p.line(inset, inset, m - inset, m - inset, a);
            p.line(m - inset, inset, inset, m - inset, a);
        }
        IconName::Check => {
            let y = s as i32 / 2;
            p.line(inset, y, inset + s as i32 / 4, m - inset - 1, a);
            p.line(inset + s as i32 / 4, m - inset - 1, m - inset, inset + 1, a);
        }
        IconName::Warning => {
            let top = inset;
            let bot = m - inset;
            let mid = s as i32 / 2;
            p.line(mid, top, inset, bot, a);
            p.line(mid, top, m - inset, bot, a);
            p.line(inset, bot, m - inset, bot, a);
            p.fill_rect(mid - 1, top + s as i32 / 3, mid + 1, bot - s as i32 / 4, a);
            p.set(mid, bot - 2, a);
        }
        IconName::Info => {
            let cx = s as i32 / 2;
            let cy = s as i32 / 2;
            for y in 0..s as i32 {
                for x in 0..s as i32 {
                    let dx = x - cx;
                    let dy = y - cy;
                    let d2 = dx * dx + dy * dy;
                    let r = s as i32 / 2 - inset;
                    let r2 = r * r;
                    let ri = (r - 2).max(1);
                    if d2 <= r2 && d2 >= ri * ri {
                        p.set(x, y, a);
                    }
                }
            }
            p.fill_rect(cx - 1, cy - 1, cx + 1, cy + s as i32 / 4, a);
            p.set(cx, cy - s as i32 / 4, a);
        }
        IconName::Search => {
            let cx = s as i32 / 2 - 1;
            let cy = s as i32 / 2 - 1;
            let r = s as i32 / 4;
            for y in (cy - r)..=(cy + r) {
                for x in (cx - r)..=(cx + r) {
                    let dx = x - cx;
                    let dy = y - cy;
                    let d2 = dx * dx + dy * dy;
                    let inner = (r - 2).max(1);
                    if d2 <= r * r && d2 >= inner * inner {
                        p.set(x, y, a);
                    }
                }
            }
            p.line(cx + r - 1, cy + r - 1, m - inset, m - inset, a);
        }
        IconName::Refresh => {
            let cx = s as i32 / 2;
            let cy = s as i32 / 2;
            let r = s as i32 / 3;
            for i in 0..20 {
                let t0 = (i as f32) / 20.0 * 1.6 * std::f32::consts::PI;
                let t1 = ((i + 1) as f32) / 20.0 * 1.6 * std::f32::consts::PI;
                let x0 = cx + (r as f32 * t0.cos()) as i32;
                let y0 = cy + (r as f32 * t0.sin()) as i32;
                let x1 = cx + (r as f32 * t1.cos()) as i32;
                let y1 = cy + (r as f32 * t1.sin()) as i32;
                p.line(x0, y0, x1, y1, a);
            }
            let tip_x = cx + r;
            let tip_y = cy;
            p.line(tip_x, tip_y, tip_x - 3, tip_y - 4, a);
            p.line(tip_x, tip_y, tip_x - 4, tip_y + 2, a);
        }
        IconName::Menu => {
            let y1 = inset + 1;
            let y2 = s as i32 / 2;
            let y3 = m - inset - 1;
            p.fill_rect(inset, y1 - 1, m - inset, y1 + 1, a);
            p.fill_rect(inset, y2 - 1, m - inset, y2 + 1, a);
            p.fill_rect(inset, y3 - 1, m - inset, y3 + 1, a);
        }
        IconName::Home => {
            let mid = s as i32 / 2;
            let roof = inset;
            let eaves = m - inset;
            let wall_top = s as i32 / 2;
            p.line(mid, roof, inset, wall_top, a);
            p.line(mid, roof, eaves, wall_top, a);
            p.stroke_rect(inset + 2, wall_top, eaves - 2, m - inset, a);
            p.fill_rect(
                mid - s as i32 / 8,
                m - inset - s as i32 / 3,
                mid + s as i32 / 8,
                m - inset,
                a / 2,
            );
        }
    }

    AlphaMask::new(s, s, px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_icon_not_empty() {
        let m = rasterize_icon(IconName::Save, 16.0);
        assert_eq!(m.width, 16);
        assert!(m.pixels.iter().any(|&p| p > 0));
    }

    #[test]
    fn database_action_icons_roundtrip_and_remain_distinct_at_supported_sizes() {
        let icons = [IconName::Database, IconName::Undo, IconName::Step, IconName::Code];
        for size in [8.0, 18.0, 32.0, 64.0] {
            let mut masks = Vec::new();
            for icon in icons {
                assert_eq!(IconName::from_name(icon.as_str()), icon);
                let mask = rasterize_icon(icon, size);
                assert_eq!((mask.width, mask.height), (size as u32, size as u32));
                assert!(mask.pixels.iter().any(|&p| p > 0));
                assert!(!masks.contains(&mask.pixels)); masks.push(mask.pixels);
            }
        }
    }

    #[test]
    fn from_name_roundtrip() {
        assert_eq!(IconName::from_name("save"), IconName::Save);
        assert_eq!(IconName::Save.as_str(), "save");
    }
}
