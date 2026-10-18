//! Field / Office **profile** — capability flags and performance budgets.
//!
//! This crate is a **config layer**, not a second UI stack. The same retained
//! `ui` / `scene` / `DrawCommand` path runs under either profile; only whether
//! GPU is allowed, present preference, fps / effects budgets, and soft-force
//! differ.
//!
//! See `docs/p4-design.md` and architecture.md §6.

use std::time::Duration;

/// Which product tier the process is running as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuiProfile {
    /// Field / HMI: Soft-stable, GPU optional-off, lower budgets.
    Field,
    /// Office desktop: GPU preferred, higher frame / effects budget.
    Office,
}

impl GuiProfile {
    /// Short English name for status bars / logs.
    pub fn name(self) -> &'static str {
        match self {
            GuiProfile::Field => "Field",
            GuiProfile::Office => "Office",
        }
    }

    /// Parse `field` / `office` (case-insensitive). Unknown → `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "field" | "f" | "hmi" => Some(GuiProfile::Field),
            "office" | "o" | "desktop" => Some(GuiProfile::Office),
            _ => None,
        }
    }
}

/// How the app should open the OS window for present.
///
/// Mapped by the app onto `next_gui_platform::PresentBackend` — this crate does
/// **not** depend on `platform` so Field packaging can omit GPU crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestedPresent {
    /// Softbuffer (or equivalent CPU present). Always valid; Field default.
    Softbuffer,
    /// Reserve the window for a native GPU surface (Office when GPU allowed).
    GpuSurface,
}

/// Coarse animation / motion budget (apps interpret; profile only flags it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationBudget {
    /// Field: no decorative motion; charts may tick slowly.
    Minimal,
    /// Office: richer motion allowed within `max_fps`.
    Full,
}

/// Why a runtime degradation was applied (logging / UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DegradationReason {
    /// No GPU adapter or `GpuRenderer` init failed.
    NoGpu,
    /// Explicit operator / env force Soft.
    ForceSoft,
    /// Thermal / load policy (reserved for apps).
    Thermal,
    /// Other / unspecified.
    Other,
}

/// Feature switches + performance budgets for one process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileConfig {
    pub profile: GuiProfile,
    /// May attempt a GPU renderer at all.
    pub allow_gpu: bool,
    /// Prefer opening a GPU-surface window (Office). Ignored when GPU disallowed.
    pub prefer_native_surface: bool,
    /// Soft cap on continuous / animated redraws (interaction may still dirty-paint).
    pub max_fps: u32,
    /// Decorative effects / heavy motion (blur, fancy shadows, etc.).
    pub effects_enabled: bool,
    /// Never use GPU; Soft only (Field default).
    pub force_soft: bool,
    /// Suggested chart / trend refresh rate (Hz). `0` = follow `max_fps`.
    pub chart_refresh_hz: u32,
    pub animation_budget: AnimationBudget,
}

impl ProfileConfig {
    /// Field defaults: Soft-forced, no GPU, 24 fps, effects off.
    pub fn field() -> Self {
        Self {
            profile: GuiProfile::Field,
            allow_gpu: false,
            prefer_native_surface: false,
            max_fps: 24,
            effects_enabled: false,
            force_soft: true,
            chart_refresh_hz: 4,
            animation_budget: AnimationBudget::Minimal,
        }
    }

    /// Office defaults: GPU allowed, native surface preferred, 60 fps, effects on.
    pub fn office() -> Self {
        Self {
            profile: GuiProfile::Office,
            allow_gpu: true,
            prefer_native_surface: true,
            max_fps: 60,
            effects_enabled: true,
            force_soft: false,
            chart_refresh_hz: 15,
            animation_budget: AnimationBudget::Full,
        }
    }

    /// `NEXT_GUI_PROFILE=field|office` (default **Office** when unset / empty).
    pub fn from_env() -> Self {
        match std::env::var("NEXT_GUI_PROFILE") {
            Ok(v) if !v.trim().is_empty() => match GuiProfile::parse(&v) {
                Some(GuiProfile::Field) => Self::field(),
                Some(GuiProfile::Office) => Self::office(),
                None => {
                    eprintln!(
                        "next-gui-profile: unknown NEXT_GUI_PROFILE={v:?}; using Office"
                    );
                    Self::office()
                }
            },
            _ => Self::office(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.profile.name()
    }

    /// Flip between Field and Office presets (replaces the whole config).
    pub fn toggle(&mut self) {
        *self = match self.profile {
            GuiProfile::Field => Self::office(),
            GuiProfile::Office => Self::field(),
        };
    }

    /// True when the app may call `GpuRenderer::try_new` / `try_new_native`.
    pub fn should_try_gpu(&self) -> bool {
        self.allow_gpu && !self.force_soft
    }

    /// True when window creation should prefer a GPU surface (and GPU is allowed).
    pub fn prefer_gpu_surface_window(&self) -> bool {
        self.should_try_gpu() && self.prefer_native_surface
    }

    /// Suggested present path for `WindowAttrs` construction.
    pub fn suggested_present(&self) -> SuggestedPresent {
        if self.prefer_gpu_surface_window() {
            SuggestedPresent::GpuSurface
        } else {
            SuggestedPresent::Softbuffer
        }
    }

    /// Minimum interval between budgeted animation / chart ticks.
    pub fn frame_interval(&self) -> Duration {
        let fps = self.max_fps.max(1);
        Duration::from_nanos(1_000_000_000u64 / u64::from(fps))
    }

    /// Interval for chart / trend refresh (`chart_refresh_hz`, else `max_fps`).
    pub fn chart_interval(&self) -> Duration {
        let hz = if self.chart_refresh_hz == 0 {
            self.max_fps
        } else {
            self.chart_refresh_hz
        };
        let hz = hz.max(1);
        Duration::from_nanos(1_000_000_000u64 / u64::from(hz))
    }

    /// Clamp `max_fps` into `1..=240`.
    pub fn with_max_fps(mut self, fps: u32) -> Self {
        self.max_fps = fps.clamp(1, 240);
        self
    }

    /// Apply a degradation step (e.g. adapter missing): force Soft, disable effects.
    pub fn degrade(&mut self, reason: DegradationReason) {
        let _ = reason;
        self.force_soft = true;
        self.allow_gpu = false;
        self.prefer_native_surface = false;
        self.effects_enabled = false;
        if self.max_fps > 30 {
            self.max_fps = 30;
        }
        if self.chart_refresh_hz > 8 {
            self.chart_refresh_hz = 8;
        }
        self.animation_budget = AnimationBudget::Minimal;
    }

    /// One-line summary for StatusBar / stderr.
    pub fn summary(&self) -> String {
        format!(
            "{} | gpu={} force_soft={} surface={:?} max_fps={} effects={} charts={}Hz anim={:?}",
            self.name(),
            self.allow_gpu,
            self.force_soft,
            self.suggested_present(),
            self.max_fps,
            self.effects_enabled,
            if self.chart_refresh_hz == 0 {
                self.max_fps
            } else {
                self.chart_refresh_hz
            },
            self.animation_budget,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_forces_soft_and_disallows_gpu() {
        let p = ProfileConfig::field();
        assert_eq!(p.profile, GuiProfile::Field);
        assert!(p.force_soft);
        assert!(!p.allow_gpu);
        assert!(!p.should_try_gpu());
        assert!(!p.prefer_gpu_surface_window());
        assert_eq!(p.suggested_present(), SuggestedPresent::Softbuffer);
        assert!(!p.effects_enabled);
        assert_eq!(p.animation_budget, AnimationBudget::Minimal);
        assert!(p.max_fps <= 30);
    }

    #[test]
    fn office_allows_gpu_and_prefers_surface() {
        let p = ProfileConfig::office();
        assert_eq!(p.profile, GuiProfile::Office);
        assert!(p.allow_gpu);
        assert!(!p.force_soft);
        assert!(p.should_try_gpu());
        assert!(p.prefer_gpu_surface_window());
        assert_eq!(p.suggested_present(), SuggestedPresent::GpuSurface);
        assert!(p.effects_enabled);
        assert_eq!(p.animation_budget, AnimationBudget::Full);
        assert!(p.max_fps >= 60);
    }

    #[test]
    fn toggle_swaps_presets() {
        let mut p = ProfileConfig::field();
        p.toggle();
        assert_eq!(p.profile, GuiProfile::Office);
        p.toggle();
        assert_eq!(p.profile, GuiProfile::Field);
    }

    #[test]
    fn parse_profile_names() {
        assert_eq!(GuiProfile::parse("FIELD"), Some(GuiProfile::Field));
        assert_eq!(GuiProfile::parse("office"), Some(GuiProfile::Office));
        assert_eq!(GuiProfile::parse("nope"), None);
    }

    #[test]
    fn degrade_disables_gpu() {
        let mut p = ProfileConfig::office();
        p.degrade(DegradationReason::NoGpu);
        assert!(p.force_soft);
        assert!(!p.should_try_gpu());
        assert_eq!(p.suggested_present(), SuggestedPresent::Softbuffer);
        assert!(!p.effects_enabled);
    }

    #[test]
    fn intervals_are_positive() {
        let f = ProfileConfig::field();
        assert!(f.frame_interval() > Duration::ZERO);
        assert!(f.chart_interval() >= f.frame_interval() || f.chart_refresh_hz < f.max_fps);
        let o = ProfileConfig::office();
        assert!(o.frame_interval() < f.frame_interval());
    }

    #[test]
    fn from_env_default_office_when_unset() {
        // Do not set env here (parallel tests); just ensure API exists and returns a valid config.
        let p = ProfileConfig::office();
        assert_eq!(p.name(), "Office");
        let _ = ProfileConfig::from_env();
    }
}
