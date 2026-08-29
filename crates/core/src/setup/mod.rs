// =================================================================================================
// setup — Dependency checks and auto-provisioning (auto-install)
// =================================================================================================

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencyStatus {
    pub ffmpeg: bool,
    pub ffprobe: bool,
    pub ytdlp: bool,
}

impl DependencyStatus {
    pub fn is_ok(&self) -> bool {
        self.ffmpeg && self.ffprobe && self.ytdlp
    }

    pub fn missing(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if !self.ffmpeg {
            v.push("ffmpeg");
        }
        if !self.ffprobe {
            v.push("ffprobe");
        }
        if !self.ytdlp {
            v.push("yt-dlp");
        }
        v
    }
}

// -------------------------------------------------------------------------------------------------
// Paths — tools dir (app-local) + bin helpers
// -------------------------------------------------------------------------------------------------

/// Directory where the app auto-installs missing tools (yt-dlp, ffmpeg).
pub fn tools_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().context("resolving data_local_dir")?;
    Ok(base.join("yt-shortmaker-v2").join("tools"))
}

fn yt_dlp_tools_path() -> Result<PathBuf> {
    let dir = tools_dir()?;
    let bin = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    Ok(dir.join(bin))
}

fn ffmpeg_tools_path() -> Result<PathBuf> {
    let dir = tools_dir()?;
    let bin = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    Ok(dir.join(bin))
}

fn ffprobe_tools_path() -> Result<PathBuf> {
    let dir = tools_dir()?;
    let bin = if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
    Ok(dir.join(bin))
}

/// Resolved binary to invoke for yt-dlp — prefers the app-local copy if it exists and works,
/// otherwise falls back to the system `PATH` (`yt-dlp` / `yt-dlp.exe`).
pub fn yt_dlp_bin() -> String {
    if let Ok(p) = yt_dlp_tools_path() {
        if p.exists() && is_available_path(&p, &["--version"]) {
            return p.to_string_lossy().to_string();
        }
    }
    // Prefer plain name; Windows also probes .exe in dependency_status
    "yt-dlp".to_string()
}

/// Resolved binary for `ffmpeg` (app-local if present, else PATH).
pub fn ffmpeg_bin() -> String {
    if let Ok(p) = ffmpeg_tools_path() {
        if p.exists() && is_available_path(&p, &["-version"]) {
            return p.to_string_lossy().to_string();
        }
    }
    "ffmpeg".to_string()
}

/// Resolved binary for `ffprobe`.
pub fn ffprobe_bin() -> String {
    if let Ok(p) = ffprobe_tools_path() {
        if p.exists() && is_available_path(&p, &["-version"]) {
            return p.to_string_lossy().to_string();
        }
    }
    "ffprobe".to_string()
}

// -------------------------------------------------------------------------------------------------
// Status — checks PATH *and* app-local tools dir
// -------------------------------------------------------------------------------------------------

pub fn dependency_status() -> DependencyStatus {
    let ffmpeg_ok =
        is_available("ffmpeg", &["-version"]) || is_available_path_opt(ffmpeg_tools_path().ok());
    let ffprobe_ok =
        is_available("ffprobe", &["-version"]) || is_available_path_opt(ffprobe_tools_path().ok());
    let ytdlp_ok = is_available("yt-dlp", &["--version"])
        || is_available("yt-dlp.exe", &["--version"])
        || is_available_path_opt(yt_dlp_tools_path().ok());
    DependencyStatus {
        ffmpeg: ffmpeg_ok,
        ffprobe: ffprobe_ok,
        ytdlp: ytdlp_ok,
    }
}

fn is_available_path_opt(p: Option<PathBuf>) -> bool {
    if let Some(path) = p {
        if path.exists() {
            return is_available_path(&path, &["--version"])
                || is_available_path(&path, &["-version"]);
        }
    }
    false
}

pub fn check_dependencies() -> Result<()> {
    let status = dependency_status();
    if status.is_ok() {
        return Ok(());
    }
    Err(anyhow!(format_missing_message(&status.missing())))
}

pub fn format_missing_message(missing: &[&str]) -> String {
    let os = std::env::consts::OS;
    let mut msg = format!("Missing dependencies: {}.", missing.join(", "));
    match os {
        "linux" => msg.push_str(
            " The app will try to auto-install them. If it fails, run: sudo apt update && sudo apt install ffmpeg; pip install -U yt-dlp",
        ),
        "windows" => msg.push_str(
            " The app will try to auto-install them. If it fails, install ffmpeg from https://ffmpeg.org/download.html and yt-dlp from https://github.com/yt-dlp/yt-dlp/releases, then ensure both are in PATH. Or run: winget install Gyan.FFmpeg yt-dlp.yt-dlp",
        ),
        "macos" => msg.push_str(
            " The app will try to auto-install them. If it fails, run: brew install ffmpeg yt-dlp",
        ),
        _ => {}
    }
    msg
}

pub fn yt_dlp_missing_error() -> anyhow::Error {
    anyhow!(format_missing_message(&["yt-dlp"]))
}

pub fn ffmpeg_missing_error() -> anyhow::Error {
    let status = dependency_status();
    let mut missing = Vec::new();
    if !status.ffmpeg {
        missing.push("ffmpeg");
    }
    if !status.ffprobe {
        missing.push("ffprobe");
    }
    if missing.is_empty() {
        missing.push("ffmpeg/ffprobe");
    }
    anyhow!(format_missing_message(&missing))
}

// -------------------------------------------------------------------------------------------------
// Auto-install — the app installs its own dependencies (no user manual step)
// -------------------------------------------------------------------------------------------------

/// Ensures external tools are present, auto-downloading/installing them if needed.
/// This is the correct entry point for the app (not `check_dependencies` alone).
/// It is async because it may download `yt-dlp` and invoke `winget`.
pub async fn ensure_tools() -> Result<()> {
    let status = dependency_status();
    if status.is_ok() {
        return Ok(());
    }
    tracing::info!(
        "ensure_tools: missing {:?}, starting auto-install",
        status.missing()
    );

    if !status.ytdlp {
        ensure_yt_dlp()
            .await
            .with_context(|| "auto-installing yt-dlp")?;
    }
    if !dependency_status().ffmpeg || !dependency_status().ffprobe {
        ensure_ffmpeg()
            .await
            .with_context(|| "auto-installing ffmpeg/ffprobe")?;
    }

    let after = dependency_status();
    if after.is_ok() {
        tracing::info!("ensure_tools: all tools now available");
        return Ok(());
    }
    Err(anyhow!(format_missing_message(&after.missing())))
}

/// Blocking variant for callers without a runtime (spawns a temporary one).
pub fn ensure_tools_blocking() -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("creating tokio runtime for ensure_tools")?;
    rt.block_on(ensure_tools())
}

async fn ensure_yt_dlp() -> Result<()> {
    let dest = yt_dlp_tools_path()?;
    if dest.exists() && is_available_path(&dest, &["--version"]) {
        tracing::info!("yt-dlp already present at {}", dest.display());
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("creating tools dir")?;
    }
    let url = if cfg!(windows) {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    };
    tracing::info!("downloading yt-dlp from {url} to {}", dest.display());
    download_file(url, &dest)
        .await
        .with_context(|| format!("downloading yt-dlp from {url}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&dest).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&dest, perms).await?;
    }

    if !is_available_path(&dest, &["--version"]) {
        anyhow::bail!(
            "yt-dlp downloaded to {} but --version check failed",
            dest.display()
        );
    }
    tracing::info!("yt-dlp installed at {}", dest.display());
    Ok(())
}

async fn ensure_ffmpeg() -> Result<()> {
    // Already present via tools dir or PATH?
    if is_available_path_opt(ffmpeg_tools_path().ok())
        && is_available_path_opt(ffprobe_tools_path().ok())
    {
        return Ok(());
    }
    if is_available("ffmpeg", &["-version"]) && is_available("ffprobe", &["-version"]) {
        return Ok(());
    }

    let os = std::env::consts::OS;
    match os {
        "windows" => ensure_ffmpeg_windows().await,
        "macos" => ensure_ffmpeg_macos().await,
        "linux" => ensure_ffmpeg_linux().await,
        _ => anyhow::bail!("auto-install of ffmpeg not supported on {os}"),
    }
}

async fn ensure_ffmpeg_windows() -> Result<()> {
    // Try winget first — lightweight, no zip handling.
    if try_winget_install("Gyan.FFmpeg").await {
        // winget may need a new PATH; re-check after a short delay
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if dependency_status().ffmpeg && dependency_status().ffprobe {
            tracing::info!("ffmpeg installed via winget");
            return Ok(());
        }
        tracing::warn!("winget reported success but ffmpeg still not in PATH");
    }
    // Fallback: try to download ffmpeg essentials zip (requires zip crate)
    // To keep dependencies light, we delegate to manual instructions if winget fails.
    // The banner will show the manual URL but the attempt was automatic.
    anyhow::bail!(
        "ffmpeg auto-install via winget failed. Please run: winget install Gyan.FFmpeg --accept-package-agreements --accept-source-agreements -h, or download from https://ffmpeg.org/download.html"
    )
}

async fn ensure_ffmpeg_macos() -> Result<()> {
    if try_brew_install("ffmpeg").await {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if dependency_status().ffmpeg {
            return Ok(());
        }
    }
    anyhow::bail!("ffmpeg auto-install via brew failed. Run: brew install ffmpeg")
}

async fn ensure_ffmpeg_linux() -> Result<()> {
    // Try apt if available, but don't require sudo here — just attempt and report.
    // We call apt-get without sudo; if it fails, the user needs to run manually.
    // For now, surface manual instructions.
    anyhow::bail!(
        "ffmpeg not found. Auto-install on Linux requires: sudo apt update && sudo apt install -y ffmpeg"
    )
}

async fn try_winget_install(package: &str) -> bool {
    let output = tokio::process::Command::new("winget")
        .args([
            "install",
            "--id",
            package,
            "--accept-package-agreements",
            "--accept-source-agreements",
            "-h",
            "--disable-interactivity",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;
    match output {
        Ok(o) => {
            let ok = o.status.success();
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            tracing::info!(
                "winget install {package} exit {} stdout={} stderr={}",
                o.status,
                stdout,
                stderr
            );
            ok
        }
        Err(e) => {
            tracing::warn!("winget not available or failed to spawn: {e}");
            false
        }
    }
}

async fn try_brew_install(package: &str) -> bool {
    let output = tokio::process::Command::new("brew")
        .args(["install", package])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let resp = reqwest::get(url)
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("download failed: HTTP {}", resp.status());
    }
    let bytes = resp.bytes().await.context("reading download bytes")?;
    // Write atomically via temp + rename
    let tmp = dest.with_extension("tmp-download");
    tokio::fs::write(&tmp, &bytes)
        .await
        .with_context(|| format!("writing {}", tmp.display()))?;
    tokio::fs::rename(&tmp, dest)
        .await
        .with_context(|| format!("renaming to {}", dest.display()))?;
    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

fn is_available(bin: &str, args: &[&str]) -> bool {
    std::process::Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success())
}

fn is_available_path(path: &Path, args: &[&str]) -> bool {
    std::process::Command::new(path)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success())
}
