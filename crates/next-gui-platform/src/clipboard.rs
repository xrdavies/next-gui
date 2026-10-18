//! OS text clipboard (thin wrapper around `arboard`).
//!
//! # Limits (MVP)
//! - **Text only** — no image / HTML.
//! - **Linux:** works with X11; Wayland may need a compositor clipboard path
//!   (`arboard`'s optional `wayland-data-control`) or may fail until then.
//! - On Linux the clipboard is often hosted by the writing process — keep a
//!   [`Clipboard`] alive (as [`crate::WindowCtx`] does via the host) so other
//!   apps can still read after `set_text`.
//! - Headless / no display: [`Clipboard::open`] returns [`crate::PlatformError`].

use crate::PlatformError;

/// Long-lived OS clipboard handle.
///
/// Prefer reusing one instance for the lifetime of a window (Linux hosts the
/// bytes until the writer drops).
pub struct Clipboard {
    inner: arboard::Clipboard,
}

impl Clipboard {
    /// Open the system clipboard.
    pub fn open() -> Result<Self, PlatformError> {
        arboard::Clipboard::new()
            .map(|inner| Self { inner })
            .map_err(|e| PlatformError::new(format!("clipboard open: {e}")))
    }

    /// Read UTF-8 text. `Ok(None)` when the clipboard is empty or has no text.
    pub fn get_text(&mut self) -> Result<Option<String>, PlatformError> {
        match self.inner.get_text() {
            Ok(s) => Ok(Some(s)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(PlatformError::new(format!("clipboard get: {e}"))),
        }
    }

    /// Replace clipboard contents with UTF-8 text.
    pub fn set_text(&mut self, text: &str) -> Result<(), PlatformError> {
        self.inner
            .set_text(text.to_owned())
            .map_err(|e| PlatformError::new(format!("clipboard set: {e}")))
    }
}

/// One-shot get (opens a temporary clipboard). Prefer [`Clipboard`] on a live window.
pub fn clipboard_get_text() -> Result<Option<String>, PlatformError> {
    Clipboard::open()?.get_text()
}

/// One-shot set (opens a temporary clipboard). Prefer [`Clipboard`] on a live window
/// so Linux peers can still read after this call returns.
pub fn clipboard_set_text(text: &str) -> Result<(), PlatformError> {
    Clipboard::open()?.set_text(text)
}
