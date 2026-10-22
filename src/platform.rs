//! Everything that differs between operating systems lives here, so the rest of the app can
//! stay the same on Linux, macOS and Windows.

use std::path::Path;
use std::process::Command;

/// A command for a helper program (ffmpeg, ffprobe). On Windows the console window that would
/// otherwise flash up for every process is suppressed.
pub fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        c.creation_flags(0x0800_0000);
    }
    c
}

/// Makes helper programs findable. An app started from the macOS Finder gets a bare PATH that
/// leaves out Homebrew, where ffmpeg usually lives.
pub fn prepare_environment() {
    if cfg!(target_os = "macos") {
        let mut parts: Vec<std::path::PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"].iter().map(std::path::PathBuf::from).collect();
        if let Some(p) = std::env::var_os("PATH") {
            parts.extend(std::env::split_paths(&p));
        }
        if let Ok(joined) = std::env::join_paths(parts) {
            // SAFETY: called once at start-up, before any other thread exists
            unsafe { std::env::set_var("PATH", joined) };
        }
    }
}

/// True when `ffmpeg` can be started.
pub fn ffmpeg_available() -> bool {
    command("ffmpeg").arg("-version").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

/// How to install ffmpeg on this system, as one line of advice.
pub fn ffmpeg_hint() -> &'static str {
    if cfg!(windows) {
        "Install it with:  winget install ffmpeg   (then restart TubeGlass)"
    } else if cfg!(target_os = "macos") {
        "Install it with:  brew install ffmpeg   (then restart TubeGlass)"
    } else {
        "Install it with your package manager, e.g.  sudo apt install ffmpeg  or  sudo pacman -S ffmpeg"
    }
}

/// Opens a file or folder with whatever the desktop uses for it (file manager, default player).
pub fn open_path(path: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = command(program).arg(path).spawn();
}

/// Opens a web address in the default browser.
pub fn open_url(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = command(program).arg(url).spawn();
}

/// Ends a process immediately (used to cancel clip cutting).
pub fn kill_process(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        let mut c = command("taskkill");
        let _ = c.args(["/PID", &pid.to_string(), "/F", "/T"]).output();
    }
}

/// Freezes a process (pausing an ffmpeg download). Returns false where that is not possible,
/// in which case the caller simply lets it run.
pub fn suspend_process(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, libc::SIGSTOP) };
        true
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Continues a process frozen by [`suspend_process`].
pub fn resume_process(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGCONT);
    }
    #[cfg(not(unix))]
    let _ = pid;
}

/// Free bytes on the disk that holds `p` (0 when it cannot be determined).
pub fn free_space(p: &Path) -> u64 {
    let mut path = p.to_path_buf();
    while !path.exists() && path.pop() {}
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return 0 };
        unsafe {
            let mut st: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c.as_ptr(), &mut st) == 0 {
                st.f_bavail as u64 * st.f_frsize as u64
            } else {
                0
            }
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        unsafe extern "system" {
            fn GetDiskFreeSpaceExW(dir: *const u16, available: *mut u64, total: *mut u64, free: *mut u64) -> i32;
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        // SAFETY: `wide` is NUL-terminated and the three pointers outlive the call
        let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
        if ok != 0 { avail } else { 0 }
    }
}

/// True for an absolute path on disk (as opposed to an http address): `/home/x.mp4`,
/// `C:\Videos\x.mp4`.
pub fn is_local_path(s: &str) -> bool {
    s.starts_with('/') || Path::new(s).is_absolute()
}

/// A `file://` address for a path, as the image loader wants it.
pub fn file_uri(path: &str) -> String {
    if cfg!(windows) {
        format!("file:///{}", path.replace('\\', "/"))
    } else {
        format!("file://{path}")
    }
}

/// Local calendar day (`YYYY-MM-DD`) of a unix time.
pub fn local_day_key(t: u64) -> String {
    use chrono::{DateTime, Local};
    DateTime::from_timestamp(t as i64, 0)
        .map(|d| d.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Restarts the app: starts a new copy and ends this one without saving over fresh data.
pub fn restart() -> ! {
    // an AppImage runs from a temporary mount that disappears with it: start the file itself
    let exe = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).or_else(|| std::env::current_exe().ok());
    if let Some(exe) = exe {
        let _ = Command::new(exe).spawn();
    }
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_addresses() {
        assert!(is_local_path("/home/a/b.mp4"));
        assert!(!is_local_path("https://x/y"));
        assert!(!is_local_path("127.0.0.1:80/s"));
        #[cfg(not(windows))]
        assert_eq!(file_uri("/a/b.jpg"), "file:///a/b.jpg");
        assert_eq!(local_day_key(0).len(), 10);
    }
}
