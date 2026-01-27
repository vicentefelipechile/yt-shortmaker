use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};
use std::env;
use std::fs::{self, File};
use std::io::{self, Stdout, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
#[cfg(windows)]
use zip::ZipArchive;

/// Status of the setup process
#[derive(Debug, Clone, PartialEq)]
enum SetupStatus {
    Welcome,
    Downloading {
        file: String,
        progress: f64, // 0.0 - 1.0
        details: String,
    },
    #[cfg(windows)]
    Extracting {
        details: String,
    },
    Error(String),
    Complete,
}

/// Run the setup wizard if dependencies are missing
pub async fn run_setup_wizard() -> Result<()> {
    // Check if we need to run setup
    if crate::video::check_dependencies().is_ok() {
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_setup_app(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_setup_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut status = SetupStatus::Welcome;
    let install_dir = get_install_dir()?;
    let bin_dir = install_dir.join("bin");

    // Ensure bin directory exists
    fs::create_dir_all(&bin_dir)?;

    // Add bin dir to PATH for this process immediately so check_dependencies works later
    add_to_process_path(&bin_dir);

    // Main loop
    loop {
        terminal.draw(|f| render_setup(f, &status, &bin_dir))?;

        // Handle input only if not downloading/extracting
        match &status {
            SetupStatus::Welcome => {
                if event::poll(Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Enter => {
                                // Start installation
                                match perform_installation(&mut status, &bin_dir, terminal).await {
                                    Ok(_) => {
                                        // After installation, check if successful
                                        status = SetupStatus::Complete;
                                        // Send notification
                                        use notify_rust::Notification;
                                        let _ = Notification::new()
                                            .summary("YT ShortMaker Setup")
                                            .body(
                                                "Installation of ffmpeg and yt-dlp completed successfully!",
                                            )
                                            .show();
                                    }
                                    Err(err) => {
                                        status = SetupStatus::Error(err.to_string());
                                    }
                                }
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                return Err(anyhow!("Setup cancelled by user"));
                            }
                            _ => {}
                        }
                    }
                }
            }
            SetupStatus::Complete => {
                if event::poll(Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
            }
            SetupStatus::Error(_) => {
                if event::poll(Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                                return Err(anyhow!("Setup failed"));
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {
                // Downloading/Extracting - just wait for redraw
            }
        }
    }
}

async fn perform_installation(
    status: &mut SetupStatus,
    bin_dir: &Path,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    // 1. Download yt-dlp
    let ytdlp_url = if cfg!(windows) {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    };

    let ytdlp_name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let ytdlp_path = bin_dir.join(ytdlp_name);

    if !ytdlp_path.exists() {
        download_file(ytdlp_url, &ytdlp_path, "yt-dlp", status, terminal).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&ytdlp_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&ytdlp_path, perms)?;
        }
    }

    // 2. Download ffmpeg
    #[cfg(windows)]
    {
        // Windows: Download zip and extract
        let ffmpeg_url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
        let zip_path = bin_dir.join("ffmpeg.zip");

        if !bin_dir.join("ffmpeg.exe").exists() {
            download_file(ffmpeg_url, &zip_path, "ffmpeg (zip)", status, terminal).await?;

            *status = SetupStatus::Extracting {
                details: "Extracting ffmpeg...".to_string(),
            };
            terminal.draw(|f| render_setup(f, status, bin_dir))?;

            extract_ffmpeg_windows(&zip_path, bin_dir)?;

            // Cleanup zip
            fs::remove_file(zip_path).ok();
        }
    }

    Ok(())
}

async fn download_file(
    url: &str,
    path: &Path,
    name: &str,
    status: &mut SetupStatus,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    let total_size = response.content_length().unwrap_or(0);

    let mut stream = response.bytes_stream();
    let mut file = File::create(path)?;
    let mut downloaded: u64 = 0;

    *status = SetupStatus::Downloading {
        file: name.to_string(),
        progress: 0.0,
        details: "Starting...".to_string(),
    };
    terminal.draw(|f| render_setup(f, status, path.parent().unwrap()))?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if total_size > 0 {
            let progress = downloaded as f64 / total_size as f64;
            let details = format!(
                "{:.1} MB / {:.1} MB",
                downloaded as f64 / 1_000_000.0,
                total_size as f64 / 1_000_000.0
            );
            *status = SetupStatus::Downloading {
                file: name.to_string(),
                progress,
                details,
            };
            terminal.draw(|f| render_setup(f, status, path.parent().unwrap()))?;
        }
    }

    Ok(())
}

#[cfg(windows)]
fn extract_ffmpeg_windows(zip_path: &Path, bin_dir: &Path) -> Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // We only care about bin/ffmpeg.exe, bin/ffprobe.exe
        if name.ends_with("ffmpeg.exe") || name.ends_with("ffprobe.exe") {
            let file_name = Path::new(&name).file_name().unwrap();
            let dest_path = bin_dir.join(file_name);
            let mut outfile = File::create(&dest_path)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

fn get_install_dir() -> Result<PathBuf> {
    if let Some(mut path) = dirs::data_local_dir() {
        path.push("yt-shortmaker");
        Ok(path)
    } else {
        Err(anyhow!("Could not determine local data directory"))
    }
}

pub fn get_bin_dir() -> PathBuf {
    get_install_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("bin")
}

pub fn add_to_process_path(bin_dir: &Path) {
    if let Some(path) = env::var_os("PATH") {
        let mut paths = env::split_paths(&path).collect::<Vec<_>>();
        paths.insert(0, bin_dir.to_path_buf());
        if let Ok(new_path) = env::join_paths(paths) {
            env::set_var("PATH", new_path);
        }
    }
}

fn render_setup(frame: &mut Frame, status: &SetupStatus, install_path: &Path) {
    let area = frame.area();

    // Center a popup block
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(popup_layout[1])[1];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" YT ShortMaker Setup ")
        .style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let content_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Status Title
            Constraint::Length(4), // Details
            Constraint::Length(4), // Progress Bar
            Constraint::Min(2),    // Instructions
        ])
        .split(inner);

    match status {
        SetupStatus::Welcome => {
            let text = vec![
                Line::from(Span::styled(
                    "Missing Components Detected!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("The application requires 'ffmpeg' and 'yt-dlp' to function."),
                Line::from("They were not found on your system."),
                Line::from(""),
                Line::from(vec![
                    Span::raw("We can download and install them to: "),
                    Span::styled(
                        install_path.to_string_lossy(),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
            ];
            frame.render_widget(
                Paragraph::new(text).wrap(Wrap { trim: true }),
                content_layout[1],
            );

            let instructions = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[ENTER]",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Install components"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "[ESC]",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Cancel and Exit"),
                ]),
            ]);
            frame.render_widget(instructions, content_layout[3]);
        }
        SetupStatus::Downloading {
            file,
            progress,
            details,
        } => {
            frame.render_widget(
                Paragraph::new(format!("Downloading {}...", file)),
                content_layout[0],
            );
            frame.render_widget(Paragraph::new(details.clone()), content_layout[1]);

            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .ratio(*progress);
            frame.render_widget(gauge, content_layout[2]);
        }
        #[cfg(windows)]
        SetupStatus::Extracting { details } => {
            frame.render_widget(Paragraph::new("Installing..."), content_layout[0]);
            frame.render_widget(Paragraph::new(details.clone()), content_layout[1]);
        }
        SetupStatus::Complete => {
            let text = vec![
                Line::from(Span::styled(
                    "Installation Complete!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Components have been installed successfully."),
                Line::from("You can now use the application."),
            ];
            frame.render_widget(
                Paragraph::new(text).wrap(Wrap { trim: true }),
                content_layout[1],
            );

            let instructions = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[ENTER]",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Start Application"),
                ]),
            ]);
            frame.render_widget(instructions, content_layout[3]);
        }
        SetupStatus::Error(e) => {
            let text = vec![
                Line::from(Span::styled(
                    "Setup Failed!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("Error: {}", e)),
            ];
            frame.render_widget(
                Paragraph::new(text).wrap(Wrap { trim: true }),
                content_layout[1],
            );

            let instructions = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[ESC]",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Exit"),
                ]),
            ]);
            frame.render_widget(instructions, content_layout[3]);
        }
    }
}
