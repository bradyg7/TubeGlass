//! Finding out whether a newer TubeGlass has been published on GitHub, and downloading it.
//!
//! Releases are read from the public GitHub API (no account or token). The page that shows the
//! result lives in `ui/update_view.rs`; the scheduling is in `app/update.rs`.

use serde_json::Value;
use std::io::{Read, Write};
use std::path::PathBuf;

/// The GitHub project releases are published on, taken from `repository` in Cargo.toml.
pub fn repo() -> &'static str {
    env!("CARGO_PKG_REPOSITORY").trim_end_matches('/').trim_start_matches("https://github.com/")
}

/// The version of the running app.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// One file attached to a release.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// A published release.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// "0.2.0" (without the leading `v`)
    pub version: String,
    /// What changed, as written on the release page.
    pub notes: String,
    /// The release's web page.
    pub page: String,
    pub assets: Vec<Asset>,
}

/// Splits "v1.2.3-beta.1" into its numbers and whether it is a pre-release.
fn parse_version(v: &str) -> (Vec<u64>, bool) {
    let v = v.trim().trim_start_matches(['v', 'V']);
    // build metadata ("+abc") does not count; a "-" suffix marks a pre-release
    let v = v.split('+').next().unwrap_or(v);
    let (core, pre) = match v.split_once('-') {
        Some((c, _)) => (c, true),
        None => (v, false),
    };
    (core.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect(), pre)
}

/// True when `remote` is a higher version than `local` (a pre-release ranks below its release).
pub fn is_newer(remote: &str, local: &str) -> bool {
    let ((mut r, rpre), (mut l, lpre)) = (parse_version(remote), parse_version(local));
    let n = r.len().max(l.len());
    r.resize(n, 0);
    l.resize(n, 0);
    match r.cmp(&l) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        // same numbers: 1.0.0 is newer than 1.0.0-beta
        std::cmp::Ordering::Equal => lpre && !rpre,
    }
}

/// Reads GitHub's "latest release" answer.
pub fn parse_release(body: &str) -> Result<Release, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("unreadable answer: {e}"))?;
    let tag = v["tag_name"].as_str().ok_or("the release has no version")?;
    let assets = v["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some(Asset {
                name: a["name"].as_str()?.to_string(),
                url: a["browser_download_url"].as_str()?.to_string(),
                size: a["size"].as_u64().unwrap_or(0),
            })
        })
        .collect();
    Ok(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        notes: v["body"].as_str().unwrap_or("").trim().to_string(),
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        assets,
    })
}

/// The address of the "latest release" answer. `TUBEGLASS_UPDATE_API` replaces it, which is how
/// the update flow is tried out against a local server.
fn api_url() -> String {
    std::env::var("TUBEGLASS_UPDATE_API").unwrap_or_else(|_| format!("https://api.github.com/repos/{}/releases/latest", repo()))
}

/// Asks GitHub for the newest published release. `Ok(None)` means nothing is published yet.
pub fn fetch_latest() -> Result<Option<Release>, String> {
    let url = api_url();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut resp = agent
        .get(&url)
        .header("User-Agent", concat!("TubeGlass/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("could not reach GitHub: {e}"))?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().map_err(|e| e.to_string())?;
    match status {
        200 => parse_release(&body).map(Some),
        404 => Ok(None),
        403 | 429 => Err("GitHub is limiting requests right now, try again later".into()),
        s => Err(format!("GitHub answered with status {s}")),
    }
}

/// Which kind of installation is running, which decides what kind of update file to fetch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Install {
    /// A single AppImage file that can replace itself.
    AppImage,
    /// Installed from a .deb (under /usr).
    Deb,
    /// Anything else on Linux (a tarball, `cargo install`, a build tree).
    LinuxOther,
    Windows,
    MacOs,
}

/// Works out the running installation from the operating system and the executable's path.
pub fn detect_install() -> Install {
    if cfg!(windows) {
        return Install::Windows;
    }
    if cfg!(target_os = "macos") {
        return Install::MacOs;
    }
    if std::env::var_os("APPIMAGE").is_some() {
        return Install::AppImage;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    if exe.starts_with("/usr/") {
        Install::Deb
    } else {
        Install::LinuxOther
    }
}

/// The release file that suits this computer (`arch` is Rust's `ARCH`: "x86_64", "aarch64").
pub fn pick_asset<'a>(assets: &'a [Asset], install: Install, arch: &str) -> Option<&'a Asset> {
    let has = |a: &Asset, parts: &[&str]| parts.iter().all(|p| a.name.to_lowercase().contains(p));
    let ends = |a: &Asset, ext: &str| a.name.to_lowercase().ends_with(ext);
    let deb_arch = if arch == "aarch64" { "arm64" } else { "amd64" };
    let find = |f: &dyn Fn(&Asset) -> bool| assets.iter().find(|a| f(a));
    match install {
        Install::AppImage => find(&|a| ends(a, ".appimage") && has(a, &[arch])),
        Install::Deb => find(&|a| ends(a, ".deb") && has(a, &[deb_arch])).or_else(|| find(&|a| ends(a, ".tar.gz") && has(a, &["linux", arch]))),
        Install::LinuxOther => find(&|a| ends(a, ".tar.gz") && has(a, &["linux", arch])),
        Install::Windows => find(&|a| ends(a, ".zip") && has(a, &["windows", arch])),
        Install::MacOs => find(&|a| ends(a, ".dmg") && has(a, &["macos", arch])).or_else(|| find(&|a| ends(a, ".zip") && has(a, &["macos", arch]))),
    }
}

/// Only files published on this project's releases are downloaded.
pub fn trusted(url: &str) -> bool {
    if let Ok(api) = std::env::var("TUBEGLASS_UPDATE_API") {
        // testing against a local server: its own files are fine
        if let Some(origin) = api.split('/').take(3).collect::<Vec<_>>().join("/").into() {
            if url.starts_with(&format!("{origin}/")) {
                return true;
            }
        }
    }
    url.starts_with(&format!("https://github.com/{}/releases/download/", repo()))
}

/// Downloads a release file into the user's Downloads folder, reporting progress (0–1).
/// The file is written as `.part` and renamed once it is complete and has the expected size.
pub fn download(asset: &Asset, on_progress: &dyn Fn(f32)) -> Result<PathBuf, String> {
    if !trusted(&asset.url) {
        return Err("refusing to download from an unexpected address".into());
    }
    let dir = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."));
    let target = dir.join(&asset.name);
    let part = dir.join(format!("{}.part", asset.name));
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(900))).build().into();
    let mut resp = agent
        .get(&asset.url)
        .header("User-Agent", concat!("TubeGlass/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let total = if asset.size > 0 { asset.size } else { resp.body().content_length().unwrap_or(0) };
    let mut reader = resp.body_mut().as_reader();
    let mut out = std::fs::File::create(&part).map_err(|e| format!("cannot write {}: {e}", part.display()))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        if total > 0 {
            on_progress((got as f32 / total as f32).min(1.0));
        }
    }
    drop(out);
    if asset.size > 0 && got != asset.size {
        let _ = std::fs::remove_file(&part);
        return Err(format!("the download is incomplete ({got} of {} bytes)", asset.size));
    }
    std::fs::rename(&part, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

/// Replaces the running AppImage with the downloaded one. Returns the AppImage's path.
pub fn replace_appimage(new_file: &std::path::Path) -> Result<PathBuf, String> {
    let current = std::env::var_os("APPIMAGE").map(PathBuf::from).ok_or("not running as an AppImage")?;
    let staging = current.with_extension("AppImage.new");
    std::fs::copy(new_file, &staging).map_err(|e| format!("cannot write next to the AppImage: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    // a running file can be replaced on Linux: the old copy lives on until the app exits
    std::fs::rename(&staging, &current).map_err(|e| e.to_string())?;
    Ok(current)
}

/// Installs a downloaded Linux tarball over the running program (when its folder can be written):
/// unpacks it and swaps in the new `tubeglass`. Returns the program's path.
pub fn install_tarball(archive: &std::path::Path) -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let work = std::env::temp_dir().join(format!("tubeglass-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let run = || -> Result<(), String> {
        let st = crate::platform::command("tar").arg("-xzf").arg(archive).arg("-C").arg(&work).status().map_err(|e| format!("cannot run tar: {e}"))?;
        if !st.success() {
            return Err("the downloaded archive could not be unpacked".into());
        }
        let new = std::fs::read_dir(&work)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|d| d.path().join("tubeglass"))
            .find(|p| p.is_file())
            .ok_or("the archive holds no program")?;
        let staging = current.with_extension("new");
        std::fs::copy(&new, &staging).map_err(|e| format!("cannot write next to the program: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        }
        // a running file can be replaced on Linux: the old copy lives on until the app exits
        std::fs::rename(&staging, &current).map_err(|e| e.to_string())
    };
    let r = run();
    let _ = std::fs::remove_dir_all(&work);
    r.map(|_| current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset { name: name.into(), url: format!("https://github.com/{}/releases/download/v1/{name}", repo()), size: 10 }
    }

    #[test]
    fn versions_compare() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        // a pre-release is older than its release
        assert!(is_newer("1.0.0", "1.0.0-beta.1"));
        assert!(!is_newer("1.0.0-beta.1", "1.0.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn a_release_is_read() {
        let body = r#"{"tag_name":"v0.2.0","body":"Fixes things","html_url":"https://github.com/x/y/releases/tag/v0.2.0",
            "assets":[{"name":"a.zip","size":5,"browser_download_url":"https://github.com/x/y/releases/download/v0.2.0/a.zip"},{"name":"bad"}]}"#;
        let r = parse_release(body).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.notes, "Fixes things");
        assert_eq!(r.assets.len(), 1);
        assert!(parse_release("{}").is_err());
        assert!(parse_release("not json").is_err());
    }

    #[test]
    fn the_right_file_is_chosen() {
        let all: Vec<Asset> = [
            "TubeGlass-0.2.0-linux-x86_64.AppImage",
            "tubeglass_0.2.0_amd64.deb",
            "TubeGlass-0.2.0-linux-x86_64.tar.gz",
            "TubeGlass-0.2.0-windows-x86_64.zip",
            "TubeGlass-0.2.0-macos-aarch64.dmg",
            "TubeGlass-0.2.0-macos-x86_64.dmg",
        ]
        .iter()
        .map(|n| asset(n))
        .collect();
        let name = |i, a| pick_asset(&all, i, a).map(|x| x.name.clone());
        assert_eq!(name(Install::AppImage, "x86_64").unwrap(), "TubeGlass-0.2.0-linux-x86_64.AppImage");
        assert_eq!(name(Install::Deb, "x86_64").unwrap(), "tubeglass_0.2.0_amd64.deb");
        assert_eq!(name(Install::LinuxOther, "x86_64").unwrap(), "TubeGlass-0.2.0-linux-x86_64.tar.gz");
        assert_eq!(name(Install::Windows, "x86_64").unwrap(), "TubeGlass-0.2.0-windows-x86_64.zip");
        assert_eq!(name(Install::MacOs, "aarch64").unwrap(), "TubeGlass-0.2.0-macos-aarch64.dmg");
        assert_eq!(name(Install::MacOs, "x86_64").unwrap(), "TubeGlass-0.2.0-macos-x86_64.dmg");
        assert!(name(Install::AppImage, "aarch64").is_none());
    }

    #[test]
    fn only_project_files_are_trusted() {
        assert!(trusted(&asset("a.zip").url));
        assert!(!trusted("https://evil.example/releases/download/a.zip"));
        assert!(!trusted("https://github.com/someone/else/releases/download/v1/a.zip"));
    }

    #[test]
    #[ignore = "network"]
    fn the_real_repo_answers() {
        // either nothing is published yet (Ok(None)) or a release that parses
        let r = fetch_latest().expect("GitHub answered");
        println!("latest: {:?}", r.map(|r| r.version));
    }
}
