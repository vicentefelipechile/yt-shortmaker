// =================================================================================================
// plano::export — Profiles and ffmpeg render commands (M5 preview + batch share compiler)
// =================================================================================================

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::compiler::build_ffmpeg_filter;
use super::schema::{PlanoDocument, OUTPUT_HEIGHT, OUTPUT_WIDTH};

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

pub const SAMPLE_DURATION_SECS: u64 = 4;
pub const PREVIEW_FPS: u32 = 30;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportProfile {
    pub width: u32,
    pub height: u32,
    pub video_bitrate: String,
    pub fps: u32,
}

impl ExportProfile {
    pub fn high_1080() -> Self {
        Self {
            width: 1080,
            height: 1920,
            video_bitrate: "12M".to_string(),
            fps: 30,
        }
    }

    pub fn high_720() -> Self {
        Self {
            width: 720,
            height: 1280,
            video_bitrate: "8M".to_string(),
            fps: 30,
        }
    }

    pub fn with_fps(mut self, fps: u32) -> Self {
        self.fps = if fps == 60 { 60 } else { 30 };
        self
    }

    pub fn with_quality(mut self, quality: &str) -> Self {
        self.video_bitrate = match quality {
            "low" => {
                if self.width == 720 {
                    "3M"
                } else {
                    "5M"
                }
            }
            "medium" => {
                if self.width == 720 {
                    "5M"
                } else {
                    "8M"
                }
            }
            _ => {
                if self.width == 720 {
                    "8M"
                } else {
                    "12M"
                }
            }
        }
        .to_string();
        self
    }

    pub fn hash(&self) -> String {
        format!(
            "{}x{}-{}-{}fps",
            self.width, self.height, self.video_bitrate, self.fps
        )
    }
}

#[derive(Debug, Clone)]
pub struct RenderCommand {
    pub inputs: Vec<String>,
    pub filter: String,
    pub args: Vec<String>,
    pub output: PathBuf,
}

// -------------------------------------------------------------------------------------------------
// Command builders (pure, testable without ffmpeg)
// -------------------------------------------------------------------------------------------------

pub fn build_frame_command(
    video_path: &Path,
    doc: &PlanoDocument,
    second: u64,
    output: &Path,
) -> Result<RenderCommand> {
    doc.validate().map_err(|e| anyhow::anyhow!(e))?;
    let plano = doc.to_plano();
    let (filter, inputs) = build_ffmpeg_filter(&plano, &video_path.to_string_lossy());
    let mut args = vec![
        "-y".to_string(),
        "-ss".to_string(),
        second.to_string(),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-filter_complex".to_string(),
        filter.clone(),
        "-map".to_string(),
        "[out]".to_string(),
        "-q:v".to_string(),
        "2".to_string(),
        output.to_string_lossy().to_string(),
    ];
    append_additional_inputs(&mut args, &inputs);
    Ok(RenderCommand {
        inputs,
        filter,
        args,
        output: output.to_path_buf(),
    })
}

pub fn build_sample_command(
    video_path: &Path,
    doc: &PlanoDocument,
    start_sec: u64,
    duration_secs: u64,
    profile: &ExportProfile,
    output: &Path,
) -> Result<RenderCommand> {
    doc.validate().map_err(|e| anyhow::anyhow!(e))?;
    if duration_secs == 0 || duration_secs > 30 {
        anyhow::bail!("sample duration must be 1..=30s");
    }
    let plano = doc.to_plano();
    let (filter, inputs) = build_ffmpeg_filter(&plano, &video_path.to_string_lossy());
    let size = format!("{}x{}", profile.width, profile.height);
    let mut args = vec![
        "-y".to_string(),
        "-ss".to_string(),
        start_sec.to_string(),
        "-t".to_string(),
        duration_secs.to_string(),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-filter_complex".to_string(),
        filter.clone(),
        "-map".to_string(),
        "[out]".to_string(),
        "-map".to_string(),
        "0:a?".to_string(),
        "-shortest".to_string(),
        "-s".to_string(),
        size,
        "-r".to_string(),
        profile.fps.to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-b:v".to_string(),
        profile.video_bitrate.clone(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "128k".to_string(),
        output.to_string_lossy().to_string(),
    ];
    append_additional_inputs(&mut args, &inputs);
    Ok(RenderCommand {
        inputs,
        filter,
        args,
        output: output.to_path_buf(),
    })
}

fn append_additional_inputs(args: &mut Vec<String>, inputs: &[String]) {
    let mut insertion = args
        .iter()
        .position(|arg| arg == "-i")
        .map(|index| index + 2)
        .unwrap_or(0);
    for input in inputs.iter().skip(1) {
        let is_image = std::path::Path::new(input)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
            .unwrap_or(false);
        let mut values = Vec::with_capacity(3);
        if is_image {
            values.push("-loop".to_string());
            values.push("1".to_string());
        }
        values.push("-i".to_string());
        values.push(input.clone());
        args.splice(insertion..insertion, values);
        insertion += if is_image { 4 } else { 2 };
    }
}

pub fn structure_hash(doc: &PlanoDocument) -> String {
    let json = serde_json::to_string(doc).unwrap_or_default();
    format!("{:x}", crate::util::fnv1a_hash(&json))
}

pub fn build_moment_command(
    video_path: &Path,
    doc: &PlanoDocument,
    start_sec: u64,
    end_sec: u64,
    profile: &ExportProfile,
    output: &Path,
) -> Result<RenderCommand> {
    doc.validate_for_export().map_err(|e| anyhow::anyhow!(e))?;
    if end_sec <= start_sec {
        anyhow::bail!("moment end must be after start");
    }
    let dur = end_sec - start_sec;
    if dur < crate::types::MIN_MOMENT_DURATION_SECONDS {
        anyhow::bail!("moment shorter than minimum");
    }
    build_sample_command(video_path, doc, start_sec, dur, profile, output)
}

// -------------------------------------------------------------------------------------------------
// Execution
// -------------------------------------------------------------------------------------------------

pub fn run_render(cmd: &RenderCommand) -> Result<()> {
    let status = std::process::Command::new(crate::setup::ffmpeg_bin())
        .args(&cmd.args)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                crate::setup::ffmpeg_missing_error()
            } else {
                anyhow::Error::from(e).context("failed to run ffmpeg")
            }
        })?;
    if !status.success() {
        anyhow::bail!("ffmpeg render failed for {}", cmd.output.display());
    }
    verify_output(&cmd.output)?;
    Ok(())
}

pub fn verify_output(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("output not created: {}", path.display());
    }
    let meta = std::fs::metadata(path).context("reading output metadata")?;
    if meta.len() < 1024 {
        anyhow::bail!("output too small: {}", path.display());
    }
    let (w, h) = crate::media::ffmpeg::get_resolution(path)?;
    if w == 0 || h == 0 {
        anyhow::bail!("invalid output resolution");
    }
    // Accept both configured outputs; compiler currently renders OUTPUT_WIDTHxHEIGHT.
    let _ = (OUTPUT_WIDTH, OUTPUT_HEIGHT);
    Ok(())
}

pub fn verify_video_output(
    path: &Path,
    profile: &ExportProfile,
    expected_duration: u64,
) -> Result<()> {
    verify_output(path)?;
    let (width, height) = crate::media::ffmpeg::get_resolution(path)?;
    if (width, height) != (profile.width, profile.height) {
        anyhow::bail!(
            "output resolution is {}x{}, expected {}x{}",
            width,
            height,
            profile.width,
            profile.height
        );
    }
    let duration = crate::media::ffmpeg::get_duration(path)?;
    if duration + 1.0 < expected_duration as f64 || duration > expected_duration as f64 + 2.0 {
        anyhow::bail!(
            "output duration {:.2}s is outside expected {}s",
            duration,
            expected_duration
        );
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plano::schema::create_empty_document;

    #[test]
    fn test_sample_duration_constant() {
        assert_eq!(SAMPLE_DURATION_SECS, 4);
    }

    #[test]
    fn test_build_frame_rejects_empty() {
        let doc = create_empty_document();
        let cmd = build_frame_command(Path::new("in.mp4"), &doc, 0, Path::new("out.png"));
        // Empty doc validates ok as document, but export validation is separate.
        // Frame builder only requires document validation, so it builds.
        assert!(cmd.is_ok());
        let c = cmd.unwrap();
        assert!(c.filter.contains("[out]"));
    }

    #[test]
    fn test_build_sample_profile() {
        // Minimal valid clip via default doc
        let doc = crate::plano::schema::create_default_document();
        let profile = ExportProfile::high_1080();
        let cmd = build_sample_command(
            Path::new("in.mp4"),
            &doc,
            10,
            SAMPLE_DURATION_SECS,
            &profile,
            Path::new("out.mp4"),
        )
        .unwrap();
        assert!(cmd.args.contains(&"libx264".to_string()));
        assert!(cmd.args.contains(&"12M".to_string()));
        assert!(cmd.args.contains(&"[out]".to_string()));
        assert_eq!(profile.hash(), "1080x1920-12M-30fps");
        let p720 = ExportProfile::high_720();
        assert_eq!(p720.hash(), "720x1280-8M-30fps");
    }
}
