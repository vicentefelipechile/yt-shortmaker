//! YT ShortMaker
//! A robust TUI tool to automate YouTube Shorts creation from long-form content
//! using Google Gemini AI for intelligent content analysis.

mod config;

mod ai;
mod security;
mod setup;
mod shorts;
mod tui;
mod types;
mod video;

use ai::{AiClient, GoogleClient, OpenRouterClient};
use anyhow::{Context, Result};
use config::AppConfig;
use crossterm::event::{self, Event, KeyEventKind};
use security::EncryptionMode;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use tui::{App, AppMessage, AppScreen, LogLevel, TuiSender};

// Init translations
rust_i18n::i18n!("locales");

use types::{SessionState, VideoMoment};
use video::extract_video_id;

use crate::types::APP_VERSION;

#[tokio::main]
async fn main() -> Result<()> {
    // No CLI commands, run TUI mode
    println!("Abriendo, por favor espera... 🥺 ({})", APP_VERSION);

    // Add local bin to PATH immediately
    setup::add_to_process_path(&setup::get_bin_dir());

    // Check for CLI commands first
    let args: Vec<String> = std::env::args().collect();

    // Check and strip --debug flag
    let debug_mode = args.contains(&"--debug".to_string());
    if debug_mode {
        let _ = WriteLogger::init(
            LevelFilter::Debug,
            Config::default(),
            OpenOptions::new()
                .create(true)
                .append(true)
                .open("debug.log")?,
        );
        log::info!("Starting YT ShortMaker with debug logging");
        log::debug!("Raw Args: {:?}", args);
    }

    let actual_args: Vec<String> = args.iter().filter(|a| *a != "--debug").cloned().collect();

    if actual_args.len() > 1 {
        return handle_cli_command(&actual_args).await;
    }

    // Run setup wizard first
    setup::run_setup_wizard().await?;

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
    println!("YT ShortMaker v{}", types::APP_VERSION);
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
    // Step 1: Load or create configuration
    // We do this BEFORE setting up the terminal so that any println! calls
    // (like creating default config) happen on the normal stdout, not messing up the TUI.
    let config = load_config_with_fallback()?;

    // Set locale
    rust_i18n::set_locale(&config.language);

    // Setup terminal
    let mut terminal = tui::setup_terminal()?;

    // Run the app
    let result = run_app(&mut terminal, config).await;

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
    config: AppConfig,
) -> Result<()> {
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
    app.active_security_mode = config.active_encryption_mode;
    app.active_password = config.active_password.clone();
    app.config = Some(config.clone());
    app.status = "Ready".to_string();

    // Start at Main Menu by default
    app.screen = AppScreen::MainMenu;

    let default_key = "YOUR_API_KEY_HERE";

    // Check for Security FIRST
    if config.active_encryption_mode == EncryptionMode::Password && config.active_password.is_none()
    {
        app.screen = AppScreen::PasswordInput;
        app.security_error = Some("Startup: Password Required".to_string());
    } else {
        // Check for API Keys - if default or empty, go to ApiKeyInput
        // We only check if Google is the provider or if generic check is needed?
        // Let's stick to the previous logic but allow if keys exist.
        if config.google_api_keys.is_empty()
            || config
                .google_api_keys
                .iter()
                .any(|k| k.value == default_key)
            || config
                .google_api_keys
                .iter()
                .any(|k| k.value.trim().is_empty())
        {
            // If Google keys are bad, do we check OpenRouter?
            // Since default provider is Google, we probably want to enforce this or update TUI logic later to choose.
            // For now, let's allow bypassing if OpenRouter keys are present?
            // Or just stick to original logic: Force config setup if keys are missing.
            // But user might want to use OpenRouter.
            // Let's relax this: if Active Provider keys are missing, then prompt?
            // But here we don't know if user wants to change provider in settings.
            // Let's just go to ApiKeyInput if NO keys are good, or if active provider keys are missing.
            // Simplification: Check Google keys only for now as default,
            // or check if both are empty.
            if config.google_api_keys.is_empty() && config.openrouter_api_keys.is_empty() {
                app.screen = AppScreen::ApiKeyInput;
                app.input.clear();
                app.cursor_pos = 0;
            }
        }
    }

    // Create message channel for async communication
    let (tx, mut rx) = tui::create_channel();

    // Session and Temp paths
    let mut session: Option<SessionState> = None;

    // Main event loop
    let mut url = String::new();
    let mut all_moments: Vec<VideoMoment> = Vec::new();
    let mut temp_dir = String::new();
    let mut custom_format: Option<String> = None;
    let mut processing_started = false;
    let mut previous_screen = app.screen.clone();

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

        // Handle transitions
        if previous_screen == AppScreen::ApiKeyInput && app.confirm_response.is_some() {
            if let Some(true) = app.confirm_response.take() {
                let new_key = app.input.trim().to_string();
                if !new_key.is_empty() && new_key != default_key {
                    if let Some(ref mut c) = app.config {
                        c.google_api_keys = vec![config::ApiKey {
                            value: new_key,
                            name: "Primary Key".to_string(),
                            enabled: true,
                        }];
                        if let Err(e) = c.save() {
                            app.log(LogLevel::Error, format!("Failed to save API key: {}", e));
                        } else {
                            app.log(LogLevel::Success, "API Key saved successfully!".to_string());
                            app.screen = AppScreen::MainMenu;
                            app.input.clear();
                            app.cursor_pos = 0;
                        }
                    }
                } else {
                    app.log(LogLevel::Error, "Invalid API Key".to_string());
                    // Reset input provided confirmation triggers
                    app.confirm_response = None;
                }
            }
        }

        // Sync local config with app config (in case it was edited)
        if let Some(ref c) = app.config {
            // Ideally we'd only do this if changed, but cloning config is cheap enough
            // We need to keep our local `config` var updated for the processing step
            if c.default_output_dir != config.default_output_dir
                || c.gpu_acceleration != config.gpu_acceleration
                || c.shorts_config.output_width != config.shorts_config.output_width
            {
                // Just update the whole thing to be safe
                // We can't assign to `config` because it's immutable binding from earlier.
                // But we can create a new binding if we shadow it?
                // No, we need it for `tokio::spawn` later.
                // We will shadow it inside the Processing block or use `app.config` directly there.
            }
        }

        // Handle logical transitions
        // Detect "Start" from MainMenu -> UrlInput
        if previous_screen == AppScreen::MainMenu && app.screen == AppScreen::UrlInput {
            // Perform Startup Checks (GPU, Resume)
            let current_config = app.config.clone().unwrap_or(config.clone());

            // Check NVENC if not configured
            let mut nvenc_needed = false;
            if current_config.gpu_acceleration.is_none() {
                if video::check_nvenc_availability() {
                    nvenc_needed = true;
                } else {
                    // Auto-disable if not available
                    if let Some(ref mut c) = app.config {
                        c.gpu_acceleration = Some(false);
                        c.save().ok();
                    }
                }
            }

            if nvenc_needed {
                app.screen = AppScreen::GpuDetectionPrompt;
            } else {
                // Check persistence
                let p_temp_path = format!("{}/temp.json", current_config.default_output_dir);
                // Note: logic below uses `temp_json_path` var which relied on initial config.
                // We should update `temp_json_path` if output dir changed.
                let effective_temp_json_path = p_temp_path.clone();

                if Path::new(&effective_temp_json_path).exists() {
                    if let Ok(content) = fs::read_to_string(&effective_temp_json_path) {
                        if let Ok(existing_session) = serde_json::from_str::<SessionState>(&content)
                        {
                            app.screen =
                                AppScreen::ResumePrompt(existing_session.youtube_url.clone());
                            session = Some(existing_session);
                        } else {
                            // Corrupt session, ignore
                            app.screen = AppScreen::UrlInput;
                        }
                    } else {
                        app.screen = AppScreen::UrlInput;
                    }
                } else {
                    // No session, stay at UrlInput
                    app.screen = AppScreen::UrlInput;
                }
            }
        }

        // Update previous screen for next iteration
        previous_screen = app.screen.clone();

        // Handle Drive Auth Request

        // Handle screen transitions
        match &app.screen {
            AppScreen::GpuDetectionPrompt => {
                if let Some(response) = app.confirm_response.take() {
                    // Update config
                    let use_gpu = response;
                    if let Some(ref mut c) = app.config {
                        c.gpu_acceleration = Some(use_gpu);
                        if let Err(e) = c.save() {
                            app.log(LogLevel::Error, format!("Failed to save settings: {}", e));
                        } else {
                            app.log(LogLevel::Success, "Settings saved!".to_string());
                        }
                    }

                    // Check for session after GPU check
                    let current_config = app.config.as_ref().unwrap(); // Safe as we just used it
                    let p_temp_path = format!("{}/temp.json", current_config.default_output_dir);

                    if Path::new(&p_temp_path).exists() {
                        if let Ok(content) = fs::read_to_string(&p_temp_path) {
                            if let Ok(existing_session) =
                                serde_json::from_str::<SessionState>(&content)
                            {
                                app.screen =
                                    AppScreen::ResumePrompt(existing_session.youtube_url.clone());
                                session = Some(existing_session);
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

                            // If resuming, we might want to skip directly to processing/confirm if data is ready?
                            // But original flow went to FormatConfirm.
                            app.screen = AppScreen::FormatConfirm;
                        }
                    } else {
                        // Clean up old session
                        if let Some(ref s) = session {
                            if Path::new(&s.temp_dir).exists() {
                                fs::remove_dir_all(&s.temp_dir).ok();
                            }
                        }
                        // We must delete the specific temp.json we found
                        // Since output dir might have changed, we need to be careful.
                        // Ideally we use the path we found it at.
                        let current_config = app.config.as_ref().unwrap();
                        let p_temp_path =
                            format!("{}/temp.json", current_config.default_output_dir);
                        fs::remove_file(&p_temp_path).ok();

                        session = None;
                        app.screen = AppScreen::UrlInput;
                    }
                }
            }
            AppScreen::UrlInput => {
                if app.confirm_response.take().is_some() {
                    let input_url = app.input.trim().to_string();
                    if video::validate_youtube_url(&input_url) {
                        url = input_url;
                        app.log(LogLevel::Success, format!("Valid URL: {}", url));

                        // Use latest config for directories
                        let current_config = app.config.as_ref().unwrap();

                        // Use video ID for caching to allow fallback
                        let video_id = extract_video_id(&url).unwrap_or_else(|| {
                            // Fallback to timestamp if ID extraction fails (shouldn't happen with valid URL)
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                .to_string()
                        });

                        temp_dir =
                            format!("{}/cache_{}", current_config.default_output_dir, video_id);

                        log::info!("Using temp directory: {}", temp_dir);

                        // Do NOT remove directory if it exists, to allow cache reuse
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
                    // IMPORTANT: Use the APP config here to get latest changes
                    let config_clone = app.config.clone().unwrap_or(config.clone());

                    let url_clone = url.clone();
                    let temp_dir_clone = temp_dir.clone();
                    let temp_json_path_clone =
                        format!("{}/temp.json", config_clone.default_output_dir);
                    let custom_format_clone = custom_format.clone();
                    let existing_moments = all_moments.clone();
                    let active_security_mode_clone = app.active_security_mode;
                    let active_password_clone = app.active_password.clone();
                    let cancellation_token = app.cancellation_token.clone();

                    // Reset token before starting
                    cancellation_token.store(false, Ordering::Relaxed);

                    tokio::spawn(async move {
                        let result = run_processing(
                            tx_clone.clone(),
                            config_clone,
                            url_clone,
                            temp_dir_clone,
                            temp_json_path_clone,
                            custom_format_clone,
                            existing_moments,
                            active_security_mode_clone,
                            active_password_clone,
                            cancellation_token,
                        )
                        .await;

                        match result {
                            Ok((_moments, shorts_dir)) => {
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
            AppScreen::ShortsConfirm(_) => {
                if let Some(response) = app.confirm_response.take() {
                    if response {
                        app.log(LogLevel::Info, "Generating shorts...".to_string());

                        // Start extraction background task
                        let tx_clone = tx.clone();
                        let config_clone = app.config.clone().unwrap_or(config.clone());
                        let url_clone = url.clone();
                        let temp_dir_clone = temp_dir.clone();
                        let temp_json_path_clone =
                            format!("{}/temp.json", config_clone.default_output_dir);
                        let custom_format_clone = custom_format.clone();
                        let moments_clone = app.moments.clone();
                        let active_security_mode_clone = app.active_security_mode;
                        let active_password_clone = app.active_password.clone();
                        let cancellation_token = app.cancellation_token.clone();

                        // Reset token
                        cancellation_token.store(false, Ordering::Relaxed);

                        // We go back to processing screen to show progress
                        app.screen = AppScreen::Processing;

                        tokio::spawn(async move {
                            let result = run_extraction(
                                tx_clone.clone(),
                                config_clone.clone(),
                                url_clone,
                                temp_dir_clone,
                                temp_json_path_clone,
                                custom_format_clone,
                                moments_clone,
                                active_security_mode_clone,
                                active_password_clone,
                                cancellation_token,
                            )
                            .await;

                            match result {
                                Ok((_, shorts_dir)) => {
                                    if let Some(dir) = shorts_dir {
                                        let _ = tx_clone.send(AppMessage::Complete(format!(
                                            "Shorts saved to: {}",
                                            dir
                                        )));
                                    }
                                    let _ = tx_clone.send(AppMessage::Finished);
                                }
                                Err(e) => {
                                    let _ =
                                        tx_clone.send(AppMessage::Error(format!("Error: {}", e)));
                                    let _ = tx_clone.send(AppMessage::Finished);
                                }
                            }
                        });
                    } else {
                        app.log(LogLevel::Info, "Skipping shorts generation".to_string());
                        app.screen = AppScreen::Done;
                    }
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
    match AppConfig::load() {
        Ok(config) => Ok(config),
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("Password required") {
                // Return a dummy config to bootstrap the App into Password Input mode
                return Ok(AppConfig {
                    google_api_keys: vec![],
                    default_output_dir: "./output".to_string(),
                    extract_shorts_when_finished_moments: false,
                    use_cookies: false,
                    cookies_path: "./cookies.json".to_string(),
                    shorts_config: config::ShortsConfig::default(),
                    gpu_acceleration: None,
                    use_fast_model: true,
                    active_provider: config::AiProviderType::Google,
                    openrouter_api_keys: vec![],
                    openrouter_models: vec![],
                    openrouter_model_index: 0,

                    active_encryption_mode: security::EncryptionMode::Password,
                    active_password: None,
                    language: "en".to_string(),
                });
            }

            if err_msg.contains("Configuration file not found") {
                println!("📝 Configuration file not found. Creating default settings.json...");
                AppConfig::create_default()?;
                return AppConfig::load();
            }

            Err(e)
        }
    }
}

/// Run the main processing pipeline
#[allow(clippy::too_many_arguments)]
async fn run_processing(
    tx: TuiSender,
    config: AppConfig,
    url: String,
    temp_dir: String,
    temp_json_path: String,
    custom_format: Option<String>,
    mut all_moments: Vec<VideoMoment>,
    _active_security_mode: security::EncryptionMode,
    _active_password: Option<String>,
    cancellation_token: Arc<AtomicBool>,
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

    // Determine optimization flag based on active provider
    // Determine optimization flag based on active provider
    let optimize_for_ai = matches!(config.active_provider, config::AiProviderType::OpenRouter);

    let video_chunks = if Path::new(&temp_chunks_dir).exists()
        && fs::read_dir(&temp_chunks_dir)?.next().is_some()
    {
        // Simple check: if optimizing, we might need to invalidate cache if existing chunks are high res?
        // For simplicity, we assume cache is valid or user can clear it.
        // Actually, if we switch providers, we might get wrong chunks.
        // Let's add provider-specific suffix to chunks dir? Or just assume cache is okay for now.
        // User can manually clear cache if needed.
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
                optimize_for_ai,
            )
            .await?
        } else {
            v_chunks
        }
    } else {
        let chunks = video::calculate_chunks(duration);
        video::split_video(&temp_low_res, &temp_chunks_dir, &chunks, optimize_for_ai).await?
    };

    let _ = tx.send(AppMessage::Log(
        LogLevel::Success,
        format!("Created {} chunks", video_chunks.len()),
    ));

    // Analyze chunks with AI
    let _ = tx.send(AppMessage::Status("Analyzing with AI...".to_string()));

    // Initialize AI Client
    let ai_client = match config.active_provider {
        config::AiProviderType::Google => {
            let enabled_keys: Vec<(String, String)> = config
                .google_api_keys
                .iter()
                .filter(|k| k.enabled)
                .map(|k| (k.name.clone(), k.value.clone()))
                .collect();

            if enabled_keys.is_empty() {
                let _ = tx.send(AppMessage::Error(
                    "No enabled Google API keys found.".to_string(),
                ));
                return Ok((Vec::new(), None));
            }
            AiClient::Google(GoogleClient::new(enabled_keys, config.use_fast_model))
        }
        config::AiProviderType::OpenRouter => {
            let enabled_keys: Vec<(String, String)> = config
                .openrouter_api_keys
                .iter()
                .filter(|k| k.enabled)
                .map(|k| (k.name.clone(), k.value.clone()))
                .collect();

            if enabled_keys.is_empty() {
                let _ = tx.send(AppMessage::Error(
                    "No enabled OpenRouter API keys found.".to_string(),
                ));
                return Ok((Vec::new(), None));
            }
            // Get selected model
            let model = config
                .openrouter_models
                .get(config.openrouter_model_index)
                .cloned()
                .unwrap_or_else(|| "google/gemini-2.0-flash-001".to_string());

            AiClient::OpenRouter(OpenRouterClient::new(enabled_keys, model))
        }
    };

    // Iterate over chunks and analyze
    let mut chunks_analyzed = 0;

    for (i, chunk) in video_chunks.iter().enumerate() {
        // Check cancellation
        if cancellation_token.load(Ordering::Relaxed) {
            let _ = tx.send(AppMessage::Status("Cancelled".to_string()));
            let _ = tx.send(AppMessage::Log(
                LogLevel::Warning,
                "Processing cancelled by user".to_string(),
            ));
            // Return early - preserve temp dir
            return Ok((Vec::new(), None));
        }

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

        // Upload first
        // Process chunk with sticky session (Upload + Analyze)
        let tx_clone = tx.clone();
        let status_cb = move |msg: String| {
            let _ = tx_clone.send(AppMessage::Status(msg));
        };

        match ai_client
            .process_chunk(&chunk.file_path, chunk.start_seconds, status_cb)
            .await
        {
            Ok(moments) => {
                chunks_analyzed += 1;
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
                let err_msg = e.to_string();
                if err_msg.contains("No active API keys available")
                    || err_msg.contains("API Keys Exhausted")
                {
                    let _ = tx.send(AppMessage::Error(
                        "API Keys Exhausted during analysis.".to_string(),
                    ));
                    break;
                } else {
                    let _ = tx.send(AppMessage::Log(
                        LogLevel::Warning,
                        format!("Chunk {} analysis failed: {}", i + 1, e),
                    ));
                }
            }
        }
    }

    // Check if we found anything or if we should fallback
    if all_moments.is_empty() && chunks_analyzed < video_chunks.len() {
        // This implies we failed early or found nothing.
        // If we broke due to keys, we should fallback.
        // Since we don't track *why* we broke explicitly outside the loop easily,
        // let's assume if moments is empty we try fallback.

        let _ = tx.send(AppMessage::Status(
            "Falling back to HQ Download...".to_string(),
        ));
        let _ = tx.send(AppMessage::Log(
            LogLevel::Warning,
            "Analysis incomplete or failed. Downloading full video.".to_string(),
        ));

        let video_id = extract_video_id(&url).unwrap_or("video".to_string());
        let output_file = format!("{}/{}_full.mp4", config.default_output_dir, video_id);

        video::download_high_res(
            &url,
            &output_file,
            config.use_cookies,
            &config.cookies_path,
            None,
        )
        .await?;

        let _ = tx.send(AppMessage::Complete(format!(
            "Full video saved to: {}",
            output_file
        )));

        return Ok((Vec::new(), Some(config.default_output_dir.clone())));
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
        let _ = tx.send(AppMessage::RequestShortsConfirm(all_moments.len()));
        return Ok((all_moments, None));
    }

    run_extraction(
        tx,
        config,
        url,
        temp_dir,
        temp_json_path,
        custom_format,
        all_moments,
        _active_security_mode,
        _active_password,
        cancellation_token,
    )
    .await
}

/// Run the extraction phase (high-res download and clipping)
#[allow(clippy::too_many_arguments)]
async fn run_extraction(
    tx: TuiSender,
    config: AppConfig,
    url: String,
    temp_dir: String,
    temp_json_path: String,
    custom_format: Option<String>,
    all_moments: Vec<VideoMoment>,
    _active_security_mode: security::EncryptionMode,
    _active_password: Option<String>,
    cancellation_token: Arc<AtomicBool>,
) -> Result<(Vec<VideoMoment>, Option<String>)> {
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
        if cancellation_token.load(Ordering::Relaxed) {
            let _ = tx.send(AppMessage::Status("Cancelled".to_string()));
            let _ = tx.send(AppMessage::Log(
                LogLevel::Warning,
                "Extraction cancelled by user".to_string(),
            ));
            // Return early - return whatever we created so far
            return Ok((all_moments, Some(shorts_dir)));
        }

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
