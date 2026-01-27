//! Terminal User Interface module for YT ShortMaker
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
    style::{Color, Modifier, Style},
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
    /// Shorts generation confirmation
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
    /// First run API Key input
    ApiKeyInput,
    /// Main Menu
    MainMenu,
    /// Settings Editor
    SettingsEditor,
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
    /// API Key Manager List
    ApiKeysManager,
    /// Input for adding a new API Key
    ApiKeyAddInput,
    /// Rename API Key
    ApiKeyRename,
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: String,
}

/// Simple enum to represent a setting type for editing
#[derive(Debug, Clone)]
pub enum SettingType {
    String,
    Bool,
    Float,
}

/// Definition of a setting to be edited
#[derive(Debug, Clone)]
pub struct SettingItem {
    pub name: String,
    pub key: String, // Internal key like "output_dir"
    pub value: String,
    pub kind: SettingType,
    pub description: String,
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
    /// Whether an error occurred during processing
    pub has_error: bool,

    // -- Menu & Settings State --
    /// Menu selection index
    pub menu_index: usize,
    /// Settings selection index
    pub settings_index: usize,
    /// Whether we are currently editing a setting
    pub editing_setting: bool,
    /// Buffer for editing a setting
    pub setting_input: String,
    /// List of editable settings
    pub settings_items: Vec<SettingItem>,

    // -- API Key Manager State --
    /// Index for API key list selection
    pub api_keys_index: usize,
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
            has_error: false,
            menu_index: 0,
            settings_index: 0,
            editing_setting: false,
            setting_input: String::new(),
            settings_items: Vec::new(),
            api_keys_index: 0,
        }
    }

    /// Reload settings items from current config
    pub fn reload_settings_items(&mut self) {
        if let Some(config) = &self.config {
            self.settings_items = vec![
                SettingItem {
                    name: "Output Directory".to_string(),
                    key: "output_dir".to_string(),
                    value: config.default_output_dir.clone(),
                    kind: SettingType::String,
                    description: "Directory where shorts are saved".to_string(),
                },
                SettingItem {
                    name: "Auto Extract".to_string(),
                    key: "auto_extract".to_string(),
                    value: config.extract_shorts_when_finished_moments.to_string(),
                    kind: SettingType::Bool,
                    description: "Extract shorts automatically after analysis".to_string(),
                },
                SettingItem {
                    name: "Use Cookies".to_string(),
                    key: "use_cookies".to_string(),
                    value: config.use_cookies.to_string(),
                    kind: SettingType::Bool,
                    description: "Use cookies for yt-dlp".to_string(),
                },
                SettingItem {
                    name: "Cookies Path".to_string(),
                    key: "cookies_path".to_string(),
                    value: config.cookies_path.clone(),
                    kind: SettingType::String,
                    description: "Path to cookies.txt/json".to_string(),
                },
                SettingItem {
                    name: "GPU Acceleration".to_string(),
                    key: "gpu".to_string(),
                    value: config.gpu_acceleration.unwrap_or(false).to_string(),
                    kind: SettingType::Bool,
                    description: "Use NVIDIA NVENC for faster rendering".to_string(),
                },
                SettingItem {
                    name: "Background Opacity".to_string(),
                    key: "bg_opacity".to_string(),
                    value: config.shorts_config.background_opacity.to_string(),
                    kind: SettingType::Float,
                    description: "Opacity of background video (0.0 - 1.0)".to_string(),
                },
                SettingItem {
                    name: "Main Video Zoom".to_string(),
                    key: "zoom".to_string(),
                    value: config.shorts_config.main_video_zoom.to_string(),
                    kind: SettingType::Float,
                    description: "Zoom level (0.5 = 50%, 1.0 = 100%)".to_string(),
                },
            ];
        }
    }

    /// Apply edited setting back to config
    pub fn apply_setting(&mut self) {
        if let Some(config) = &mut self.config {
            if self.settings_index < self.settings_items.len() {
                let item = &self.settings_items[self.settings_index];
                let val = &self.setting_input;

                match item.key.as_str() {
                    "output_dir" => config.default_output_dir = val.clone(),
                    "auto_extract" => {
                        config.extract_shorts_when_finished_moments = val.parse().unwrap_or(false)
                    }
                    "use_cookies" => config.use_cookies = val.parse().unwrap_or(false),
                    "cookies_path" => config.cookies_path = val.clone(),
                    "gpu" => config.gpu_acceleration = Some(val.parse().unwrap_or(false)),
                    "bg_opacity" => {
                        config.shorts_config.background_opacity = val.parse().unwrap_or(0.4)
                    }
                    "zoom" => config.shorts_config.main_video_zoom = val.parse().unwrap_or(0.7),
                    _ => {}
                }

                // Try to save to disk immediately
                let _ = config.save();

                // Reload items to reflect changes
                self.reload_settings_items();
            }
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
        // Also send to global logger
        match level {
            LogLevel::Info => log::info!("{}", message),
            LogLevel::Success => log::info!("(SUCCESS) {}", message),
            LogLevel::Warning => log::warn!("{}", message),
            LogLevel::Error => log::error!("{}", message),
        }

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
            AppScreen::ApiKeyInput => match key {
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
            AppScreen::ApiKeysManager => match key {
                KeyCode::Up => {
                    if self.api_keys_index > 0 {
                        self.api_keys_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if let Some(config) = &self.config {
                        if !config.google_api_keys.is_empty()
                            && self.api_keys_index < config.google_api_keys.len() - 1
                        {
                            self.api_keys_index += 1;
                        }
                    }
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.screen = AppScreen::ApiKeyAddInput;
                    self.input.clear();
                    self.cursor_pos = 0;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Some(config) = &self.config {
                        if !config.google_api_keys.is_empty() {
                            self.screen = AppScreen::ApiKeyRename;
                            self.input = config.google_api_keys[self.api_keys_index].name.clone();
                            self.cursor_pos = self.input.len();
                        }
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(config) = &mut self.config {
                        if !config.google_api_keys.is_empty() {
                            let enabled = &mut config.google_api_keys[self.api_keys_index].enabled;
                            *enabled = !*enabled;
                            let _ = config.save();
                        }
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let Some(config) = &mut self.config {
                        if !config.google_api_keys.is_empty() {
                            config.google_api_keys.remove(self.api_keys_index);
                            if self.api_keys_index > 0
                                && self.api_keys_index >= config.google_api_keys.len()
                            {
                                self.api_keys_index -= 1;
                            }
                            let _ = config.save();
                        }
                    }
                }
                KeyCode::Esc => {
                    self.screen = AppScreen::MainMenu;
                }
                _ => {}
            },
            AppScreen::ApiKeyAddInput => match key {
                KeyCode::Enter => {
                    if !self.input.trim().is_empty() {
                        if let Some(config) = &mut self.config {
                            config.google_api_keys.push(crate::config::ApiKey {
                                value: self.input.trim().to_string(),
                                name: format!("Gemini Key {}", config.google_api_keys.len() + 1),
                                enabled: true,
                            });
                            let _ = config.save();
                            self.screen = AppScreen::ApiKeysManager;
                        }
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
                    self.screen = AppScreen::ApiKeysManager;
                }
                _ => {}
            },
            AppScreen::ApiKeyRename => match key {
                KeyCode::Enter => {
                    if !self.input.trim().is_empty() {
                        if let Some(config) = &mut self.config {
                            config.google_api_keys[self.api_keys_index].name =
                                self.input.trim().to_string();
                            let _ = config.save();
                            self.screen = AppScreen::ApiKeysManager;
                        }
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
                    self.screen = AppScreen::ApiKeysManager;
                }
                _ => {}
            },
            AppScreen::MainMenu => match key {
                KeyCode::Up => {
                    if self.menu_index > 0 {
                        self.menu_index -= 1;
                    } else {
                        self.menu_index = 3; // Loop to bottom
                    }
                }
                KeyCode::Down => {
                    if self.menu_index < 3 {
                        self.menu_index += 1;
                    } else {
                        self.menu_index = 0; // Loop to top
                    }
                }
                KeyCode::Enter => {
                    match self.menu_index {
                        0 => {
                            // Comenzar
                            self.screen = AppScreen::UrlInput;
                        }
                        1 => {
                            // Config
                            self.reload_settings_items();
                            self.settings_index = 0;
                            self.screen = AppScreen::SettingsEditor;
                        }
                        2 => {
                            // API Keys
                            self.screen = AppScreen::ApiKeysManager;
                            self.api_keys_index = 0;
                        }
                        3 => {
                            // Salir
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
                KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },
            AppScreen::SettingsEditor => {
                if self.editing_setting {
                    match key {
                        KeyCode::Enter => {
                            self.apply_setting();
                            self.editing_setting = false;
                        }
                        KeyCode::Esc => {
                            self.editing_setting = false;
                            self.setting_input.clear();
                        }
                        KeyCode::Char(c) => {
                            self.setting_input.push(c);
                        }
                        KeyCode::Backspace => {
                            self.setting_input.pop();
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        KeyCode::Up => {
                            if self.settings_index > 0 {
                                self.settings_index -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if self.settings_index < self.settings_items.len() - 1 {
                                self.settings_index += 1;
                            }
                        }
                        KeyCode::Enter => {
                            let item = &self.settings_items[self.settings_index];
                            if let SettingType::Bool = item.kind {
                                // Toggle bool immediately
                                let current = item.value.parse().unwrap_or(false);
                                self.setting_input = (!current).to_string();
                                self.apply_setting();
                            } else {
                                // Edit mode
                                self.setting_input = item.value.clone();
                                self.editing_setting = true;
                            }
                        }
                        KeyCode::Esc => {
                            self.screen = AppScreen::MainMenu;
                        }
                        _ => {}
                    }
                }
            }
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
                    // Go back to menu instead of quit?
                    self.screen = AppScreen::MainMenu;
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
                    // Back to Main Menu
                    self.screen = AppScreen::MainMenu;
                }
                _ => {}
            },
            AppScreen::Processing => match key {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.screen = AppScreen::MainMenu;
                }
                _ => {}
            },
            AppScreen::Done => match key {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                    // Return to main menu instead of quit
                    self.screen = AppScreen::MainMenu;
                    // self.should_quit = true;
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
                self.log(LogLevel::Error, msg.clone());
                self.has_error = true;
                self.result_message = Some(msg);
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
        AppScreen::ApiKeyInput => render_apikey_input(frame, app, area),
        AppScreen::MainMenu => render_main_menu(frame, app, area),
        AppScreen::SettingsEditor => render_settings_editor(frame, app, area),
        AppScreen::ResumePrompt(url) => render_resume_prompt(frame, url, area),
        AppScreen::UrlInput => render_url_input(frame, app, area),
        AppScreen::FormatConfirm => render_format_confirm(frame, area),
        AppScreen::Processing => render_processing(frame, app, area),
        AppScreen::ShortsConfirm(count) => render_shorts_confirm(frame, *count, area),
        AppScreen::GpuDetectionPrompt => render_gpu_prompt(frame, area),
        AppScreen::Done => render_done(frame, app, area),
        AppScreen::ApiKeysManager => render_api_keys_manager(frame, app, area),
        AppScreen::ApiKeyAddInput => render_api_key_add_input(frame, app, area),
        AppScreen::ApiKeyRename => render_api_key_rename(frame, app, area),
    }
}

fn render_api_keys_manager(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" API Keys Manager ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // List
            Constraint::Length(3), // Instructions
        ])
        .split(inner);

    if let Some(config) = &app.config {
        let items: Vec<ListItem> = config
            .google_api_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let is_selected = i == app.api_keys_index;
                let bg_color = if is_selected {
                    Color::DarkGray
                } else {
                    Color::Reset
                };
                let prefix = if is_selected { "> " } else { "  " };

                let check = if key.enabled { "[x]" } else { "[ ]" };

                // Mask the key: "AIza...1234"
                let masked = if key.value.len() > 10 {
                    format!(
                        "{}...{}",
                        &key.value[0..4],
                        &key.value[key.value.len() - 4..]
                    )
                } else {
                    "***".to_string()
                };

                let content = format!("{} {} {} ({})", prefix, check, key.name, masked);
                let style = if key.enabled {
                    Style::default().bg(bg_color)
                } else {
                    Style::default().bg(bg_color).fg(Color::Gray)
                };
                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::NONE));
        frame.render_widget(list, layout[0]);
    }

    let help = Paragraph::new("[A] Add   [R] Rename   [Space] Toggle   [D] Delete   [Esc] Back")
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(help, layout[1]);
}

fn render_api_key_rename(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Rename API Key ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Input
            Constraint::Min(1),    // Help
        ])
        .split(inner);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let input = Paragraph::new(app.input.as_str()).block(input_block);
    frame.render_widget(input, layout[1]);

    let help =
        Paragraph::new("Enter new name for the key.\nPress [Enter] to save, [Esc] to cancel.")
            .style(Style::default().fg(Color::Gray));
    frame.render_widget(help, layout[2]);

    // Cursor
    frame.set_cursor_position((layout[1].x + 1 + app.cursor_pos as u16, layout[1].y + 1));
}

fn render_api_key_add_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" Add New API Key ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Input
            Constraint::Min(1),    // Help
        ])
        .split(inner);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let input = Paragraph::new(app.input.as_str()).block(input_block);
    frame.render_widget(input, layout[1]);

    let help = Paragraph::new(
        "Paste your Google Gemini API Key here.\nPress [Enter] to save, [Esc] to cancel.",
    )
    .style(Style::default().fg(Color::Gray));
    frame.render_widget(help, layout[2]);

    // Cursor
    frame.set_cursor_position((layout[1].x + 1 + app.cursor_pos as u16, layout[1].y + 1));
}

fn render_apikey_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" 🔑 Google Gemini API Key Required ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let input_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Instructions
            Constraint::Length(3), // Input
            Constraint::Min(0),    // Note
        ])
        .split(inner);

    // Instructions
    let instructions = Paragraph::new(
        "Welcome! It looks like this is your first time running AutoShorts.\nPlease enter your Google Gemini API Key to continue.",
    )
    .style(Style::default().fg(Color::White))
    .wrap(Wrap { trim: true });
    frame.render_widget(instructions, input_layout[0]);

    // Input field
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    // Mask input for API key? Usually good practice, but user might want to check it.
    // Let's show it for now as it helps debugging typos.
    let input_text = Paragraph::new(app.input.as_str())
        .block(input_block)
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(input_text, input_layout[1]);

    // Note
    let note = Paragraph::new(
        "To get an API Key, visit https://aistudio.google.com/app/apikey\nPress Enter to save.",
    )
    .style(Style::default().fg(Color::Gray));
    frame.render_widget(note, input_layout[2]);

    // Set cursor position
    frame.set_cursor_position((
        input_layout[1].x + 1 + app.cursor_pos as u16,
        input_layout[1].y + 1,
    ));
}

fn render_main_menu(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Menu Principal ");

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let options = &["Comenzar", "Configuracion", "Administrar Keys", "Salir"];

    let list_area = Rect {
        x: area.width / 2 - 15,
        y: area.height / 2 - 5,
        width: 30,
        height: 12,
    };

    // Ensure we don't go out of bounds if terminal is small
    let list_area = list_area.intersection(inner_area);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, &text)| {
            let style = if i == app.menu_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            // Center text in item
            let content = format!(" {:^26} ", text);
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Seleccione una opcion "),
    );

    frame.render_widget(list, list_area);
}

fn render_settings_editor(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" Configuracion ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // List
            Constraint::Length(3), // Help/Edit area
        ])
        .split(inner);

    // Render list
    let items: Vec<ListItem> = app
        .settings_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == app.settings_index;

            let val_color = if is_selected {
                Color::Yellow
            } else {
                Color::White
            };
            let key_color = if is_selected {
                Color::Cyan
            } else {
                Color::Gray
            };

            let prefix = if is_selected { "> " } else { "  " };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(val_color)),
                Span::styled(
                    format!("{:<20}: ", item.name),
                    Style::default().fg(key_color),
                ),
                Span::styled(&item.value, Style::default().fg(val_color)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::DarkGray))
        .block(Block::default().borders(Borders::NONE));

    frame.render_widget(list, layout[0]);

    // Render help or edit box
    if app.editing_setting {
        let edit_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(" Editing Value ");

        let input = Paragraph::new(app.setting_input.as_str())
            .block(edit_block)
            .style(Style::default().fg(Color::Yellow));

        frame.render_widget(input, layout[1]);
    } else if app.settings_index < app.settings_items.len() {
        let help_text = &app.settings_items[app.settings_index].description;
        let help =
            Paragraph::new(format!("ℹ️ {}", help_text)).style(Style::default().fg(Color::Gray));
        frame.render_widget(help, layout[1]);
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

    // Progress bar customization based on state
    let (prog_title, prog_color) = match app.screen {
        AppScreen::Done => {
            if app.has_error {
                (" ❌ Failed ", Color::Red)
            } else {
                (" ✅ Complete ", Color::Green)
            }
        }
        _ => (" Progress ", Color::Cyan),
    };

    let progress_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(prog_color))
        .title(prog_title);

    let progress_inner = progress_block.inner(layout[0]);
    frame.render_widget(progress_block, layout[0]);

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(prog_color).bg(Color::DarkGray))
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
    let (title, border_color) = if app.has_error {
        (" ❌ Failed ", Color::Red)
    } else {
        (" ✅ Complete ", Color::Green)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let (msg, msg_color) = if app.has_error {
        ("Process failed with errors.", Color::Red)
    } else {
        ("Process completed successfully!", Color::Green)
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            msg,
            Style::default().fg(msg_color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(ref msg) = app.result_message {
        lines.push(Line::from(Span::styled(
            msg,
            Style::default().fg(if app.has_error {
                Color::Red
            } else {
                Color::Yellow
            }),
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
    lines.push(Line::from("Press Enter or Q to return to Menu..."));

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}

/// Render the footer with keyboard shortcuts
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match &app.screen {
        AppScreen::MainMenu => "Arrows: Navigate | Enter: Select | Esc: Exit",
        AppScreen::SettingsEditor => {
            if app.editing_setting {
                "Enter: Save | Esc: Cancel Log"
            } else {
                "Arrows: Navigate | Enter: Edit | Esc: Back"
            }
        }
        AppScreen::UrlInput => "Enter: Submit | Esc: Back",
        AppScreen::ResumePrompt(_)
        | AppScreen::FormatConfirm
        | AppScreen::ShortsConfirm(_)
        | AppScreen::GpuDetectionPrompt => "Y: Yes | N: No | Esc: Menu",
        AppScreen::Processing => "Q/Esc: Quit",
        AppScreen::Done => "Enter/Q: Menu",
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
