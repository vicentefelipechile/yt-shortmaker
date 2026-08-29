// =================================================================================================
// setup — Dependency checks and auto-provisioning (fully automatic, no user action)
// =================================================================================================

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Context, Result};

static INSTALLING: AtomicBool = AtomicBool::new(false);

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
        "linux" => msg.push_str(" The app is installing them automatically..."),
        "windows" => msg.push_str(" The app is installing them automatically..."),
        "macos" => msg.push_str(" The app is installing them automatically..."),
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
// Auto-install — fully automatic, no button required, with progress callback for toast
// -------------------------------------------------------------------------------------------------

/// Ensures external tools are present, auto-downloading/installing them if needed.
/// Progress is reported via `on_progress` for toast UI. Guarded against concurrent runs.
pub async fn ensure_tools_with_progress<F>(on_progress: F) -> Result<()>
where
    F: Fn(String) + Send + 'static,
{
    if INSTALLING.swap(true, Ordering::SeqCst) {
        tracing::info!("ensure_tools: already installing, skipping concurrent call");
        return Ok(());
    }
    let res = ensure_tools_inner(&on_progress).await;
    INSTALLING.store(false, Ordering::SeqCst);
    res
}

async fn ensure_tools_inner<F>(on_progress: &F) -> Result<()>
where
    F: Fn(String) + Send,
{
    let status = dependency_status();
    if status.is_ok() {
        return Ok(());
    }
    tracing::info!(
        "ensure_tools: missing {:?}, starting auto-install",
        status.missing()
    );
    on_progress(format!("Instalando {}...", status.missing().join(", ")));

    if !status.ytdlp {
        on_progress("Descargando yt-dlp...".to_string());
        ensure_yt_dlp(on_progress)
            .await
            .with_context(|| "auto-installing yt-dlp")?;
        on_progress("yt-dlp listo".to_string());
    }
    if !dependency_status().ffmpeg || !dependency_status().ffprobe {
        on_progress("Instalando ffmpeg...".to_string());
        ensure_ffmpeg(on_progress)
            .await
            .with_context(|| "auto-installing ffmpeg/ffprobe")?;
        on_progress("ffmpeg listo".to_string());
    }

    let after = dependency_status();
    if after.is_ok() {
        tracing::info!("ensure_tools: all tools now available");
        on_progress("Herramientas listas".to_string());
        return Ok(());
    }
    Err(anyhow!(format_missing_message(&after.missing())))
}

/// Simple wrapper without progress (uses tracing only).
pub async fn ensure_tools() -> Result<()> {
    ensure_tools_with_progress(|msg| tracing::info!("install progress: {msg}")).await
}

/// Blocking variant for callers without a runtime (spawns a temporary one).
pub fn ensure_tools_blocking() -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("creating tokio runtime for ensure_tools")?;
    rt.block_on(ensure_tools())
}

async fn ensure_yt_dlp<F>(on_progress: &F) -> Result<()>
where
    F: Fn(String) + Send,
{
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
    download_file_with_progress(url, &dest, on_progress)
        .await
        .with_context(|| format!("downloading yt-dlp from {url}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&dest).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&dest, perms).await?;
    }

    // Verify with retry (Windows antivirus may lock file briefly)
    for attempt in 0..3 {
        if is_available_path(&dest, &["--version"]) {
            tracing::info!("yt-dlp installed at {}", dest.display());
            return Ok(());
        }
        if attempt < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    let bytes = tokio::fs::metadata(&dest)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    anyhow::bail!(
        "yt-dlp downloaded to {} ({} bytes) but --version check failed. Try running it manually.",
        dest.display(),
        bytes
    );
}

async fn ensure_ffmpeg<F>(on_progress: &F) -> Result<()>
where
    F: Fn(String) + Send,
{
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
        "windows" => ensure_ffmpeg_windows(on_progress).await,
        "macos" => ensure_ffmpeg_macos(on_progress).await,
        "linux" => ensure_ffmpeg_linux(on_progress).await,
        _ => anyhow::bail!("auto-install of ffmpeg not supported on {os}"),
    }
}

async fn ensure_ffmpeg_windows<F>(on_progress: &F) -> Result<()>
where
    F: Fn(String) + Send,
{
    on_progress("Intentando instalar ffmpeg vía winget...".to_string());
    if try_winget_install("Gyan.FFmpeg").await {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if dependency_status().ffmpeg && dependency_status().ffprobe {
            tracing::info!("ffmpeg installed via winget");
            return Ok(());
        }
        tracing::warn!("winget reported success but ffmpeg still not in PATH");
    }
    // Fallback: download ffmpeg zip directly (automatic, no user action)
    on_progress("Descargando ffmpeg (esto puede tardar)...".to_string());
    let dir = tools_dir()?;
    let zip_url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";
    let zip_path = dir.join("ffmpeg.zip");
    tracing::info!("downloading ffmpeg zip from {zip_url}");
    download_file_with_progress(zip_url, &zip_path, on_progress)
        .await
        .context("downloading ffmpeg zip")?;
    on_progress("Extrayendo ffmpeg...".to_string());
    extract_ffmpeg_zip(&zip_path, &dir)
        .await
        .context("extracting ffmpeg zip")?;
    let _ = tokio::fs::remove_file(&zip_path).await;
    if dependency_status().ffmpeg && dependency_status().ffprobe {
        tracing::info!("ffmpeg installed via zip");
        return Ok(());
    }
    anyhow::bail!(
        "ffmpeg zip extracted but binaries still not found in {}",
        dir.display()
    )
}

async fn extract_ffmpeg_zip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let zip_path = zip_path.to_owned();
    let dest_dir = dest_dir.to_owned();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path).context("opening ffmpeg zip")?;
        let mut archive = zip::ZipArchive::new(file).context("reading ffmpeg zip")?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).context("reading zip entry")?;
            let name = entry.name().to_owned();
            // We only need bin/ffmpeg.exe and bin/ffprobe.exe
            if name.ends_with("bin/ffmpeg.exe") || name.ends_with("bin/ffprobe.exe") {
                let bin_name = if name.contains("ffmpeg.exe") {
                    "ffmpeg.exe"
                } else {
                    "ffprobe.exe"
                };
                let out_path = dest_dir.join(bin_name);
                let mut out = std::fs::File::create(&out_path)
                    .with_context(|| format!("creating {}", out_path.display()))?;
                std::io::copy(&mut entry, &mut out).context("extracting bin")?;
                tracing::info!("extracted {bin_name} to {}", out_path.display());
            }
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .context("spawn_blocking for zip")?
}

async fn ensure_ffmpeg_macos<F>(on_progress: &F) -> Result<()>
where
    F: Fn(String) + Send,
{
    on_progress("Instalando ffmpeg vía brew...".to_string());
    if try_brew_install("ffmpeg").await {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if dependency_status().ffmpeg {
            return Ok(());
        }
    }
    anyhow::bail!("ffmpeg auto-install via brew failed. Run: brew install ffmpeg")
}

async fn ensure_ffmpeg_linux<F>(_on_progress: &F) -> Result<()>
where
    F: Fn(String) + Send,
{
    anyhow::bail!(
        "ffmpeg not found. The app will try apt, but if it fails run: sudo apt update && sudo apt install -y ffmpeg"
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

async fn download_file_with_progress<F>(url: &str, dest: &Path, on_progress: &F) -> Result<()>
where
    F: Fn(String) + Send,
{
    let client = reqwest::Client::builder()
        .user_agent("yt-shortmaker/2.0")
        .build()
        .context("building reqwest client")?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("download failed: HTTP {} for {url}", resp.status());
    }
    let total = resp.content_length().unwrap_or(0);
    tracing::info!("downloading {url} total={total} bytes");
    let bytes = {
        let mut buf = Vec::new();
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_pct: u64 = 0;
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("reading download chunk")?;
            buf.extend_from_slice(&chunk);
            downloaded += chunk.len() as u64;
            if total > 0 {
                let pct = downloaded * 100 / total;
                if pct != last_pct && pct % 10 == 0 {
                    last_pct = pct;
                    on_progress(format!("Descargando... {pct}%"));
                    tracing::info!("download {pct}% {downloaded}/{total}");
                }
            } else if downloaded % (512 * 1024) == 0 {
                on_progress(format!("Descargando... {} KB", downloaded / 1024));
            }
        }
        buf
    };
    tracing::info!("download complete: {} bytes from {url}", bytes.len());
    if bytes.len() < 1024 {
        let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(500)]);
        anyhow::bail!(
            "download too small ({} bytes), likely error page: {}",
            bytes.len(),
            snippet
        );
    }
    if bytes.starts_with(b"<!DOCTYPE") || bytes.starts_with(b"<html") {
        let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(500)]);
        anyhow::bail!("download returned HTML, not binary: {}", snippet);
    }
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
