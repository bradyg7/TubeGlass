//! Getting ffmpeg onto the computer. Release packages already contain it; when it is missing
//! (a build from source, a `.deb` whose dependency was removed ...) a pinned, checksummed static
//! build is downloaded once into TubeGlass's own data folder.
//!
//! The list of builds is `packaging/ffmpeg-sources.tsv`, shared with the packaging scripts.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;

/// The build list, compiled into the program.
const SOURCES: &str = include_str!("../../packaging/ffmpeg-sources.tsv");

/// One downloadable ffmpeg build.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// "linux-x64", "darwin-arm64", "win32-x64" ...
    pub platform: String,
    pub url: String,
    /// SHA-256 of the downloaded (gzip) file, lower-case hex
    pub sha256: String,
}

/// Reads the build list (`platform <TAB> url <TAB> sha256` per line; `#` starts a comment).
pub fn parse_sources(text: &str) -> Vec<Source> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut it = l.split('\t');
            Some(Source { platform: it.next()?.to_string(), url: it.next()?.to_string(), sha256: it.next()?.to_lowercase() })
        })
        .collect()
}

/// The platform key of this computer in the build list.
pub fn platform_key() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os}-{arch}")
}

/// The build for this computer, if there is one.
pub fn source_for_this_system() -> Option<Source> {
    parse_sources(SOURCES).into_iter().find(|s| s.platform == platform_key())
}

/// Downloads, checks and unpacks ffmpeg into the data folder; returns the program's path.
/// `on_progress` gets 0–1 while the file comes in.
pub fn install(on_progress: &dyn Fn(f32)) -> Result<PathBuf, String> {
    let src = source_for_this_system().ok_or_else(|| format!("there is no ready-made ffmpeg for {}", platform_key()))?;
    let dir = crate::platform::managed_ffmpeg_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let gz = dir.join("ffmpeg.gz.part");
    let part = dir.join("ffmpeg.part");
    let target = dir.join(crate::platform::ffmpeg_file_name());

    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(1200))).build().into();
    let mut resp = agent
        .get(&src.url)
        .header("User-Agent", concat!("TubeGlass/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let total = resp.body().content_length().unwrap_or(0);
    let mut reader = resp.body_mut().as_reader();
    let mut out = std::fs::File::create(&gz).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 128 * 1024];
    let mut got = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        if total > 0 {
            // the last stretch (unpacking) is quick, so the download fills most of the bar
            on_progress((got as f32 / total as f32).min(1.0) * 0.95);
        }
    }
    drop(out);
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if digest != src.sha256 {
        let _ = std::fs::remove_file(&gz);
        return Err("the downloaded file does not match its checksum, so it was discarded".into());
    }

    // unpack the single executable
    let mut dec = flate2::read::GzDecoder::new(std::fs::File::open(&gz).map_err(|e| e.to_string())?);
    let mut exe = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    std::io::copy(&mut dec, &mut exe).map_err(|e| format!("cannot unpack ffmpeg: {e}"))?;
    drop(exe);
    let _ = std::fs::remove_file(&gz);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(&target);
    std::fs::rename(&part, &target).map_err(|e| e.to_string())?;
    crate::platform::refresh_ffmpeg();
    on_progress(1.0);
    if !crate::platform::ffmpeg_available() {
        return Err("the downloaded ffmpeg does not run on this computer".into());
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_list_is_complete_and_well_formed() {
        let all = parse_sources(SOURCES);
        for p in ["linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64"] {
            let s = all.iter().find(|s| s.platform == p).unwrap_or_else(|| panic!("no build for {p}"));
            assert!(s.url.starts_with("https://github.com/"), "{p}: {}", s.url);
            assert!(s.url.ends_with(".gz"));
            assert_eq!(s.sha256.len(), 64, "{p}");
            assert!(s.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let v = parse_sources("# note\n\nlinux-x64\thttps://x/y.gz\tABCD\nbroken line\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].sha256, "abcd");
    }

    #[test]
    fn explicit_choice_wins() {
        // SAFETY: tests in this module do not run other threads that read the environment
        unsafe { std::env::set_var("TUBEGLASS_FFMPEG", file!()) };
        crate::platform::refresh_ffmpeg();
        assert_eq!(crate::platform::ffmpeg_program(), std::path::PathBuf::from(file!()));
        unsafe { std::env::remove_var("TUBEGLASS_FFMPEG") };
        crate::platform::refresh_ffmpeg();
    }

    #[test]
    #[ignore = "downloads about 30 MB"]
    fn installs_a_working_ffmpeg() {
        let home = std::env::temp_dir().join("tg-ffmpeg-test");
        let _ = std::fs::remove_dir_all(&home);
        // SAFETY: set before anything else in this test looks at the environment
        unsafe {
            std::env::set_var("XDG_DATA_HOME", &home);
            std::env::set_var("HOME", &home);
        }
        let p = install(&|f| {
            if (f * 100.0) as u32 % 25 == 0 {
                eprintln!("progress {:.0}%", f * 100.0);
            }
        })
        .expect("install");
        let out = std::process::Command::new(&p).arg("-version").output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).starts_with("ffmpeg version"));
        assert!(crate::platform::ffmpeg_available());
    }
}
