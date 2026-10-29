//! Installed font metadata and bounded face cache, shared by font-aware widgets.
use crate::{Font, FontError};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Debug)]
struct Face {
    family: String,
    aliases: Vec<String>,
    path: PathBuf,
    index: u32,
    weight: u16,
    italic: bool,
}

struct CachedFace {
    index: usize,
    bold: bool,
    italic: bool,
    font: Result<Font, FontError>,
}

#[derive(Default)]
struct Catalog {
    faces: Vec<Face>,
    families: Vec<String>,
    cache: Mutex<VecDeque<CachedFace>>,
}

static SYSTEM_FONTS: OnceLock<Catalog> = OnceLock::new();

/// Installed TrueType/OpenType family names, sorted without case distinctions.
/// Metadata is read once, lazily. Font files are loaded only when requested.
pub fn system_font_families() -> &'static [String] {
    &SYSTEM_FONTS.get_or_init(Catalog::system).families
}

/// Resolve an installed family (including localized aliases) and weight/slant.
/// Missing bold/italic faces are synthesized. Unknown families return an error so
/// the caller can preserve the requested name and choose its own fallback.
pub fn load_system_font(family: &str, bold: bool, italic: bool) -> Result<Font, FontError> {
    SYSTEM_FONTS
        .get_or_init(Catalog::system)
        .load(family, bold, italic)
}

/// Resolve an installed family at an OpenType weight between 1 and 1000.
/// The closest installed face with the requested slant is preferred. Missing bold
/// (600 or greater) and italic faces are synthesized as in [`load_system_font`].
pub fn load_system_font_with_weight(
    family: &str,
    weight: u16,
    italic: bool,
) -> Result<Font, FontError> {
    SYSTEM_FONTS
        .get_or_init(Catalog::system)
        .load_weight(family, weight, italic)
}

impl Catalog {
    fn system() -> Self {
        let mut catalog = Self::default();
        #[cfg(target_os = "macos")]
        let (files, native_families) = macos::installed();
        #[cfg(target_os = "macos")]
        for file in &files {
            catalog.read(file);
        }
        #[cfg(not(target_os = "macos"))]
        let files: Vec<PathBuf> = Vec::new();
        if files.is_empty() {
            for directory in font_directories() {
                catalog.scan(&directory);
            }
        }
        #[cfg(target_os = "macos")]
        if !native_families.is_empty() {
            catalog.faces.retain_mut(|face| {
                if let Some(family) = native_families
                    .iter()
                    .find(|family| face.aliases.contains(&family.to_lowercase()))
                {
                    face.family = family.clone();
                    true
                } else {
                    false
                }
            });
        }
        catalog.faces.sort_by(|a, b| {
            a.family
                .to_lowercase()
                .cmp(&b.family.to_lowercase())
                .then(a.path.cmp(&b.path))
                .then(a.index.cmp(&b.index))
        });
        catalog.families = catalog
            .faces
            .iter()
            .map(|face| face.family.clone())
            .collect();
        catalog
            .families
            .dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
        catalog
    }

    fn scan(&mut self, directory: &Path) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            // Do not follow directory links into cycles or unrelated trees.
            if kind.is_dir() {
                self.scan(&path);
            } else if path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| {
                    matches!(
                        value.to_ascii_lowercase().as_str(),
                        "ttf" | "ttc" | "otf" | "otc"
                    )
                })
            {
                self.read(&path);
            }
        }
    }

    fn read(&mut self, path: &Path) {
        // shortcut: ignore fonts above256 MiB; stream metadata if larger installed fonts are needed.
        if !path
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= 256 * 1024 * 1024)
        {
            return;
        }
        if let Ok(data) = std::fs::read(path) {
            self.add(path, &data);
        }
    }

    fn add(&mut self, path: &Path, data: &[u8]) {
        // A corrupt TTC count must not create a billions-of-faces scan.
        let count = ttf_parser::fonts_in_collection(data)
            .unwrap_or(1)
            .min(data.len() as u32 / 4);
        for index in 0..count {
            let Ok(face) = ttf_parser::Face::parse(data, index) else {
                continue;
            };
            let mut names: Vec<_> = face
                .names()
                .into_iter()
                .filter(|name| {
                    matches!(
                        name.name_id,
                        ttf_parser::name_id::TYPOGRAPHIC_FAMILY | ttf_parser::name_id::FAMILY
                    )
                })
                .filter_map(|name| {
                    name.to_string()
                        .or_else(|| {
                            (name.platform_id == ttf_parser::PlatformId::Macintosh
                                && name.encoding_id == 0
                                && name.name.iter().all(u8::is_ascii))
                            .then(|| String::from_utf8_lossy(name.name).into_owned())
                        })
                        .filter(|value| {
                            !value.trim().is_empty() && !value.chars().any(char::is_control)
                        })
                        .map(|value| {
                            (
                                (
                                    name.name_id != ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
                                    name.language_id != 0x409,
                                    name.language_id,
                                ),
                                value,
                            )
                        })
                })
                .collect();
            names.sort_by_key(|(rank, _)| *rank);
            let Some((_, family)) = names.first() else {
                continue;
            };
            self.faces.push(Face {
                family: family.clone(),
                aliases: names
                    .into_iter()
                    .map(|(_, name)| name.to_lowercase())
                    .collect(),
                path: path.into(),
                index,
                weight: face.weight().to_number(),
                italic: face.is_italic() || face.is_oblique(),
            });
        }
    }

    fn load(&self, family: &str, bold: bool, italic: bool) -> Result<Font, FontError> {
        self.load_weight(family, if bold { 700 } else { 400 }, italic)
    }

    fn load_weight(&self, family: &str, weight: u16, italic: bool) -> Result<Font, FontError> {
        if !(1..=1000).contains(&weight) {
            return Err(FontError("Font weight must be between 1 and 1000".into()));
        }
        let key = family.to_lowercase();
        let bold = weight >= 600;
        let index = self
            .faces
            .iter()
            .enumerate()
            .filter(|(_, face)| face.aliases.contains(&key))
            .min_by_key(|(_, face)| {
                (
                    u16::from(face.italic != italic),
                    face.weight.abs_diff(weight),
                )
            })
            .map(|(index, _)| index)
            .ok_or_else(|| FontError(format!("Font family is not installed: {family}")))?;
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(position) = cache
            .iter()
            .position(|entry| (entry.index, entry.bold, entry.italic) == (index, bold, italic))
        {
            let entry = cache.remove(position).unwrap();
            let font = entry.font.clone();
            cache.push_back(entry);
            return font;
        }
        drop(cache);
        let face = &self.faces[index];
        let font = Font::from_path(&face.path, face.index).map(|font| {
            font.with_synthetic_style(bold && face.weight < 600, italic && !face.italic)
        });
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        // Retain at most16 loaded faces; active widgets keep their own shared references.
        if cache.len() >= 16 {
            cache.pop_front();
        }
        cache.push_back(CachedFace {
            index,
            bold,
            italic,
            font: font.clone(),
        });
        font
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::ffi::{c_char, c_void, CStr};

    #[link(name = "CoreText", kind = "framework")]
    extern "C" {
        fn CTFontManagerCopyAvailableFontURLs() -> *const c_void;
        fn CTFontManagerCopyAvailableFontFamilyNames() -> *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFURLCopyFileSystemPath(url: *const c_void, style: isize) -> *const c_void;
        fn CFStringGetLength(string: *const c_void) -> isize;
        fn CFStringGetCString(
            string: *const c_void,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(value: *const c_void);
    }

    struct Owned(*const c_void);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CFRelease(self.0);
                }
            }
        }
    }

    unsafe fn string(value: *const c_void) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let size = CFStringGetLength(value).checked_mul(3)?.checked_add(1)?;
        if size <= 0 {
            return None;
        }
        let mut buffer = vec![0u8; size as usize];
        if CFStringGetCString(value, buffer.as_mut_ptr().cast(), size, 0x08000100) == 0 {
            return None;
        }
        CStr::from_ptr(buffer.as_ptr().cast())
            .to_str()
            .ok()
            .map(str::to_owned)
    }

    pub(super) fn installed() -> (Vec<PathBuf>, Vec<String>) {
        let mut files = Vec::new();
        let mut families = Vec::new();
        // Copy/Create references are owned here; array elements remain valid until release.
        unsafe {
            let urls = Owned(CTFontManagerCopyAvailableFontURLs());
            if !urls.0.is_null() {
                for index in 0..CFArrayGetCount(urls.0) {
                    let path = Owned(CFURLCopyFileSystemPath(
                        CFArrayGetValueAtIndex(urls.0, index),
                        0,
                    ));
                    if let Some(path) = string(path.0) {
                        files.push(PathBuf::from(path));
                    }
                }
            }
            let names = Owned(CTFontManagerCopyAvailableFontFamilyNames());
            if !names.0.is_null() {
                for index in 0..CFArrayGetCount(names.0) {
                    if let Some(name) = string(CFArrayGetValueAtIndex(names.0, index)) {
                        families.push(name);
                    }
                }
            }
        }
        files.sort();
        files.dedup();
        (files, families)
    }
}

fn font_directories() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if cfg!(target_os = "macos") {
        paths.extend(
            [
                "/System/Library/Fonts",
                "/Library/Fonts",
                "/Network/Library/Fonts",
            ]
            .map(PathBuf::from),
        );
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(home).join("Library/Fonts"));
        }
    } else if cfg!(target_os = "windows") {
        paths.push(
            PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
                .join("Fonts"),
        );
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            paths.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
        }
    } else {
        paths.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            paths.push(home.join(".fonts"));
            paths.push(
                PathBuf::from(
                    std::env::var_os("XDG_DATA_HOME")
                        .unwrap_or_else(|| home.join(".local/share").into_os_string()),
                )
                .join("fonts"),
            );
        }
        if let Some(directories) = std::env::var_os("XDG_DATA_DIRS") {
            paths.extend(std::env::split_paths(&directories).map(|path| path.join("fonts")));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_catalog_resolves_families_styles_aliases_and_caches_faces() {
        let started = std::time::Instant::now();
        let families = system_font_families();
        assert!(!families.is_empty());
        assert!(families
            .windows(2)
            .all(|pair| pair[0].to_lowercase() < pair[1].to_lowercase()));
        assert!(load_system_font("not-an-installed-family-4b276", false, false).is_err());
        let catalog = SYSTEM_FONTS.get().unwrap();
        #[cfg(target_os = "macos")]
        {
            let (_, native) = macos::installed();
            let missing: Vec<_> = native
                .iter()
                .filter(|family| !families.contains(family))
                .collect();
            eprintln!(
                "Native family coverage: {}/{}; missing: {:?}",
                native.len() - missing.len(),
                native.len(),
                missing
            );
        }
        let face = catalog
            .faces
            .iter()
            .find(|face| {
                face.family.contains("Menlo")
                    || face.family.contains("DejaVu Sans")
                    || face.family.contains("Consolas")
            })
            .unwrap_or(&catalog.faces[0]);
        let regular = catalog.load(&face.family, false, false).unwrap();
        let repeated = catalog
            .load(&face.family.to_uppercase(), false, false)
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(&regular.inner, &repeated.inner));
        assert!(catalog.load(&face.aliases[0], false, false).is_ok());
        let normal = regular.rasterize_text("Font ABC abc 123", 24.0, None).1;
        for (bold, italic) in [(true, false), (false, true), (true, true)] {
            let styled = catalog.load(&face.family, bold, italic).unwrap();
            let mask = styled.rasterize_text("Font ABC abc 123", 24.0, None).1;
            assert!(mask.pixels.iter().any(|pixel| *pixel > 0));
            assert!(normal.width != mask.width || normal.pixels != mask.pixels);
        }
        for weight in [0, 1001, u16::MAX] {
            assert!(load_system_font_with_weight(&face.family, weight, false).is_err());
        }
        let weighted = load_system_font_with_weight(&face.family, 400, false).unwrap();
        assert!(std::sync::Arc::ptr_eq(&regular.inner, &weighted.inner));
        for weight in [1, 100, 200, 300, 500, 600, 800, 900, 1000] {
            assert!(load_system_font_with_weight(&face.family, weight, false).is_ok());
        }
        let bold = load_system_font(&face.family, true, false).unwrap();
        let weighted = load_system_font_with_weight(&face.family, 700, false).unwrap();
        assert!(std::sync::Arc::ptr_eq(&bold.inner, &weighted.inner));
        let mut tested = std::collections::BTreeSet::new();
        for normal in catalog
            .faces
            .iter()
            .filter(|face| face.weight == 400 && !face.italic)
        {
            if tested.contains(&normal.family) {
                continue;
            }
            let Some(medium) = catalog
                .faces
                .iter()
                .find(|face| face.weight == 500 && !face.italic && face.family == normal.family)
            else {
                continue;
            };
            let regular = catalog.load_weight(&normal.family, 400, false).unwrap();
            let medium_font = catalog.load_weight(&normal.family, 500, false).unwrap();
            assert!(!std::sync::Arc::ptr_eq(&regular.inner, &medium_font.inner));
            let a = regular.rasterize_text("Medium ABC 123", 24.0, None).1;
            let b = medium_font.rasterize_text("Medium ABC 123", 24.0, None).1;
            assert!(a.width != b.width || a.pixels != b.pixels);
            let repeated = catalog
                .load_weight(&normal.family.to_uppercase(), 500, false)
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(&medium_font.inner, &repeated.inner));
            eprintln!(
                "Weighted face verified: {}400 -> {}500 ({}:{})",
                normal.family,
                medium.family,
                medium.path.display(),
                medium.index
            );
            tested.insert(normal.family.clone());
            if tested.len() == 3 {
                break;
            }
        }
        #[cfg(target_os = "macos")]
        assert!(
            !tested.is_empty(),
            "macOS installed regular/medium face coverage"
        );
        let mut isolated = Catalog::default();
        isolated.add(Path::new("bad.ttf"), b"invalid font");
        assert!(isolated.faces.is_empty());
        eprintln!(
            "{} installed families; metadata and four face loads in {:?}",
            families.len(),
            started.elapsed()
        );
    }
}
