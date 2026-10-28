//! OS text, HTML and RGBA image clipboard (thin wrapper around `arboard`).
//!
//! # Limits (MVP)
//! - UTF-8 text, HTML with a plain-text alternative, and straight RGBA8 images.
//! - **Linux:** works with X11; Wayland may need a compositor clipboard path
//!   (`arboard`'s optional `wayland-data-control`) or may fail until then.
//! - **Windows / macOS:** text, HTML and images via `arboard` (same thin API).
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

    /// Read HTML, or `Ok(None)` if this clipboard has no HTML representation.
    pub fn get_html(&mut self) -> Result<Option<String>, PlatformError> {
        match self.inner.get().html() {
            Ok(html) => Ok(Some(html)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(PlatformError::new(format!("clipboard get HTML: {e}"))),
        }
    }

    /// Replace clipboard contents with HTML and its plain-text alternative.
    pub fn set_html(&mut self, html: &str, text: &str) -> Result<(), PlatformError> {
        self.inner.set_html(html.to_owned(), Some(text.to_owned()))
            .map_err(|e| PlatformError::new(format!("clipboard set HTML: {e}")))
    }

    /// Replace clipboard contents with UTF-8 text.
    pub fn set_text(&mut self, text: &str) -> Result<(), PlatformError> {
        self.inner
            .set_text(text.to_owned())
            .map_err(|e| PlatformError::new(format!("clipboard set: {e}")))
    }

    /// Read straight RGBA8 pixels, or `Ok(None)` when no image is available.
    /// Decoded images over 128 MiB or with invalid dimensions return an error.
    pub fn get_image(&mut self) -> Result<Option<next_gui_scene::RgbaImage>, PlatformError> {
        match self.inner.get_image() {
            Ok(image) => {
                let (width,height) = image_dimensions(image.width,image.height,image.bytes.len())?;
                next_gui_scene::RgbaImage::new(width,height,image.bytes.into_owned()).map(Some)
                    .ok_or_else(||PlatformError::new("clipboard image has invalid dimensions"))
            }
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(error) => Err(PlatformError::new(format!("clipboard get image: {error}"))),
        }
    }

    /// Replace clipboard contents with straight RGBA8 pixels, up to 128 MiB.
    /// Keep this clipboard handle alive so Linux peers can retrieve the image.
    pub fn set_image(&mut self, image: &next_gui_scene::RgbaImage) -> Result<(), PlatformError> {
        image_dimensions(image.width() as usize,image.height() as usize,image.pixels().len())?;
        self.inner.set_image(arboard::ImageData { width:image.width() as usize,height:image.height() as usize,bytes:std::borrow::Cow::Borrowed(image.pixels()) })
            .map_err(|error|PlatformError::new(format!("clipboard set image: {error}")))
    }
}

fn image_dimensions(width:usize,height:usize,len:usize) -> Result<(u32,u32),PlatformError> {
    let bytes=width.checked_mul(height).and_then(|n|n.checked_mul(4));
    if width==0 || height==0 || width>u32::MAX as usize || height>u32::MAX as usize || bytes!=Some(len) || len>128*1024*1024 {
        return Err(PlatformError::new("clipboard image must be valid RGBA8 within 128 MiB"));
    }
    Ok((width as u32,height as u32))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_dimensions_reject_invalid_overflow_and_oversized_buffers() {
        assert_eq!(image_dimensions(2,3,24).unwrap(),(2,3));
        for (width,height,len) in [(0,1,0),(1,0,0),(2,3,23),(usize::MAX,2,4),(u32::MAX as usize,2,0),(8192,8192,8192*8192*4)] {
            assert!(image_dimensions(width,height,len).is_err());
        }
    }

    #[test]
    fn owned_clipboard_image_roundtrip() {
        if std::env::var("NEXT_GUI_CLIPBOARD_IMAGE_QA").as_deref()!=Ok("1") {
            eprintln!("owned_clipboard_image_roundtrip: not_run (requires an owned disposable smoke marker)");
            return;
        }
        let mut clipboard=Clipboard::open().unwrap();
        let marker="next-gui-clipboard-smoke-7c9427c";
        assert_eq!(clipboard.get_text().unwrap().as_deref(),Some(marker),"Refuse to replace unowned clipboard contents");
        assert!(clipboard.get_html().unwrap().is_none());
        assert!(clipboard.get_image().unwrap().is_none());
        let image=next_gui_scene::RgbaImage::new(3,2,vec![255,0,0,128,0,255,0,255,0,0,255,255,11,22,33,255,44,55,66,255,0,0,0,0]).unwrap();
        let write=clipboard.set_image(&image);
        let read=write.and_then(|()|clipboard.get_image());
        clipboard.set_text(marker).unwrap();
        let actual=read.unwrap().expect("Expected clipboard image");
        assert_eq!((actual.width(),actual.height()),(3,2));
        assert_eq!(actual.pixels(),image.pixels());
        assert_eq!(clipboard.get_text().unwrap().as_deref(),Some(marker));
        eprintln!("Owned OS image clipboard: exact RGBA round trip, marker restored");
    }
}
