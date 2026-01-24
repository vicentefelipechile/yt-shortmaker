//! Terminal User Interface module for AutoShorts-Rust-CLI
//! Built with Ratatui for a rich interactive experience

use std::io::{self, Stdout};
use std::time::Instant;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::types::{VideoMoment, APP_NAME, APP_VERSION};

/// Messages sent from background tasks to the TUI
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// Update current status message
    Status(String),
    /// Add a log entry
    Log(LogLevel, String),
    /// Update progress (0.0 - 1.0)
    Progress(f64, String),
    /// Add a found moment
    MomentFound(VideoMoment),
    /// Task completed successfully
    Complete(String),
    /// Error occurred
    Error(String),
    /// Request URL input
    RequestUrl,
    /// Request format selection confirmation
    RequestFormatConfirm,
    /// Request shorts generation confirmation
    RequestShortsConfirm(usize),
    /// Processing finished, ready to exit
    Finished,
}

/// Log levels for messages
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Current screen/state of the application
#[derive(Debug, Clone, PartialEq)]
pub enum AppScreen {
    /// Initial loading/setup
    Setup,
    /// Asking for resume
    ResumePrompt(String), // URL to resume
    /// URL input
    UrlInput,
    /// Format selection confirmation
    FormatConfirm,
    /// Main processing dashboard
    Processing,
    /// Shorts generation confirmation
    ShortsConfirm(usize),
    /// GPU Detection Prompt
    GpuDetectionPrompt,
    /// Completed
    Done,
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: String,
}

/// Main application state
pub struct App {
    /// Current screen
    pub screen: AppScreen,
    /// Application start time
    pub start_time: Instant,
    /// Current status message
    pub status: String,
    /// Log entries
    pub logs: Vec<LogEntry>,
    /// Current progress (0.0 - 1.0)
    pub progress: f64,
    /// Progress label
    pub progress_label: String,
    /// Found moments
    pub moments: Vec<VideoMoment>,
    /// User input buffer
    pub input: String,
    /// Cursor position in input
    pub cursor_pos: usize,
    /// Whether app should quit
    pub should_quit: bool,
    /// User response for confirmations
    pub confirm_response: Option<bool>,
    /// Output directory
    pub output_dir: String,
    /// Final result message
    pub result_message: Option<String>,
    /// Config reference
    pub config: Option<AppConfig>,
}

impl App {
    /// Create new app instance
    pub fn new(output_dir: String) -> Self {
        Self {
            screen: AppScreen::Setup,
            start_time: Instant::now(),
            status: "Initializing...".to_string(),
            logs: Vec::new(),
            progress: 0.0,
            progress_label: String::new(),
            moments: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            should_quit: false,
            confirm_response: None,
            output_dir,
            result_message: None,
            config: None,
        }
    }

    /// Get formatted uptime
    pub fn uptime(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let secs = elapsed.as_secs();
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }

    /// Add a log entry
    pub fn log(&mut self, level: LogLevel, message: String) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(LogEntry {
            level,
            message,
            timestamp,
        });
        // Keep logs manageable
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: KeyCode) {
        match &self.screen {
            AppScreen::UrlInput => match key {
                KeyCode::Enter => {
                    if !self.input.trim().is_empty() {
                        self.confirm_response = Some(true);
                    }
                }
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                }
                KeyCode::Backspace => {
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                        self.input.remove(self.cursor_pos);
                    }
                }
                KeyCode::Delete => {
                    if self.cursor_pos < self.input.len() {
                        self.input.remove(self.cursor_pos);
                    }
                }
                KeyCode::Left => {
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.cursor_pos < self.input.len() {
                        self.cursor_pos += 1;
                    }
                }
                KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },
            AppScreen::ResumePrompt(_)
            | AppScreen::FormatConfirm
            | AppScreen::ShortsConfirm(_)
            | AppScreen::GpuDetectionPrompt => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.confirm_response = Some(true);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.confirm_response = Some(false);
                }
                KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },
            AppScreen::Processing => match key {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },
            AppScreen::Done => match key {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                    self.should_quit = true;
                }
                _ => {}
            },
            _ => {
                if key == KeyCode::Esc || key == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
        }
    }

    /// Process messages from background tasks
    pub fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::Status(s) => self.status = s,
            AppMessage::Log(level, message) => self.log(level, message),
            AppMessage::Progress(p, label) => {
                self.progress = p;
                self.progress_label = label;
            }
            AppMessage::MomentFound(moment) => {
                self.moments.push(moment);
            }
            AppMessage::Complete(msg) => {
                self.log(LogLevel::Success, msg);
            }
            AppMessage::Error(msg) => {
                self.log(LogLevel::Error, msg);
            }
            AppMessage::RequestUrl => {
                self.screen = AppScreen::UrlInput;
                self.input.clear();
                self.cursor_pos = 0;
            }
            AppMessage::RequestFormatConfirm => {
                self.screen = AppScreen::FormatConfirm;
                self.confirm_response = None;
            }
            AppMessage::RequestShortsConfirm(count) => {
                self.screen = AppScreen::ShortsConfirm(count);
                self.confirm_response = None;
            }
            AppMessage::Finished => {
                self.screen = AppScreen::Done;
            }
        }
    }
}

/// Setup the terminal for TUI
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal to normal state
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Render the TUI
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: Header, Content, Footer
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Header
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Footer
        ])
        .split(area);

    render_header(frame, app, main_layout[0]);
    render_content(frame, app, main_layout[1]);
    render_footer(frame, app, main_layout[2]);
}

/// Render the header section
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" {} v{} ", APP_NAME, APP_VERSION),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = header_block.inner(area);
    frame.render_widget(header_block, area);

    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    // Left side: Output dir and status
    let left_text = Text::from(vec![
        Line::from(vec![
            Span::raw("📁 Output: "),
            Span::styled(&app.output_dir, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("⚡ Status: "),
            Span::styled(&app.status, Style::default().fg(Color::Green)),
        ]),
    ]);
    frame.render_widget(Paragraph::new(left_text), header_layout[0]);

    // Right side: Uptime and moments count
    let right_text = Text::from(vec![
        Line::from(vec![
            Span::raw("⏱  Uptime: "),
            Span::styled(app.uptime(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("🎬 Moments: "),
            Span::styled(
                app.moments.len().to_string(),
                Style::default().fg(Color::Green),
            ),
        ]),
    ]);
    frame.render_widget(Paragraph::new(right_text), header_layout[1]);
}

/// Render the main content area
fn render_content(frame: &mut Frame, app: &App, area: Rect) {
    match &app.screen {
        AppScreen::Setup => render_setup(frame, app, area),
        AppScreen::ResumePrompt(url) => render_resume_prompt(frame, url, area),
        AppScreen::UrlInput => render_url_input(frame, app, area),
        AppScreen::FormatConfirm => render_format_confirm(frame, area),
        AppScreen::Processing => render_processing(frame, app, area),
        AppScreen::ShortsConfirm(count) => render_shorts_confirm(frame, *count, area),
        AppScreen::GpuDetectionPrompt => render_gpu_prompt(frame, area),
        AppScreen::Done => render_done(frame, app, area),
    }
}

fn render_gpu_prompt(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" 🚀 Hardware Acceleration Detected ");

    let text = Text::from(vec![
        Line::from(""),
        Line::from("NVIDIA GPU detected!"),
        Line::from(""),
        Line::from("Do you want to use NVENC for faster video rendering?"),
        Line::from(""),
        Line::from(vec![
            Span::raw("(Y)es - Use NVENC "),
            Span::styled("(Recommended)", Style::default().fg(Color::Green)),
        ]),
        Line::from("(N)o  - Use CPU encoding"),
        Line::from(""),
    ]);

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_setup(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Initialization ");

    let text = Paragraph::new("Loading configuration...")
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(text, area);
}

fn render_resume_prompt(frame: &mut Frame, url: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" 📋 Previous Session Found ");

    let text = Text::from(vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Found previous session for: "),
            Span::styled(url, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from("Do you want to resume? (Y/n)").style(Style::default().fg(Color::Yellow)),
    ]);

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_url_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" 🎬 Enter YouTube URL ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let input_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);

    // Instructions
    let instructions = Paragraph::new("Enter a valid YouTube video URL and press Enter:")
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(instructions, input_layout[0]);

    // Input field
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let input_text = Paragraph::new(app.input.as_str())
        .block(input_block)
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(input_text, input_layout[1]);

    // Set cursor position
    frame.set_cursor_position((
        input_layout[1].x + 1 + app.cursor_pos as u16,
        input_layout[1].y + 1,
    ));
}

fn render_format_confirm(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" 📹 Format Selection ");

    let text = Text::from(vec![
        Line::from(""),
        Line::from("Do you want to manually select the video format?"),
        Line::from(""),
        Line::from("(Y)es - Show available formats and select one"),
        Line::from("(N)o  - Use default format (recommended)"),
        Line::from(""),
    ]);

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_processing(frame: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Progress
            Constraint::Min(5),    // Logs
            Constraint::Length(8), // Moments preview
        ])
        .split(area);

    // Progress bar
    let progress_block = Block::default().borders(Borders::ALL).title(" Progress ");

    let progress_inner = progress_block.inner(layout[0]);
    frame.render_widget(progress_block, layout[0]);

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .percent((app.progress * 100.0) as u16)
        .label(&app.progress_label);
    frame.render_widget(gauge, progress_inner);

    // Logs
    let logs_block = Block::default()
        .borders(Borders::ALL)
        .title(" Activity Log ");

    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(10)
        .map(|entry| {
            let (icon, color) = match entry.level {
                LogLevel::Info => ("ℹ️ ", Color::Blue),
                LogLevel::Success => ("✔ ", Color::Green),
                LogLevel::Warning => ("⚠ ", Color::Yellow),
                LogLevel::Error => ("✘ ", Color::Red),
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(icon),
                Span::styled(&entry.message, Style::default().fg(color)),
            ]))
        })
        .collect();

    let logs_list = List::new(log_items).block(logs_block);
    frame.render_widget(logs_list, layout[1]);

    // Moments preview
    let moments_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 🎬 Moments Found ({}) ", app.moments.len()));

    let moment_items: Vec<ListItem> = app
        .moments
        .iter()
        .rev()
        .take(5)
        .map(|m| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{} - {}] ", m.start_time, m.end_time),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(&m.category, Style::default().fg(Color::Magenta)),
                Span::raw(" - "),
                Span::styled(&m.description, Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let moments_list = List::new(moment_items).block(moments_block);
    frame.render_widget(moments_list, layout[2]);
}

fn render_shorts_confirm(frame: &mut Frame, count: usize, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" ✨ Analysis Complete ");

    let text = Text::from(vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Found "),
            Span::styled(
                count.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" moments suitable for YouTube Shorts!"),
        ]),
        Line::from(""),
        Line::from("Do you want to generate shorts from the source video?"),
        Line::from(""),
        Line::from("(Y)es - Download high-res and extract clips"),
        Line::from("(N)o  - Save moments only"),
        Line::from(""),
    ]);

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_done(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" ✅ Complete ");

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Process completed successfully!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(ref msg) = app.result_message {
        lines.push(Line::from(Span::styled(
            msg,
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::raw("Total moments found: "),
        Span::styled(
            app.moments.len().to_string(),
            Style::default().fg(Color::Cyan),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from("Press any key to exit..."));

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}

/// Render the footer with keyboard shortcuts
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match &app.screen {
        AppScreen::UrlInput => "Enter: Submit | Esc: Quit",
        AppScreen::ResumePrompt(_)
        | AppScreen::FormatConfirm
        | AppScreen::ShortsConfirm(_)
        | AppScreen::GpuDetectionPrompt => "Y: Yes | N: No | Esc: Quit",
        AppScreen::Processing => "Q/Esc: Quit",
        AppScreen::Done => "Press any key to exit",
        _ => "Esc: Quit",
    };

    let footer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Keyboard Shortcuts ");

    let footer_text = Paragraph::new(shortcuts)
        .block(footer_block)
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(footer_text, area);
}

/// Channel for sending messages to the TUI
pub type TuiSender = mpsc::UnboundedSender<AppMessage>;
pub type TuiReceiver = mpsc::UnboundedReceiver<AppMessage>;

/// Create a new message channel
pub fn create_channel() -> (TuiSender, TuiReceiver) {
    mpsc::unbounded_channel()
}
