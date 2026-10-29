//! Default-application requests. Launchers run off the event thread and are always reaped.

use crate::PlatformError;
use std::{ffi::OsString, path::Path, sync::mpsc};

/// An asynchronous default-application launch, not the lifetime of the opened document.
pub struct OpenRequest(mpsc::Receiver<Result<(), PlatformError>>);

impl OpenRequest {
    /// Open a local file or directory. Relative paths resolve against the current directory.
    pub fn path(path: &Path) -> Result<Self, PlatformError> {
        if path.as_os_str().is_empty() {
            return Err(PlatformError::new("Empty file path"));
        }
        let path =
            std::path::absolute(path).map_err(|error| PlatformError::new(error.to_string()))?;
        Self::start(path.into_os_string())
    }

    /// Open an explicit URI with its registered handler. No shell is used.
    pub fn url(url: &str) -> Result<Self, PlatformError> {
        if !valid_uri(url) {
            return Err(PlatformError::new("Expected a URI with a valid scheme"));
        }
        Self::start(url.into())
    }

    fn start(target: OsString) -> Result<Self, PlatformError> {
        let (send, receive) = mpsc::channel();
        std::thread::Builder::new()
            .name("desktop-open".into())
            .spawn(move || {
                let _ = send.send(open(&target));
            })
            .map_err(|error| PlatformError::new(error.to_string()))?;
        Ok(Self(receive))
    }

    /// Poll once. `None` means the launcher has not returned; call again later.
    /// Some Linux handlers remain active until their application exits.
    /// Consume this request after receiving a result.
    pub fn poll(&self) -> Option<Result<(), PlatformError>> {
        match self.0.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(PlatformError::new(
                "Desktop launcher stopped without a result",
            ))),
        }
    }
}

fn valid_uri(url: &str) -> bool {
    let Some((scheme, value)) = url.split_once(':') else {
        return false;
    };
    !value.is_empty()
        && !url.chars().any(char::is_control)
        && scheme
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+.-".contains(&byte))
}

#[cfg(not(target_os = "windows"))]
fn open(target: &std::ffi::OsStr) -> Result<(), PlatformError> {
    let mut command = command(target);
    let status = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| PlatformError::new(error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(PlatformError::new(format!(
            "Desktop open failed ({status})"
        )))
    }
}

#[cfg(not(target_os = "windows"))]
fn command(target: &std::ffi::OsStr) -> std::process::Command {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("/usr/bin/open");
    #[cfg(target_os = "macos")]
    command.arg("--");
    #[cfg(not(target_os = "macos"))]
    let mut command = std::process::Command::new("xdg-open");
    command.arg(target);
    command
}

#[cfg(target_os = "windows")]
fn open(target: &std::ffi::OsStr) -> Result<(), PlatformError> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            window: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> *mut std::ffi::c_void;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut std::ffi::c_void, flags: u32) -> i32;
        fn CoUninitialize();
    }
    let mut target: Vec<u16> = target.encode_wide().collect();
    if target.contains(&0) {
        return Err(PlatformError::new("File/URL contains NUL"));
    }
    target.push(0);
    // Default handlers can be COM shell extensions; this dedicated thread owns its STA.
    let initialized = unsafe { CoInitializeEx(ptr::null_mut(), 6) };
    if initialized < 0 {
        return Err(PlatformError::new(format!(
            "Desktop COM initialization failed ({initialized:#x})"
        )));
    }
    // ShellExecute's result is a historical integer, not an owned process handle.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            ptr::null(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            1,
        )
    } as isize;
    unsafe {
        CoUninitialize();
    }
    if result > 32 {
        Ok(())
    } else {
        Err(PlatformError::new(format!(
            "Desktop open failed (ShellExecuteW {result})"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_and_launcher_arguments_cannot_become_options_or_shell_commands() {
        for text in [
            "",
            "-a Calculator",
            "file",
            ":abc",
            "1bad:value",
            "https:",
            "https://a\n-b",
            "file:a\0b",
        ] {
            assert!(!valid_uri(text), "{text:?}");
            assert!(OpenRequest::url(text).is_err());
        }
        for text in [
            "https://example.invalid/a?q=1&b=2",
            "file:///tmp/a%20b",
            "mailto:user@example.invalid",
            "custom+one.foo-bar:value",
        ] {
            assert!(valid_uri(text));
        }
        assert!(OpenRequest::path(Path::new("")).is_err());
        #[cfg(not(target_os = "windows"))]
        {
            let target = std::ffi::OsStr::new("/tmp/--a; $(echo nope) '中文'.txt");
            let command = command(target);
            let args: Vec<_> = command.get_args().collect();
            assert_eq!(args.last().copied(), Some(target));
            assert_eq!(args.len(), if cfg!(target_os = "macos") { 2 } else { 1 });
            let (send, receive) = mpsc::channel();
            let request = OpenRequest(receive);
            assert!(request.poll().is_none());
            send.send(Ok(())).unwrap();
            assert!(request.poll().unwrap().is_ok());
            drop(send);
            assert!(request.poll().unwrap().is_err());
        }
    }
}
