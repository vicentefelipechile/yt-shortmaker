//! AutoShorts-Rust-CLI
//! A robust TUI tool to automate YouTube Shorts creation from long-form content
//! using Google Gemini AI for intelligent content analysis.

mod config;
mod gemini;
mod shorts;
mod tui;
mod types;
mod video;

use anyhow::{Context, Result};
use config::AppConfig;
use crossterm::event::{self, Event, KeyEventKind};
use gemini::GeminiClient;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tui::{App, AppMessage, AppScreen, LogLevel, TuiSender};
use types::{SessionState, VideoMoment};

#[tokio::main]
async fn main() -> Result<()> {
    // Check for CLI commands first
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        return handle_cli_command(&args).await;
    }

    // No CLI commands, run TUI mode
    run_tui_mode().await
}

/// Handle CLI commands (preview, transform)
async fn handle_cli_command(args: &[String]) -> Result<()> {
    let command = args[1].as_str();

    // Load config (minimal validation for CLI commands)
    let config = load_config_for_cli()?;

    match command {
        "preview" => {
            if args.len() < 3 {
                eprintln!(
                    "Usage: {} preview <video_path> [timestamp_seconds]",
                    args[0]
                );
                eprintln!("\nExample:");
                eprintln!("  {} preview video.mp4", args[0]);
                eprintln!(
                    "  {} preview video.mp4 5.5  # Preview at 5.5 seconds",
                    args[0]
                );
                std::process::exit(1);
            }

            let video_path = &args[2];
            let timestamp: f64 = args.get(3).map(|s| s.parse().unwrap_or(0.0)).unwrap_or(0.0);

            let output_image = format!("{}_preview.png", video_path.trim_end_matches(".mp4"));

            println!("🎬 Generating preview...");
            println!("   Input: {}", video_path);
            println!("   Timestamp: {:.2}s", timestamp);
            println!(
                "   Config: {} overlays",
                config.shorts_config.overlays.len()
            );

            shorts::generate_preview(
                video_path,
                &output_image,
                &config.shorts_config,
                timestamp,
                config.gpu_acceleration.unwrap_or(false),
            )?;

            println!("✅ Preview saved to: {}", output_image);
            Ok(())
        }

        "transform" => {
            if args.len() < 3 {
                eprintln!("Usage: {} transform <video_path> [output_path]", args[0]);
                eprintln!("\nExample:");
                eprintln!("  {} transform video.mp4", args[0]);
                eprintln!("  {} transform video.mp4 output_short.mp4", args[0]);
                std::process::exit(1);
            }

            let video_path = &args[2];
            let output_path = args
                .get(3)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}_short.mp4", video_path.trim_end_matches(".mp4")));

            println!("🎬 Transforming to YouTube Short...");
            println!("   Input: {}", video_path);
            println!("   Output: {}", output_path);
            println!(
                "   Resolution: {}x{}",
                config.shorts_config.output_width, config.shorts_config.output_height
            );
            println!(
                "   Background: {}",
                config
                    .shorts_config
                    .background_video
                    .as_ref()
                    .unwrap_or(&"None".to_string())
            );
            println!("   Overlays: {}", config.shorts_config.overlays.len());

            shorts::transform_to_short(
                video_path,
                &output_path,
                &config.shorts_config,
                config.gpu_acceleration.unwrap_or(false),
            )
            .await?;

            println!("✅ Short saved to: {}", output_path);
            Ok(())
        }

        "batch" => {
            if args.len() < 3 {
                eprintln!("Usage: {} batch <input_dir> [output_dir]", args[0]);
                eprintln!("\nExample:");
                eprintln!("  {} batch ./clips", args[0]);
                eprintln!("  {} batch ./clips ./shorts", args[0]);
                std::process::exit(1);
            }

            let input_dir = &args[2];
            let output_dir = args
                .get(3)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}_shorts", input_dir));

            println!("🎬 Batch transforming videos...");
            println!("   Input dir: {}", input_dir);
            println!("   Output dir: {}", output_dir);

            let results = shorts::transform_batch(
                input_dir,
                &output_dir,
                &config.shorts_config,
                config.gpu_acceleration.unwrap_or(false),
                Some(Box::new(|current, total, name| {
                    println!("   [{}/{}] Processing: {}", current, total, name);
                })),
            )
            .await?;

            println!("✅ Transformed {} videos to: {}", results.len(), output_dir);
            Ok(())
        }

        "help" | "--help" | "-h" => {
            print_help(&args[0]);
            Ok(())
        }

        _ => {
            eprintln!("Unknown command: {}", command);
            print_help(&args[0]);
            std::process::exit(1);
        }
    }
}

/// Print help message
fn print_help(program: &str) {
    println!("AutoShorts-Rust-CLI v{}", types::APP_VERSION);
    println!();
    println!("USAGE:");
    println!(
        "  {}                           Run TUI mode (interactive)",
        program
    );
    println!(
        "  {} preview <video> [time]    Generate preview image",
        program
    );
    println!(
        "  {} transform <video> [out]   Transform single video to short",
        program
    );
    println!(
        "  {} batch <dir> [out_dir]     Batch transform all videos in directory",
        program
    );
    println!(
        "  {} help                      Show this help message",
        program
    );
    println!();
    println!("EXAMPLES:");
    println!(
        "  {} preview clip.mp4 2.5      Preview at 2.5 seconds",
        program
    );
    println!(
        "  {} transform clip.mp4        Creates clip_short.mp4",
        program
    );
    println!(
        "  {} batch ./clips ./shorts    Transform all clips to shorts",
        program
    );
    println!();
    println!("CONFIGURATION:");
    println!("  Edit settings.json to configure:");
    println!("  - shorts_config.background_video   Background video path (looped)");
    println!("  - shorts_config.background_opacity Opacity (0.0-1.0, default 0.4)");
    println!("  - shorts_config.overlays           Array of image overlays with x,y positions");
}

/// Load config for CLI commands (doesn't require API keys)
fn load_config_for_cli() -> Result<AppConfig> {
    if !Path::new("settings.json").exists() {
        // Create default config
        AppConfig::create_default()?;
        println!("📝 Created default settings.json");
    }

    let content = fs::read_to_string("settings.json")?;
    let config: AppConfig =
        serde_json::from_str(&content).context("Failed to parse settings.json")?;

    Ok(config)
}

/// Run the TUI mode
async fn run_tui_mode() -> Result<()> {
    // Setup terminal
    let mut terminal = tui::setup_terminal()?;

    // Run the app
    let result = run_app(&mut terminal).await;

    // Restore terminal
    tui::restore_terminal(&mut terminal)?;

    // Handle any errors
    if let Err(ref e) = result {
        eprintln!("\n❌ Error: {}", e);
    }

    result
}

async fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    // Step 1: Load or create configuration
    let config = load_config_with_fallback()?;

    // Step 2: Check dependencies
    if let Err(e) = video::check_dependencies() {
        // Show error in simple terminal mode since TUI isn't fully up yet
        tui::restore_terminal(terminal)?;
        eprintln!("\n❌ {}", e);
        eprintln!("\nPlease install the missing dependencies:");
        eprintln!("  • ffmpeg: https://ffmpeg.org/download.html");
        eprintln!("  • yt-dlp: https://github.com/yt-dlp/yt-dlp#installation");
        std::process::exit(1);
    }

    // Initialize app state
    let mut app = App::new(config.default_output_dir.clone());
    app.config = Some(config.clone());
    app.status = "Ready".to_string();

    // Check for NVENC availability if not configured
    // If detected, we start at the GpuDetectionPrompt screen instead of UrlInput/Resume
    let mut nvenc_detected = false;
    if config.gpu_acceleration.is_none() {
        if video::check_nvenc_availability() {
            nvenc_detected = true;
        } else {
            // GPU not detected, set to false to avoid future checks
            let mut new_config = config.clone();
            new_config.gpu_acceleration = Some(false);
            new_config.save().ok();
        }
    }

    // Create message channel for async communication
    let (tx, mut rx) = tui::create_channel();

    // Check for existing session
    let temp_json_path = format!("{}/temp.json", config.default_output_dir);
    let mut session: Option<SessionState> = None;
    let mut resume_prompted = false;

    if nvenc_detected {
        app.screen = AppScreen::GpuDetectionPrompt;
    } else {
        if Path::new(&temp_json_path).exists() {
            if let Ok(content) = fs::read_to_string(&temp_json_path) {
                if let Ok(existing_session) = serde_json::from_str::<SessionState>(&content) {
                    app.screen = AppScreen::ResumePrompt(existing_session.youtube_url.clone());
                    session = Some(existing_session);
                    resume_prompted = true;
                }
            }
        }

        if !resume_prompted {
            app.screen = AppScreen::UrlInput;
        }
    }

    // Main event loop
    let mut url = String::new();
    let mut all_moments: Vec<VideoMoment> = Vec::new();
    let mut temp_dir = String::new();
    let mut custom_format: Option<String> = None;
    let mut processing_started = false;

    loop {
        // Render UI
        terminal.draw(|frame| tui::render(frame, &app))?;

        // Handle messages from background tasks
        while let Ok(msg) = rx.try_recv() {
            app.handle_message(msg);
        }

        // Poll for events with timeout
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
                }
            }
        }

        // Check for quit
        if app.should_quit {
            break;
        }

        // Handle screen transitions
        match &app.screen {
            AppScreen::GpuDetectionPrompt => {
                if let Some(response) = app.confirm_response.take() {
                    // Update config
                    let use_gpu = response;

                    // We need to update both the local config variable (for this run)
                    // and the saved file (for future runs).
                    // However, 'config' is immutable here.
                    // But app.config is mutable.
                    if let Some(ref mut c) = app.config {
                        c.gpu_acceleration = Some(use_gpu);
                        if let Err(e) = c.save() {
                            app.log(LogLevel::Error, format!("Failed to save settings: {}", e));
                        } else {
                            app.log(LogLevel::Success, "Settings saved!".to_string());
                        }
                    }

                    // Now proceed to Resume check or Url Input
                    // We duplicate the logic from above, but 'nvenc_detected' is logically handled now.
                    // Actually, we can check for session here.
                    // IMPORTANT: We need to know if we should go to ResumePrompt or UrlInput.
                    // Re-check for session file.
                    if Path::new(&temp_json_path).exists() {
                        if let Ok(content) = fs::read_to_string(&temp_json_path) {
                            if let Ok(existing_session) =
                                serde_json::from_str::<SessionState>(&content)
                            {
                                app.screen =
                                    AppScreen::ResumePrompt(existing_session.youtube_url.clone());
                                session = Some(existing_session);
                                resume_prompted = true; // Just local var, doesn't matter much here
                            } else {
                                app.screen = AppScreen::UrlInput;
                            }
                        } else {
                            app.screen = AppScreen::UrlInput;
                        }
                    } else {
                        app.screen = AppScreen::UrlInput;
                    }
                }
            }
            AppScreen::ResumePrompt(_) => {
                if let Some(response) = app.confirm_response.take() {
                    if response {
                        // Resume session
                        if let Some(s) = session.take() {
                            url = s.youtube_url;
                            all_moments = s.moments;
                            temp_dir = s.temp_dir;
                            app.moments = all_moments.clone();
                            app.log(LogLevel::Info, format!("Resuming session for: {}", url));
                            app.screen = AppScreen::FormatConfirm;
                        }
                    } else {
                        // Clean up old session
                        if let Some(ref s) = session {
                            if Path::new(&s.temp_dir).exists() {
                                fs::remove_dir_all(&s.temp_dir).ok();
                            }
                        }
                        fs::remove_file(&temp_json_path).ok();
                        session = None;
                        app.screen = AppScreen::UrlInput;
                    }
                }
            }
            AppScreen::UrlInput => {
                if let Some(_) = app.confirm_response.take() {
                    let input_url = app.input.trim().to_string();
                    if video::validate_youtube_url(&input_url) {
                        url = input_url;
                        app.log(LogLevel::Success, format!("Valid URL: {}", url));

                        // Create new session
                        let session_id = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        temp_dir = format!("{}/temp_{}", config.default_output_dir, session_id);

                        // Clean up old temp directories
                        if let Ok(entries) = fs::read_dir(&config.default_output_dir) {
                            for entry in entries.flatten() {
                                if let Some(name) = entry.file_name().to_str() {
                                    if name.starts_with("temp_") && entry.path().is_dir() {
                                        fs::remove_dir_all(entry.path()).ok();
                                    }
                                }
                            }
                        }

                        fs::create_dir_all(&temp_dir)?;
                        all_moments.clear();
                        app.screen = AppScreen::FormatConfirm;
                    } else {
                        app.log(LogLevel::Error, "Invalid YouTube URL".to_string());
                        app.input.clear();
                        app.cursor_pos = 0;
                    }
                }
            }
            AppScreen::FormatConfirm => {
                if let Some(response) = app.confirm_response.take() {
                    if response {
                        // User wants to select format - for simplicity, skip this in TUI
                        // A full implementation would show format list
                        app.log(
                            LogLevel::Info,
                            "Using default format (custom format selection not available in TUI)"
                                .to_string(),
                        );
                    }
                    custom_format = None;
                    app.screen = AppScreen::Processing;
                    processing_started = false;
                }
            }
            AppScreen::Processing => {
                if !processing_started {
                    processing_started = true;

                    // Start background processing
                    let tx_clone = tx.clone();
                    let config_clone = config.clone();
                    let url_clone = url.clone();
                    let temp_dir_clone = temp_dir.clone();
                    let temp_json_path_clone = temp_json_path.clone();
                    let custom_format_clone = custom_format.clone();
                    let existing_moments = all_moments.clone();

                    tokio::spawn(async move {
                        let result = run_processing(
                            tx_clone.clone(),
                            config_clone,
                            url_clone,
                            temp_dir_clone,
                            temp_json_path_clone,
                            custom_format_clone,
                            existing_moments,
                        )
                        .await;

                        match result {
                            Ok((moments, shorts_dir)) => {
                                for m in &moments {
                                    let _ = tx_clone.send(AppMessage::MomentFound(m.clone()));
                                }
                                if let Some(dir) = shorts_dir {
                                    let _ = tx_clone.send(AppMessage::Complete(format!(
                                        "Shorts saved to: {}",
                                        dir
                                    )));
                                }
                                let _ = tx_clone.send(AppMessage::Finished);
                            }
                            Err(e) => {
                                let _ = tx_clone.send(AppMessage::Error(format!("Error: {}", e)));
                                let _ = tx_clone.send(AppMessage::Finished);
                            }
                        }
                    });
                }
            }
            AppScreen::ShortsConfirm(count) => {
                if let Some(response) = app.confirm_response.take() {
                    if response {
                        app.log(LogLevel::Info, "Generating shorts...".to_string());
                    } else {
                        app.log(LogLevel::Info, "Skipping shorts generation".to_string());
                    }
                    app.screen = AppScreen::Done;
                }
            }
            AppScreen::Done => {
                // Already handled by key press
            }
            _ => {}
        }
    }

    Ok(())
}

/// Load configuration with fallback for missing file
fn load_config_with_fallback() -> Result<AppConfig> {
    // Try to load existing config
    if Path::new("settings.json").exists() {
        let content = fs::read_to_string("settings.json")?;
        let config: AppConfig = serde_json::from_str(&content).map_err(|_| {
            anyhow::anyhow!(
                "Configuration file format has changed. Please delete settings.json and restart."
            )
        })?;

        if config.google_api_keys.is_empty() {
            return Err(anyhow::anyhow!(
                "No API keys found in configuration. Please delete settings.json and restart."
            ));
        }

        return Ok(config);
    }

    // Create default config automatically
    println!("📝 Configuration file not found. Creating default settings.json...");
    AppConfig::create_default()?;

    // Load the newly created config
    let config = AppConfig::load()?;
    Ok(config)
}

/// Run the main processing pipeline
async fn run_processing(
    tx: TuiSender,
    config: AppConfig,
    url: String,
    temp_dir: String,
    temp_json_path: String,
    custom_format: Option<String>,
    mut all_moments: Vec<VideoMoment>,
) -> Result<(Vec<VideoMoment>, Option<String>)> {
    // Ensure output directory exists
    config.ensure_output_dir()?;

    // Save initial state
    save_session(&temp_json_path, &url, &all_moments, &temp_dir)?;

    let temp_low_res = format!("{}/low_res.mp4", temp_dir);

    // Download low-res if needed
    if !Path::new(&temp_low_res).exists() {
        let _ = tx.send(AppMessage::Status(
            "Downloading Low-Res video...".to_string(),
        ));
        let _ = tx.send(AppMessage::Progress(0.1, "Downloading...".to_string()));

        video::download_low_res(
            &url,
            &temp_low_res,
            config.use_cookies,
            &config.cookies_path,
        )
        .await
        .context("Failed to download low-res video")?;

        let _ = tx.send(AppMessage::Log(
            LogLevel::Success,
            "Low-res video downloaded".to_string(),
        ));
    } else {
        let _ = tx.send(AppMessage::Log(
            LogLevel::Info,
            "Using cached low-res video".to_string(),
        ));
    }

    // Get video duration
    let duration = video::get_video_duration(&temp_low_res)?;
    let _ = tx.send(AppMessage::Log(
        LogLevel::Info,
        format!("Video duration: {} seconds", duration),
    ));

    // Split into chunks
    let temp_chunks_dir = format!("{}/chunks", temp_dir);
    let _ = tx.send(AppMessage::Status(
        "Splitting video into chunks...".to_string(),
    ));
    let _ = tx.send(AppMessage::Progress(0.2, "Splitting...".to_string()));

    let video_chunks = if Path::new(&temp_chunks_dir).exists()
        && fs::read_dir(&temp_chunks_dir)?.next().is_some()
    {
        let _ = tx.send(AppMessage::Log(
            LogLevel::Info,
            "Using existing video chunks".to_string(),
        ));
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
        let chunks = video::calculate_chunks(duration);
        video::split_video(&temp_low_res, &temp_chunks_dir, &chunks).await?
    };

    let _ = tx.send(AppMessage::Log(
        LogLevel::Success,
        format!("Created {} chunks", video_chunks.len()),
    ));

    // Analyze chunks with Gemini
    if all_moments.is_empty() {
        let _ = tx.send(AppMessage::Status(
            "Analyzing with Gemini AI...".to_string(),
        ));
        let gemini = GeminiClient::new(config.google_api_keys.clone());

        for (i, chunk) in video_chunks.iter().enumerate() {
            let progress = 0.3 + (0.5 * (i as f64 / video_chunks.len() as f64));
            let _ = tx.send(AppMessage::Progress(
                progress,
                format!("Analyzing chunk {}/{}", i + 1, video_chunks.len()),
            ));
            let _ = tx.send(AppMessage::Status(format!(
                "Analyzing chunk {}/{}...",
                i + 1,
                video_chunks.len()
            )));

            match gemini.upload_video(&chunk.file_path).await {
                Ok(file_uri) => match gemini.analyze_video(&file_uri, chunk.start_seconds).await {
                    Ok(moments) => {
                        let _ = tx.send(AppMessage::Log(
                            LogLevel::Info,
                            format!("Chunk {}: Found {} moments", i + 1, moments.len()),
                        ));
                        for m in &moments {
                            let _ = tx.send(AppMessage::MomentFound(m.clone()));
                        }
                        all_moments.extend(moments);
                        save_session(&temp_json_path, &url, &all_moments, &temp_dir)?;
                    }
                    Err(e) => {
                        let _ = tx.send(AppMessage::Log(
                            LogLevel::Warning,
                            format!("Chunk {} analysis failed: {}", i + 1, e),
                        ));
                    }
                },
                Err(e) => {
                    let _ = tx.send(AppMessage::Log(
                        LogLevel::Warning,
                        format!("Chunk {} upload failed: {}", i + 1, e),
                    ));
                }
            }
        }
    }

    // Save final moments
    save_session(&temp_json_path, &url, &all_moments, &temp_dir)?;
    let _ = tx.send(AppMessage::Log(
        LogLevel::Success,
        format!("Found {} total moments", all_moments.len()),
    ));

    // Also create human-readable text file
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
        let _ = tx.send(AppMessage::Log(
            LogLevel::Warning,
            "No suitable moments found".to_string(),
        ));
        cleanup_temp_dir(&temp_dir)?;
        fs::remove_file(&temp_json_path).ok();
        return Ok((all_moments, None));
    }

    // Auto-extract if enabled, otherwise we would ask
    let generate_shorts = config.extract_shorts_when_finished_moments;

    if !generate_shorts {
        let _ = tx.send(AppMessage::Log(
            LogLevel::Info,
            "Moments saved. Skipping extraction (auto-extract disabled)".to_string(),
        ));
        cleanup_temp_dir(&temp_dir)?;
        fs::remove_file(&temp_json_path).ok();
        return Ok((all_moments, None));
    }

    // Download high-res
    let _ = tx.send(AppMessage::Status(
        "Downloading High-Res video...".to_string(),
    ));
    let _ = tx.send(AppMessage::Progress(
        0.85,
        "High-res download...".to_string(),
    ));

    let source_high_res = format!("{}/high_res.mp4", temp_dir);
    if !Path::new(&source_high_res).exists() {
        video::download_high_res(
            &url,
            &source_high_res,
            config.use_cookies,
            &config.cookies_path,
            custom_format,
        )
        .await
        .context("Failed to download high-res video")?;

        let _ = tx.send(AppMessage::Log(
            LogLevel::Success,
            "High-res video downloaded".to_string(),
        ));
    }

    // Extract clips
    let _ = tx.send(AppMessage::Status("Extracting clips...".to_string()));
    let shorts_session = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let shorts_dir = format!("{}/shorts_{}", config.default_output_dir, shorts_session);
    fs::create_dir_all(&shorts_dir)?;

    let total_clips = all_moments.len();
    for (i, moment) in all_moments.iter().enumerate() {
        let progress = 0.9 + (0.1 * (i as f64 / total_clips as f64));
        let _ = tx.send(AppMessage::Progress(
            progress,
            format!("Extracting {}/{}", i + 1, total_clips),
        ));

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
            config.gpu_acceleration.unwrap_or(false),
        )
        .await
        {
            let _ = tx.send(AppMessage::Log(
                LogLevel::Warning,
                format!("Failed to extract clip {}: {}", i + 1, e),
            ));
        } else {
            let _ = tx.send(AppMessage::Log(
                LogLevel::Success,
                format!("Created: short_{}.mp4", i + 1),
            ));
        }
    }

    // Cleanup
    cleanup_temp_dir(&temp_dir)?;
    fs::remove_file(&temp_json_path).ok();

    let _ = tx.send(AppMessage::Progress(1.0, "Complete!".to_string()));
    let _ = tx.send(AppMessage::Complete(format!(
        "{} shorts saved to: {}",
        total_clips, shorts_dir
    )));

    Ok((all_moments, Some(shorts_dir)))
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
