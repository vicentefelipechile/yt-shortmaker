//! AutoShorts-Rust-CLI
//! A robust CLI tool to automate YouTube Shorts creation from long-form content
//! using Google Gemini AI for intelligent content analysis.

mod config;
mod dashboard;
mod gemini;
mod types;
mod video;

use anyhow::{Context, Result};
use config::AppConfig;
use console::style;
use dashboard::Dashboard;
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use gemini::GeminiClient;
use std::fs;
use std::path::Path;
use types::{SessionState, VideoMoment};

#[tokio::main]
async fn main() -> Result<()> {
    // Step 1: Load or create configuration
    let config = AppConfig::load_or_create().context("Failed to load configuration")?;

    // Step 2: Check dependencies
    if let Err(e) = video::check_dependencies() {
        eprintln!("\n❌ {}", e);
        eprintln!("\nPlease install the missing dependencies:");
        eprintln!("  • ffmpeg: https://ffmpeg.org/download.html");
        eprintln!("  • yt-dlp: https://github.com/yt-dlp/yt-dlp#installation");
        std::process::exit(1);
    }

    // Step 3: Initialize Dashboard
    let dashboard = Dashboard::init(&config.default_output_dir);

    // Ensure output directory exists
    config.ensure_output_dir()?;

    let temp_json_path = format!("{}/temp.json", config.default_output_dir);
    let mut session: Option<SessionState> = None;

    // Check for existing session
    if Path::new(&temp_json_path).exists() {
        let content = fs::read_to_string(&temp_json_path)?;
        if let Ok(existing_session) = serde_json::from_str::<SessionState>(&content) {
            println!();
            let resume = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Found previous session for: {}. Resume?",
                    style(&existing_session.youtube_url).cyan()
                ))
                .default(true)
                .interact()?;

            if resume {
                session = Some(existing_session);
            } else {
                // If not resuming, clean up the old temp dir mentioned in the session
                if Path::new(&existing_session.temp_dir).exists() {
                    fs::remove_dir_all(&existing_session.temp_dir).ok();
                }
                fs::remove_file(&temp_json_path).ok();
            }
        }
    }

    let (url, mut all_moments, temp_dir) = if let Some(s) = session {
        (s.youtube_url, s.moments, s.temp_dir)
    } else {
        // Create unique temp directory with timestamp
        let session_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let t_dir = format!("{}/temp_{}", config.default_output_dir, session_id);

        // Clean up any old temp directories first
        if let Ok(entries) = fs::read_dir(&config.default_output_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("temp_") && entry.path().is_dir() {
                        fs::remove_dir_all(entry.path()).ok();
                    }
                }
            }
        }

        fs::create_dir_all(&t_dir)?;

        // Step 4: Get YouTube URL from user
        dashboard.set_status("Waiting for URL input...");

        let u: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("\n🎬 Enter YouTube video URL")
            .validate_with(|input: &String| -> Result<(), &str> {
                if video::validate_youtube_url(input) {
                    Ok(())
                } else {
                    Err("Please enter a valid YouTube URL")
                }
            })
            .interact_text()?;

        (u, Vec::new(), t_dir)
    };

    // Ask for manual format selection
    println!();
    let use_custom_format = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Do you want to manually select the video format (yt-dlp)?")
        .default(false)
        .interact()?;

    let mut custom_format: Option<String> = None;
    if use_custom_format {
        dashboard.info("Listing available formats...");
        if let Err(e) = video::list_formats(&url, config.use_cookies, &config.cookies_path) {
            dashboard.warn(&format!("Failed to list formats: {}", e));
        } else {
            let format_input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter the format ID string (e.g., '137+140' or '22')")
                .interact_text()?;
            custom_format = Some(format_input);
        }
    }

    // Save initial state if new session
    save_session(&temp_json_path, &url, &all_moments, &temp_dir)?;

    let temp_low_res = format!("{}/low_res.mp4", temp_dir);

    // Skip download if resuming and file exists
    if !Path::new(&temp_low_res).exists() {
        dashboard.set_status("Downloading Low-Res video for analysis...");
        if let Err(e) = video::download_low_res(
            &url,
            &temp_low_res,
            config.use_cookies,
            &config.cookies_path,
        )
        .await
        {
            dashboard.error(&format!("Download failed: {}", e));
            return Err(e);
        }
        dashboard.info("Low-res video downloaded successfully");
    }

    // Get video duration
    let duration = video::get_video_duration(&temp_low_res)?;

    // Step 6: Split into chunks
    let temp_chunks_dir = format!("{}/chunks", temp_dir);
    let video_chunks = if Path::new(&temp_chunks_dir).exists()
        && fs::read_dir(&temp_chunks_dir)?.next().is_some()
    {
        // Guessing chunks if they already exist might be complex, let's just re-calculate
        // and check if files exist to be safe, but typically split_video is fast
        dashboard.info("Using existing video chunks");
        let chunks = video::calculate_chunks(duration);
        let mut v_chunks = Vec::new();
        for (i, (start, _)) in chunks.iter().enumerate() {
            let chunk_path = format!("{}/chunk_{}.mp4", temp_chunks_dir, i);
            if Path::new(&chunk_path).exists() {
                v_chunks.push(types::VideoChunk {
                    start_seconds: *start,
                    file_path: chunk_path,
                });
            }
        }
        if v_chunks.is_empty() {
            video::split_video(
                &temp_low_res,
                &temp_chunks_dir,
                &video::calculate_chunks(duration),
            )
            .await?
        } else {
            v_chunks
        }
    } else {
        dashboard.set_status("Splitting video into chunks...");
        let chunks = video::calculate_chunks(duration);
        video::split_video(&temp_low_res, &temp_chunks_dir, &chunks).await?
    };

    // Step 7: Analyze each chunk with Gemini
    if all_moments.is_empty() {
        dashboard.set_status("Analyzing video with Gemini AI...");
        let gemini = GeminiClient::new(config.google_api_keys.clone());

        for (i, chunk) in video_chunks.iter().enumerate() {
            dashboard.set_status(&format!(
                "Analyzing chunk {}/{} with Gemini AI...",
                i + 1,
                video_chunks.len()
            ));

            match gemini.upload_video(&chunk.file_path).await {
                Ok(file_uri) => {
                    match gemini.analyze_video(&file_uri, chunk.start_seconds).await {
                        Ok(moments) => {
                            dashboard.info(&format!(
                                "Chunk {}: Found {} moments",
                                i + 1,
                                moments.len()
                            ));
                            all_moments.extend(moments);
                            // Save state after each chunk
                            save_session(&temp_json_path, &url, &all_moments, &temp_dir)?;
                        }
                        Err(e) => {
                            dashboard.warn(&format!("Chunk {} analysis failed: {}", i + 1, e));
                        }
                    }
                }
                Err(e) => {
                    dashboard.warn(&format!("Chunk {} upload failed: {}", i + 1, e));
                }
            }
        }
    }

    // Save final moments to temp.json (as requested, replacing moments.json)
    save_session(&temp_json_path, &url, &all_moments, &temp_dir)?;

    dashboard.info(&format!("Found {} total moments", all_moments.len()));
    dashboard.info(&format!("State saved to: {}", temp_json_path));

    // Also create a human-readable text file
    let moments_txt = format!("{}/moments.txt", config.default_output_dir);
    let mut txt_content = String::new();
    txt_content.push_str("=== YouTube Shorts Moments ===\n\n");
    for (i, moment) in all_moments.iter().enumerate() {
        txt_content.push_str(&format!(
            "{}. [{} - {}] ({})\n   {}\n\n",
            i + 1,
            moment.start_time,
            moment.end_time,
            moment.category,
            moment.description
        ));
    }
    fs::write(&moments_txt, &txt_content)?;

    if all_moments.is_empty() {
        dashboard.warn("No suitable moments found for shorts");
        cleanup_temp_dir(&temp_dir)?;
        fs::remove_file(&temp_json_path).ok();
        dashboard.success("Process completed (no moments to extract)");
        return Ok(());
    }

    // Step 9: Ask if user wants to generate shorts (respecting auto-extract setting but still asking)
    println!();
    let generate_shorts = if config.extract_shorts_when_finished_moments {
        println!("🚀 Auto-extraction enabled in settings.");
        true
    } else {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Found {} moments. Generate YouTube Shorts from source video?",
                all_moments.len()
            ))
            .default(true)
            .interact()?
    };

    if !generate_shorts {
        cleanup_temp_dir(&temp_dir)?;
        dashboard.success("Moments saved in temp.json. Exiting without generating shorts.");
        return Ok(());
    }

    // Step 10: Download High-Res video
    dashboard.set_status("Downloading High-Res video for extraction...");
    let source_high_res = format!("{}/high_res.mp4", temp_dir);

    if !Path::new(&source_high_res).exists() {
        if let Err(e) = video::download_high_res(
            &url,
            &source_high_res,
            config.use_cookies,
            &config.cookies_path,
            custom_format,
        )
        .await
        {
            dashboard.error(&format!("High-res download failed: {}", e));
            return Err(e);
        }
        dashboard.info("High-res video downloaded successfully");
    }

    // Step 11: Extract clips
    dashboard.set_status("Extracting YouTube Shorts clips...");
    let shorts_session = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let shorts_dir = format!("{}/shorts_{}", config.default_output_dir, shorts_session);
    fs::create_dir_all(&shorts_dir)?;

    let total_clips = all_moments.len();
    for (i, moment) in all_moments.iter().enumerate() {
        dashboard.set_status(&format!("Extracting clip {}/{}...", i + 1, total_clips));
        let output_path = format!(
            "{}/short_{}_{}.mp4",
            shorts_dir,
            i + 1,
            moment.category.replace(' ', "_").to_lowercase()
        );

        if let Err(e) = video::extract_clip(
            &source_high_res,
            &moment.start_time,
            &moment.end_time,
            &output_path,
        )
        .await
        {
            dashboard.warn(&format!("Failed to extract clip {}: {}", i + 1, e));
        } else {
            dashboard.info(&format!("Created: {}", output_path));
        }
    }

    // Step 12: Cleanup and finish
    cleanup_temp_dir(&temp_dir)?;
    fs::remove_file(&temp_json_path).ok();

    println!();
    println!(
        "{}",
        style("============================================")
            .cyan()
            .bold()
    );
    println!(
        "   {} shorts saved to: {}",
        style(total_clips).green().bold(),
        style(&shorts_dir).yellow()
    );
    println!(
        "{}",
        style("============================================")
            .cyan()
            .bold()
    );

    dashboard.success(&format!(
        "Done! {} shorts saved to {}",
        total_clips, shorts_dir
    ));

    Ok(())
}

/// Save session state for resumption
fn save_session(path: &str, url: &str, moments: &[VideoMoment], temp_dir: &str) -> Result<()> {
    let state = SessionState {
        youtube_url: url.to_string(),
        moments: moments.to_vec(),
        temp_dir: temp_dir.to_string(),
    };
    let json = serde_json::to_string_pretty(&state)?;
    fs::write(path, json)?;
    Ok(())
}

/// Clean up temporary directory
fn cleanup_temp_dir(temp_dir: &str) -> Result<()> {
    if Path::new(temp_dir).exists() {
        fs::remove_dir_all(temp_dir).ok();
    }
    Ok(())
}
