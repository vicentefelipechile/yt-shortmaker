//! Terminal User Interface module for YT ShortMaker
//! Built with Ratatui for a rich interactive experience

use std::io::{self, Stdout};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
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

    /// Completed
    Done,
    /// API Key Manager List
    ApiKeysManager,
    /// Input for adding a new API Key
    ApiKeyAddInput,
    /// Rename API Key
    ApiKeyRename,
    /// Security Setup (First run or migration)
    SecuritySetup,
    /// Password Input (Startup)
    PasswordInput,
    /// Language Selection Menu
    LanguageMenu,
    /// Confirmation for cancelling processing
    ProcessingCancelConfirm,
    /// Export shorts main screen
    ExportShorts,
    /// Select folders containing clips
    ExportSelectFolders,
    /// Select or create plano template
    ExportSelectPlano,
    /// Preview the export result
    ExportPreview,
    /// Export processing
    ExportProcessing,
    /// Confirmation for cancelling export processing
    ExportProcessingCancellationConfirm,
    /// Export process finished
    ExportDone,
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
    // String,
    Bool,
    Float,
    Path,
    Directory,
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
    pub logs: Vec<LogEntry>,

    // Security State
    pub security_password_input: String,
    pub security_confirm_input: String, // For setting new password
    pub security_selected_mode: usize,  // 0: None, 1: Simple, 2: Password
    pub security_error: Option<String>,

    // Active Security Context (for saving)
    pub active_security_mode: crate::security::EncryptionMode,
    pub active_password: Option<String>,
    /// Current progress (0.0 - 1.0)
    pub progress: f64,
    /// Progress label
    pub progress_label: String,
    /// Selected language index (0: English, 1: Spanish)
    pub language_index: usize,
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

    /// Cancellation token for background tasks
    pub cancellation_token: Arc<AtomicBool>,

    // -- Export Shorts State --
    /// Selected folders containing clips
    pub export_clip_folders: Vec<String>,
    /// Selected plano file path
    pub export_plano_path: Option<String>,
    /// Loaded plano objects
    pub export_plano: Vec<crate::exporter::PlanoObject>,
    /// Export folder selection index
    pub export_folder_index: usize,
    /// Path to generated preview image
    pub export_preview_path: Option<String>,
    /// Output directory for exported shorts
    pub export_output_dir: Option<String>,
    /// Video path for preview (instead of fallback image)
    pub export_preview_video_path: Option<String>,
}

impl App {
    /// Create new app instance
    pub fn new(output_dir: String) -> Self {
        Self {
            screen: AppScreen::Setup,
            start_time: Instant::now(),
            status: rust_i18n::t!("status_initializing").to_string(),
            logs: Vec::new(),
            security_password_input: String::new(),
            security_confirm_input: String::new(),
            security_selected_mode: 1, // Default to Simple (Recommended)
            security_error: None,

            active_security_mode: crate::security::EncryptionMode::None,
            active_password: None,
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
            language_index: 0,
            editing_setting: false,
            setting_input: String::new(),
            settings_items: Vec::new(),
            api_keys_index: 0,
            cancellation_token: Arc::new(AtomicBool::new(false)),
            export_clip_folders: Vec::new(),
            export_plano_path: None,
            export_plano: Vec::new(),
            export_folder_index: 0,
            export_preview_path: None,
            export_output_dir: None,
            export_preview_video_path: None,
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
                    kind: SettingType::Directory,
                    description: rust_i18n::t!("desc_output_dir").to_string(),
                },
                SettingItem {
                    name: "Auto Extract".to_string(),
                    key: "auto_extract".to_string(),
                    value: config.extract_shorts_when_finished_moments.to_string(),
                    kind: SettingType::Bool,
                    description: rust_i18n::t!("desc_auto_extract").to_string(),
                },
                SettingItem {
                    name: "Use Cookies".to_string(),
                    key: "use_cookies".to_string(),
                    value: config.use_cookies.to_string(),
                    kind: SettingType::Bool,
                    description: rust_i18n::t!("desc_use_cookies").to_string(),
                },
                SettingItem {
                    name: "Cookies Path".to_string(),
                    key: "cookies_path".to_string(),
                    value: config.cookies_path.clone(),
                    kind: SettingType::Path,
                    description: rust_i18n::t!("desc_cookies_path").to_string(),
                },
                SettingItem {
                    name: "Background Opacity".to_string(),
                    key: "bg_opacity".to_string(),
                    value: config.shorts_config.background_opacity.to_string(),
                    kind: SettingType::Float,
                    description: rust_i18n::t!("desc_bg_opacity").to_string(),
                },
                SettingItem {
                    name: "Main Video Zoom".to_string(),
                    key: "zoom".to_string(),
                    value: config.shorts_config.main_video_zoom.to_string(),
                    kind: SettingType::Float,
                    description: rust_i18n::t!("desc_zoom").to_string(),
                },
                SettingItem {
                    name: "Use Fast Model".to_string(),
                    key: "fast_model".to_string(),
                    value: config.use_fast_model.to_string(),
                    kind: SettingType::Bool,
                    description: "Use faster model (gemini-3-flash)".to_string(),
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

                    "bg_opacity" => {
                        config.shorts_config.background_opacity = val.parse().unwrap_or(0.4)
                    }
                    "zoom" => config.shorts_config.main_video_zoom = val.parse().unwrap_or(0.7),
                    "fast_model" => config.use_fast_model = val.parse().unwrap_or(true),
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
                    } else {
                        self.log(
                            LogLevel::Error,
                            rust_i18n::t!("msg_api_key_invalid").to_string(),
                        );
                        self.confirm_response = None;
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
                                name: rust_i18n::t!(
                                    "default_key_name",
                                    number = config.google_api_keys.len() + 1
                                )
                                .to_string(),
                                enabled: true,
                            });
                            if let Err(e) = config.save() {
                                self.log(LogLevel::Error, format!("Failed to save API key: {}", e));
                            } else {
                                self.log(
                                    LogLevel::Success,
                                    rust_i18n::t!("msg_api_key_saved").to_string(),
                                );
                                self.screen = AppScreen::ApiKeysManager;
                            }
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
            AppScreen::SecuritySetup => match key {
                KeyCode::Up => {
                    if self.security_selected_mode > 0 {
                        self.security_selected_mode -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.security_selected_mode < 2 {
                        self.security_selected_mode += 1;
                    }
                }
                KeyCode::Char(c) => {
                    // Handle password input if mode 2 is selected
                    if self.security_selected_mode == 2 {
                        self.security_password_input.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if self.security_selected_mode == 2 {
                        self.security_password_input.pop();
                    }
                }
                KeyCode::Enter => {
                    // Apply Security Settings
                    let mode = match self.security_selected_mode {
                        0 => crate::security::EncryptionMode::None,
                        1 => crate::security::EncryptionMode::Simple,
                        2 => crate::security::EncryptionMode::Password,
                        _ => crate::security::EncryptionMode::Simple,
                    };

                    let mut valid = true;
                    let mut password_to_save = None;

                    if let crate::security::EncryptionMode::Password = mode {
                        if self.security_password_input.len() < 4 {
                            self.security_error =
                                Some(rust_i18n::t!("msg_password_too_short").to_string());
                            valid = false;
                        } else {
                            password_to_save = Some(self.security_password_input.clone());
                        }
                    }

                    if valid {
                        if let Some(config) = &mut self.config {
                            config.active_encryption_mode = mode;
                            config.active_password = password_to_save.clone();

                            if let Err(e) = config.save() {
                                self.security_error = Some(
                                    rust_i18n::t!("msg_save_failed", "error" => e.to_string())
                                        .to_string(),
                                );
                            } else {
                                self.active_security_mode = mode;
                                self.active_password = password_to_save;
                                self.screen = AppScreen::MainMenu;
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    self.screen = AppScreen::MainMenu;
                    self.security_password_input.clear();
                    self.security_error = None;
                }
                _ => {}
            },
            AppScreen::PasswordInput => match key {
                KeyCode::Char(c) => {
                    self.security_password_input.push(c);
                }
                KeyCode::Backspace => {
                    self.security_password_input.pop();
                }
                KeyCode::Enter => {
                    // Attempt to decrypt
                    match crate::config::AppConfig::load_with_password(Some(
                        &self.security_password_input,
                    )) {
                        Ok(config) => {
                            self.config = Some(config.clone());
                            self.active_security_mode = config.active_encryption_mode;
                            self.active_password = config.active_password;
                            self.security_error = None;

                            // Check keys again
                            let default_key = "YOUR_API_KEY_HERE";
                            if config.google_api_keys.is_empty()
                                || config
                                    .google_api_keys
                                    .iter()
                                    .any(|k| k.value == default_key)
                            {
                                self.screen = AppScreen::ApiKeyInput;
                            } else {
                                self.screen = AppScreen::MainMenu;
                            }
                        }
                        Err(_) => {
                            self.security_error =
                                Some(rust_i18n::t!("msg_incorrect_password").to_string());
                            self.security_password_input.clear();
                        }
                    }
                }
                KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },

            AppScreen::LanguageMenu => match key {
                KeyCode::Up => {
                    if self.language_index > 0 {
                        self.language_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.language_index < 2 {
                        self.language_index += 1;
                    }
                }
                KeyCode::Enter => {
                    let new_lang = match self.language_index {
                        0 => "en",
                        1 => "es",
                        2 => "ru",
                        _ => "en",
                    };
                    rust_i18n::set_locale(new_lang);
                    if let Some(config) = &mut self.config {
                        config.language = new_lang.to_string();
                        let _ = config.save();
                    }
                    self.screen = AppScreen::MainMenu;
                }
                KeyCode::Esc => {
                    self.screen = AppScreen::MainMenu;
                }
                _ => {}
            },
            AppScreen::MainMenu => match key {
                KeyCode::Up => {
                    if self.menu_index > 0 {
                        self.menu_index -= 1;
                    } else {
                        self.menu_index = 6; // Loop to bottom (7 items: 0-6)
                    }
                }
                KeyCode::Down => {
                    if self.menu_index < 6 {
                        self.menu_index += 1;
                    } else {
                        self.menu_index = 0; // Loop to top
                    }
                }
                KeyCode::Enter => {
                    match self.menu_index {
                        0 => {
                            self.screen = AppScreen::UrlInput;
                            self.input.clear();
                            self.cursor_pos = 0;
                            self.moments.clear();
                        }
                        1 => {
                            // Export Shorts
                            self.screen = AppScreen::ExportShorts;
                            self.export_clip_folders.clear();
                            self.export_folder_index = 0;
                        }
                        2 => {
                            if let Some(config) = &self.config {
                                self.language_index = match config.language.as_str() {
                                    "es" => 1,
                                    "ru" => 2,
                                    _ => 0,
                                };
                            }
                            self.screen = AppScreen::LanguageMenu;
                        }
                        3 => {
                            self.reload_settings_items();
                            self.settings_index = 0;
                            self.screen = AppScreen::SettingsEditor;
                        }
                        4 => {
                            // Security
                            // Initialize input state
                            if let Some(config) = &self.config {
                                let mode_idx = match config.active_encryption_mode {
                                    crate::security::EncryptionMode::None => 0,
                                    crate::security::EncryptionMode::Simple => 1,
                                    crate::security::EncryptionMode::Password => 2,
                                };
                                self.security_selected_mode = mode_idx;
                                self.security_password_input.clear();
                                self.security_error = None;
                            }
                            self.screen = AppScreen::SecuritySetup;
                        }
                        5 => {
                            // API Keys
                            self.screen = AppScreen::ApiKeysManager;
                            self.api_keys_index = 0;
                        }
                        6 => self.should_quit = true, // Exit
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
                            } else if let SettingType::Path = item.kind {
                                // Open file dialog
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    self.setting_input = path.to_string_lossy().to_string();
                                    self.apply_setting();
                                }
                            } else if let SettingType::Directory = item.kind {
                                // Open directory dialog
                                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                    self.setting_input = path.to_string_lossy().to_string();
                                    self.apply_setting();
                                }
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

            AppScreen::ResumePrompt(_) | AppScreen::FormatConfirm | AppScreen::ShortsConfirm(_) => {
                match key {
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
                }
            }
            AppScreen::Processing => match key {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.screen = AppScreen::ProcessingCancelConfirm;
                }
                _ => {}
            },
            AppScreen::ProcessingCancelConfirm => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.cancellation_token.store(true, Ordering::Relaxed);
                    self.screen = AppScreen::Processing; // Go back to processing to wait for task to finish cleanup
                    self.log(LogLevel::Warning, "Cancelling...".to_string());
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.screen = AppScreen::Processing;
                }
                _ => {}
            },
            AppScreen::Done => match key {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                    // Return to main menu instead of quit
                    self.screen = AppScreen::MainMenu;
                    self.moments.clear();
                    self.input.clear();
                    // self.should_quit = true;
                }
                _ => {}
            },
            // Export Shorts Key Handlers
            AppScreen::ExportShorts => match key {
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    // Navigate to folder selection screen
                    self.screen = AppScreen::ExportSelectFolders;
                    self.export_folder_index = 0;
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.screen = AppScreen::ExportSelectPlano;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    // Select video for preview
                    self.log(
                        LogLevel::Info,
                        rust_i18n::t!("export_selecting_output")
                            .to_string()
                            .replace("output folder", "preview video"), // Reuse i18n logic or add new key if needed, for now stick to simple
                    );

                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Video", &["mp4", "mkv", "webm", "mov"])
                        .pick_file()
                    {
                        let path_str = path.to_string_lossy().to_string();
                        self.export_preview_video_path = Some(path_str.clone());
                        self.log(LogLevel::Success, format!("Video preview: {}", path_str));
                    }
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    // Auto-reload plano if loaded from file
                    if let Some(path) = &self.export_plano_path {
                        match crate::exporter::load_plano(path) {
                            Ok(plano) => {
                                self.export_plano = plano;
                                self.log(
                                    LogLevel::Info,
                                    rust_i18n::t!("export_plano_reloaded", path = path).to_string(),
                                );
                            }
                            Err(e) => {
                                self.log(
                                    LogLevel::Error,
                                    rust_i18n::t!(
                                        "export_plano_reload_error",
                                        error = e.to_string()
                                    )
                                    .to_string(),
                                );
                            }
                        }
                    }

                    // Generate preview
                    if !self.export_plano.is_empty() {
                        self.log(
                            LogLevel::Info,
                            rust_i18n::t!("export_generating_preview").to_string(),
                        );

                        // Use unique filename to force image viewer to open new instance
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let output_dir = std::env::temp_dir();
                        let preview_filename = format!("yt_shortmaker_preview_{}.png", timestamp);
                        let preview_path = output_dir.join(preview_filename);
                        let preview_str = preview_path.to_string_lossy().to_string();

                        let result = if let Some(video_path) = &self.export_preview_video_path {
                            crate::exporter::generate_preview_from_video(
                                video_path,
                                &self.export_plano,
                                &preview_str,
                            )
                        } else {
                            crate::exporter::generate_preview_embedded(
                                &self.export_plano,
                                &preview_str,
                            )
                        };

                        match result {
                            Ok(_) => {
                                self.export_preview_path = Some(preview_str.clone());
                                self.log(
                                    LogLevel::Success,
                                    rust_i18n::t!("export_preview_generated", path = preview_str)
                                        .to_string(),
                                );

                                self.log(
                                    LogLevel::Info,
                                    rust_i18n::t!("export_opening_preview").to_string(),
                                );
                                #[cfg(target_os = "windows")]
                                {
                                    let _ = std::process::Command::new("cmd")
                                        .args(["/C", "start", "", &preview_str])
                                        .spawn();
                                }
                                #[cfg(target_os = "macos")]
                                {
                                    let _ = std::process::Command::new("open")
                                        .arg(&preview_str)
                                        .spawn();
                                }
                                #[cfg(target_os = "linux")]
                                {
                                    let _ = std::process::Command::new("xdg-open")
                                        .arg(&preview_str)
                                        .spawn();
                                }
                            }
                            Err(e) => {
                                self.log(
                                    LogLevel::Error,
                                    rust_i18n::t!("export_preview_error", error = e.to_string())
                                        .to_string(),
                                );
                            }
                        }
                        self.screen = AppScreen::ExportPreview;
                    } else {
                        self.log(
                            LogLevel::Warning,
                            rust_i18n::t!("export_select_template_first").to_string(),
                        );
                    }
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    // Select output directory
                    self.log(
                        LogLevel::Info,
                        rust_i18n::t!("export_selecting_output").to_string(),
                    );
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        let path_str = path.to_string_lossy().to_string();
                        self.export_output_dir = Some(path_str.clone());
                        self.log(
                            LogLevel::Success,
                            rust_i18n::t!("export_output_set", path = path_str).to_string(),
                        );
                    }
                }
                KeyCode::Enter => {
                    // Validate all requirements before starting export
                    if self.export_clip_folders.is_empty() {
                        self.log(
                            LogLevel::Warning,
                            rust_i18n::t!("export_select_clips_first").to_string(),
                        );
                    } else if self.export_plano.is_empty() {
                        self.log(
                            LogLevel::Warning,
                            rust_i18n::t!("export_select_template").to_string(),
                        );
                    } else if let Some(output) = self.export_output_dir.clone() {
                        // All requirements met - start export
                        let num_folders = self.export_clip_folders.len();
                        self.log(
                            LogLevel::Info,
                            rust_i18n::t!("export_starting", count = num_folders).to_string(),
                        );
                        self.log(
                            LogLevel::Info,
                            rust_i18n::t!("export_output_label", path = output).to_string(),
                        );
                        self.screen = AppScreen::ExportProcessing;
                    } else {
                        self.log(
                            LogLevel::Warning,
                            rust_i18n::t!("export_select_output_first").to_string(),
                        );
                    }
                }
                KeyCode::Esc => {
                    self.screen = AppScreen::MainMenu;
                }
                _ => {}
            },
            AppScreen::ExportSelectFolders => match key {
                KeyCode::Up => {
                    if self.export_folder_index > 0 {
                        self.export_folder_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if !self.export_clip_folders.is_empty()
                        && self.export_folder_index < self.export_clip_folders.len() - 1
                    {
                        self.export_folder_index += 1;
                    }
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.export_clip_folders
                            .push(path.to_string_lossy().to_string());
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if !self.export_clip_folders.is_empty() {
                        self.export_clip_folders.remove(self.export_folder_index);
                        if self.export_folder_index > 0
                            && self.export_folder_index >= self.export_clip_folders.len()
                        {
                            self.export_folder_index -= 1;
                        }
                    }
                }
                KeyCode::Enter | KeyCode::Esc => {
                    self.screen = AppScreen::ExportShorts;
                }
                _ => {}
            },
            AppScreen::ExportSelectPlano => match key {
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    // Load existing plano file
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    {
                        let path_str = path.to_string_lossy().to_string();
                        match crate::exporter::load_plano(&path_str) {
                            Ok(plano) => {
                                self.export_plano_path = Some(path_str);
                                self.export_plano = plano;
                                self.log(LogLevel::Success, "Plantilla cargada".to_string());
                            }
                            Err(e) => {
                                self.log(
                                    LogLevel::Error,
                                    format!("Error cargando plantilla: {}", e),
                                );
                            }
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    // Create new default plano
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name("plano.json")
                        .save_file()
                    {
                        let path_str = path.to_string_lossy().to_string();
                        let default_plano = crate::exporter::create_default_plano();
                        if let Err(e) = crate::exporter::save_plano(&path_str, &default_plano) {
                            self.log(LogLevel::Error, format!("Error guardando plantilla: {}", e));
                        } else {
                            self.export_plano_path = Some(path_str);
                            self.export_plano = default_plano;
                            self.log(LogLevel::Success, "Plantilla creada".to_string());
                        }
                    }
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    // Open in external editor
                    if let Some(ref path) = self.export_plano_path {
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("notepad").arg(path).spawn();
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
                        }
                    }
                }
                KeyCode::Esc => {
                    self.screen = AppScreen::ExportShorts;
                }
                _ => {}
            },
            AppScreen::ExportPreview => match key {
                KeyCode::Char('g') | KeyCode::Char('G') => {
                    // Auto-reload plano if loaded from file (Same logic as 'V' had)
                    if let Some(path) = &self.export_plano_path {
                        match crate::exporter::load_plano(path) {
                            Ok(plano) => {
                                self.export_plano = plano;
                                self.log(
                                    LogLevel::Info,
                                    rust_i18n::t!("export_plano_reloaded", path = path).to_string(),
                                );
                            }
                            Err(e) => {
                                self.log(
                                    LogLevel::Error,
                                    rust_i18n::t!(
                                        "export_plano_reload_error",
                                        error = e.to_string()
                                    )
                                    .to_string(),
                                );
                            }
                        }
                    }

                    // Generate preview
                    if !self.export_plano.is_empty() {
                        self.log(
                            LogLevel::Info,
                            rust_i18n::t!("export_generating_preview").to_string(),
                        );

                        // Use unique filename to force image viewer to open new instance
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let output_dir = std::env::temp_dir();
                        let preview_filename = format!("yt_shortmaker_preview_{}.png", timestamp);
                        let preview_path = output_dir.join(preview_filename);
                        let preview_str = preview_path.to_string_lossy().to_string();

                        let result = if let Some(video_path) = &self.export_preview_video_path {
                            crate::exporter::generate_preview_from_video(
                                video_path,
                                &self.export_plano,
                                &preview_str,
                            )
                        } else {
                            crate::exporter::generate_preview_embedded(
                                &self.export_plano,
                                &preview_str,
                            )
                        };

                        match result {
                            Ok(_) => {
                                self.export_preview_path = Some(preview_str.clone());
                                self.log(
                                    LogLevel::Success,
                                    rust_i18n::t!("export_preview_generated", path = preview_str)
                                        .to_string(),
                                );

                                // Open the preview
                                self.log(
                                    LogLevel::Info,
                                    rust_i18n::t!("export_opening_preview").to_string(),
                                );
                                if let Err(e) = open::that(&preview_str) {
                                    self.log(
                                        LogLevel::Error,
                                        format!("Failed to open preview: {}", e),
                                    );
                                }
                            }
                            Err(e) => {
                                self.log(
                                    LogLevel::Error,
                                    rust_i18n::t!("export_preview_error", error = e.to_string())
                                        .to_string(),
                                );
                            }
                        }
                    } else {
                        self.log(
                            LogLevel::Warning,
                            rust_i18n::t!("export_select_template_first").to_string(),
                        );
                    }
                }
                KeyCode::Enter | KeyCode::Esc => {
                    self.screen = AppScreen::ExportShorts;
                }
                _ => {}
            },
            AppScreen::ExportProcessing => {
                if let KeyCode::Esc = key {
                    self.screen = AppScreen::ExportProcessingCancellationConfirm;
                }
            }
            AppScreen::ExportProcessingCancellationConfirm => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    // Confirm cancellation
                    self.cancellation_token.store(true, Ordering::Relaxed);
                    self.log(
                        LogLevel::Warning,
                        rust_i18n::t!("export_cancelling_log").to_string(),
                    );
                    // We don't change screen here immediately to allow logs to show cancellation progress
                    self.screen = AppScreen::ExportProcessing;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    // Abort cancellation
                    self.screen = AppScreen::ExportProcessing;
                }
                _ => {}
            },
            _ => {
                if key == KeyCode::Esc || key == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
        }

        // Handle keys for ExportDone (same as Done)
        if let AppScreen::ExportDone = self.screen {
            match key {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                    self.screen = AppScreen::MainMenu;
                    self.export_plano_path = None;
                }
                _ => {}
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
                self.status = "Error".to_string(); // Update status for visibility
                self.has_error = true;
                self.result_message = Some(msg);
            }

            AppMessage::RequestShortsConfirm(count) => {
                self.screen = AppScreen::ShortsConfirm(count);
                self.confirm_response = None;
            }

            AppMessage::Finished => {
                if self.screen == AppScreen::ExportProcessing
                    || self.screen == AppScreen::ExportProcessingCancellationConfirm
                {
                    self.screen = AppScreen::ExportDone;
                } else {
                    self.screen = AppScreen::Done;
                }
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
            Span::raw(format!("📁 {}", rust_i18n::t!("header_output"))),
            Span::styled(&app.output_dir, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw(format!("⚡ {}", rust_i18n::t!("header_status"))),
            Span::styled(&app.status, Style::default().fg(Color::Green)),
        ]),
    ]);
    frame.render_widget(Paragraph::new(left_text), header_layout[0]);

    // Right side: Uptime and moments count
    let right_text = Text::from(vec![
        Line::from(vec![
            Span::raw(format!("⏱  {}", rust_i18n::t!("header_uptime"))),
            Span::styled(app.uptime(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw(format!("🎬 {}", rust_i18n::t!("header_moments"))),
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

        AppScreen::Done => render_done(frame, app, area),
        AppScreen::ApiKeysManager => render_api_keys_manager(frame, app, area),
        AppScreen::ApiKeyAddInput => render_api_key_add_input(frame, app, area),
        AppScreen::ApiKeyRename => render_api_key_rename(frame, app, area),
        AppScreen::SecuritySetup => render_security_setup(frame, app, area),
        AppScreen::PasswordInput => render_password_input(frame, app, area),
        AppScreen::LanguageMenu => render_language_menu(frame, app, area),
        AppScreen::ProcessingCancelConfirm => render_processing_cancel_confirm(frame, area),
        AppScreen::ExportShorts => render_export_shorts(frame, app, area),
        AppScreen::ExportSelectFolders => render_export_select_folders(frame, app, area),
        AppScreen::ExportSelectPlano => render_export_select_plano(frame, app, area),
        AppScreen::ExportPreview => render_export_preview(frame, app, area),
        AppScreen::ExportProcessing => render_export_processing(frame, app, area),
        AppScreen::ExportProcessingCancellationConfirm => {
            render_export_processing(frame, app, area); // Render background
            render_export_processing_cancel_confirm(frame, app, area); // Render popup overlay
        }
        AppScreen::ExportDone => render_export_done(frame, app, area),
    }
}

fn render_export_done(frame: &mut Frame, app: &App, area: Rect) {
    let (title, border_color) = if app.has_error {
        (rust_i18n::t!("proc_failed").to_string(), Color::Red)
    } else {
        (rust_i18n::t!("proc_complete").to_string(), Color::Green)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let (msg, msg_color) = if app.has_error {
        (rust_i18n::t!("done_msg_fail").to_string(), Color::Red)
    } else {
        (rust_i18n::t!("done_msg_success").to_string(), Color::Green)
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

    lines.push(Line::from(rust_i18n::t!("done_return")));

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}

fn render_export_processing(frame: &mut Frame, app: &App, area: Rect) {
    render_processing(frame, app, area)
}

fn render_export_processing_cancel_confirm(frame: &mut Frame, _app: &App, area: Rect) {
    // Use fixed dimensions for better readability on small screens
    let width = 60.min(area.width);
    let height = 12.min(area.height);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    let block = Block::default()
        .title(format!(
            " {} ",
            rust_i18n::t!("export_cancel_confirm_title")
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(Color::Reset));

    let text = vec![
        Line::from(vec![Span::styled(
            rust_i18n::t!("export_cancel_confirm_message").to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(rust_i18n::t!("export_cancel_confirm_submessage").to_string()),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                rust_i18n::t!("export_cancel_yes").to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                rust_i18n::t!("export_cancel_no").to_string(),
                Style::default().fg(Color::Green),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(paragraph, popup_area);
}

fn render_api_keys_manager(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {} ", rust_i18n::t!("keys_title")));

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

    let help = Paragraph::new(rust_i18n::t!("keys_help").to_string())
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(help, layout[1]);
}

fn render_api_key_rename(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" {} ", rust_i18n::t!("keys_rename_title")));

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

    let help = Paragraph::new(rust_i18n::t!("keys_rename_help").to_string())
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(help, layout[2]);

    // Cursor
    frame.set_cursor_position((layout[1].x + 1 + app.cursor_pos as u16, layout[1].y + 1));
}

fn render_api_key_add_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(format!(" {} ", rust_i18n::t!("keys_add_title")));

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

    let help = Paragraph::new(rust_i18n::t!("keys_add_help").to_string())
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(help, layout[2]);

    // Cursor
    frame.set_cursor_position((layout[1].x + 1 + app.cursor_pos as u16, layout[1].y + 1));
}

fn render_apikey_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(format!(" {} ", rust_i18n::t!("setup_apikey_title")));

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
    let instructions = Paragraph::new(rust_i18n::t!("setup_apikey_welcome"))
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
    let note =
        Paragraph::new(rust_i18n::t!("setup_apikey_instr")).style(Style::default().fg(Color::Gray));
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
        .title(format!(" {} ", rust_i18n::t!("main_menu_title")));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Dynamic localization for options
    let options = [
        rust_i18n::t!("menu_start"),
        rust_i18n::t!("menu_export_shorts"),
        rust_i18n::t!("language"),
        rust_i18n::t!("menu_settings"),
        rust_i18n::t!("menu_security"),
        rust_i18n::t!("menu_keys"),
        rust_i18n::t!("menu_exit"),
    ];

    let list_area = Rect {
        x: area.width / 2 - 15,
        y: area.height / 2 - 8, // Adjusted for extra item
        width: 30,
        height: 16, // Adjusted for extra item (7 items)
    };

    // Ensure we don't go out of bounds if terminal is small
    let list_area = list_area.intersection(inner_area);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, text)| {
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
            .title(format!(" {} ", rust_i18n::t!("select_option"))),
    );

    frame.render_widget(list, list_area);
}

fn render_settings_editor(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(format!(" {} ", rust_i18n::t!("settings_title")));

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

            // Add hint for Path/Directory
            if is_selected {
                match item.kind {
                    SettingType::Path | SettingType::Directory => {
                        let hint = Span::styled(
                            " (Press Enter to browse)",
                            Style::default().fg(Color::DarkGray),
                        );
                        // Reconstruct line to add hint
                        ListItem::new(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(val_color)),
                            Span::styled(
                                format!("{:<20}: ", item.name),
                                Style::default().fg(key_color),
                            ),
                            Span::styled(&item.value, Style::default().fg(val_color)),
                            hint,
                        ]))
                    }
                    _ => ListItem::new(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(val_color)),
                        Span::styled(
                            format!("{:<20}: ", item.name),
                            Style::default().fg(key_color),
                        ),
                        Span::styled(&item.value, Style::default().fg(val_color)),
                    ])),
                }
            } else {
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(val_color)),
                    Span::styled(
                        format!("{:<20}: ", item.name),
                        Style::default().fg(key_color),
                    ),
                    Span::styled(&item.value, Style::default().fg(val_color)),
                ]))
            }
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
            .title(format!(" {} ", rust_i18n::t!("settings_editing")));

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

fn render_setup(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", rust_i18n::t!("setup_title")));

    let text = Paragraph::new(rust_i18n::t!("setup_loading"))
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(text, area);
}

fn render_resume_prompt(frame: &mut Frame, url: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" 📋 {} ", rust_i18n::t!("resume_title")));

    let text = Text::from(vec![
        Line::from(""),
        Line::from(vec![
            Span::raw(rust_i18n::t!("resume_found")),
            Span::styled(url, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(rust_i18n::t!("resume_ask")).style(Style::default().fg(Color::Yellow)),
    ]);

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_url_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" 🎬 {} ", rust_i18n::t!("url_title")));

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
    let instructions =
        Paragraph::new(rust_i18n::t!("url_instr")).style(Style::default().fg(Color::Gray));
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
        .title(format!(" {} ", rust_i18n::t!("format_title")));

    let text = Text::from(vec![
        Line::from(""),
        Line::from(""),
        Line::from(rust_i18n::t!("format_msg")),
        Line::from(""),
        Line::from(rust_i18n::t!("format_yes")),
        Line::from(rust_i18n::t!("format_no")),
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
                (rust_i18n::t!("proc_failed").to_string(), Color::Red)
            } else {
                (rust_i18n::t!("proc_complete").to_string(), Color::Green)
            }
        }
        _ => (rust_i18n::t!("proc_running").to_string(), Color::Cyan),
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
        .title(format!(" {} ", rust_i18n::t!("log_title")));

    let log_height = layout[1].height.saturating_sub(2);

    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(log_height as usize)
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
    let moments_block = Block::default().borders(Borders::ALL).title(format!(
        " {} ",
        rust_i18n::t!("moments_found_title", count = app.moments.len())
    ));

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
        .title(format!(" {} ", rust_i18n::t!("analysis_complete")));

    let text = Text::from(vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            rust_i18n::t!("analysis_found_msg", count = count),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(rust_i18n::t!("analysis_ask")),
        Line::from(""),
        Line::from(rust_i18n::t!("analysis_yes")),
        Line::from(rust_i18n::t!("analysis_no")),
        Line::from(""),
    ]);

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_done(frame: &mut Frame, app: &App, area: Rect) {
    let (title, border_color) = if app.has_error {
        (rust_i18n::t!("proc_failed").to_string(), Color::Red)
    } else {
        (rust_i18n::t!("proc_complete").to_string(), Color::Green)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let (msg, msg_color) = if app.has_error {
        (rust_i18n::t!("done_msg_fail").to_string(), Color::Red)
    } else {
        (rust_i18n::t!("done_msg_success").to_string(), Color::Green)
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
        Span::raw(rust_i18n::t!("done_total_moments")),
        Span::styled(
            app.moments.len().to_string(),
            Style::default().fg(Color::Cyan),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(rust_i18n::t!("done_return")));

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}

/// Render the footer with keyboard shortcuts
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match &app.screen {
        AppScreen::MainMenu => rust_i18n::t!("shortcuts_main"),
        AppScreen::SettingsEditor => {
            if app.editing_setting {
                rust_i18n::t!("shortcuts_settings_edit")
            } else {
                rust_i18n::t!("shortcuts_settings_nav")
            }
        }
        AppScreen::UrlInput => rust_i18n::t!("shortcuts_url"),
        AppScreen::ResumePrompt(_) | AppScreen::FormatConfirm | AppScreen::ShortsConfirm(_) => {
            rust_i18n::t!("shortcuts_confirm")
        }
        AppScreen::Processing => rust_i18n::t!("shortcuts_process"),
        AppScreen::Done => rust_i18n::t!("shortcuts_done"),
        _ => rust_i18n::t!("shortcuts_default"),
    };

    let footer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" {} ", rust_i18n::t!("shortcuts_footer_title")));

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

fn render_security_setup(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {} ", rust_i18n::t!("security_title")));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(4), // Intro text
                Constraint::Length(3), // Options header
                Constraint::Min(5),    // Options list
                Constraint::Length(3), // Description
                Constraint::Length(3), // Confirm button/hint
            ]
            .as_ref(),
        )
        .split(inner);

    let intro_text = Paragraph::new(rust_i18n::t!("security_intro"))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(intro_text, chunks[0]);

    let modes = [
        rust_i18n::t!("security_mode_none"),
        rust_i18n::t!("security_mode_simple"),
        rust_i18n::t!("security_mode_pass"),
    ];
    let mode_descriptions = [
        rust_i18n::t!("security_desc_none"),
        rust_i18n::t!("security_desc_simple"),
        rust_i18n::t!("security_desc_pass"),
    ];

    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let style = if i == app.security_selected_mode {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            ListItem::new(format!(" {} ", text)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", rust_i18n::t!("security_modes_title"))),
    );
    frame.render_widget(list, chunks[2]);

    // Description box
    let desc_text = mode_descriptions
        .get(app.security_selected_mode)
        .unwrap_or(&std::borrow::Cow::Borrowed(""));
    let desc = Paragraph::new(rust_i18n::t!("security_detail_prefix", "desc" => desc_text))
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(desc, chunks[3]);

    if app.security_selected_mode == 2 {
        let pass_text = rust_i18n::t!("security_pass_label", "mask" => "*".repeat(app.security_password_input.len()));
        let confirm_text = rust_i18n::t!("security_confirm_label", "mask" => "*".repeat(app.security_confirm_input.len()));

        let input_text = format!("{}  |  {}", pass_text, confirm_text);

        let input = Paragraph::new(input_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", rust_i18n::t!("security_set_pass_title"))),
        );
        frame.render_widget(input, chunks[4]);
    } else {
        let help = Paragraph::new(rust_i18n::t!("security_confirm_help"))
            .style(Style::default().add_modifier(Modifier::ITALIC))
            .alignment(Alignment::Center);
        frame.render_widget(help, chunks[4]);
    }
}

fn render_password_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(format!(" {} ", rust_i18n::t!("password_req_title")));

    let area = centered_rect(50, 40, area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3), // Label
                Constraint::Length(3), // Input
                Constraint::Length(3), // Error
            ]
            .as_ref(),
        )
        .split(area);

    let label = Paragraph::new(rust_i18n::t!("password_req_label"))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(label, chunks[0]);

    let pass_text = "*".repeat(app.security_password_input.len());
    let input = Paragraph::new(pass_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", rust_i18n::t!("password_input_title"))),
        );
    frame.render_widget(input, chunks[1]);

    if let Some(err) = &app.security_error {
        let err_text = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        frame.render_widget(err_text, chunks[2]);
    }
}

fn render_language_menu(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" {} ", rust_i18n::t!("language")));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let options = ["English", "Español", "Русский"];

    let width = 40;
    let height = 10;

    let list_area = Rect {
        x: area.width.saturating_sub(width) / 2,
        y: area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    let list_area = list_area.intersection(inner_area);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, &text)| {
            let style = if i == app.language_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let content = format!(" {:^36} ", text);
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", rust_i18n::t!("select_option"))),
    );

    frame.render_widget(list, list_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn render_processing_cancel_confirm(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(format!(" 🛑 {} ", rust_i18n::t!("cancel_title")));

    let area = centered_rect(60, 60, area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Msg
                Constraint::Min(4),    // Warning
                Constraint::Length(4), // Options
            ]
            .as_ref(),
        )
        .split(area);

    let msg = Paragraph::new(rust_i18n::t!("cancel_msg"))
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(msg, chunks[0]);

    let warn = Paragraph::new(rust_i18n::t!("cancel_warn"))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(warn, chunks[1]);

    let options = Text::from(vec![
        Line::from(rust_i18n::t!("cancel_instr")).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from(rust_i18n::t!("cancel_yes")).style(Style::default().fg(Color::Red)),
        Line::from(rust_i18n::t!("cancel_no")).style(Style::default().fg(Color::Green)),
    ]);

    let opts = Paragraph::new(options).alignment(Alignment::Center);
    frame.render_widget(opts, chunks[2]);
}

// ============================================================================
// Export Shorts Screens
// ============================================================================

fn render_export_shorts(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" 📤 {} ", rust_i18n::t!("export_title")));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Folders info
            Constraint::Length(3), // Plano info
            Constraint::Length(3), // Output folder info
            Constraint::Min(6),    // Instructions
        ])
        .split(inner_area);

    // Title
    let title = Paragraph::new(rust_i18n::t!("export_select_folders"))
        .style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    // Folders count
    let folders_text = format!(
        "{}: {}",
        rust_i18n::t!("export_folders_count"),
        app.export_clip_folders.len()
    );
    let folders = Paragraph::new(folders_text)
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center);
    frame.render_widget(folders, chunks[1]);

    // Plano status
    let plano_text = match &app.export_plano_path {
        Some(path) => format!("📋 {}", path),
        None => rust_i18n::t!("export_no_plano").to_string(),
    };
    let plano = Paragraph::new(plano_text)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);
    frame.render_widget(plano, chunks[2]);

    // Output folder status
    let output_text = match &app.export_output_dir {
        Some(path) => rust_i18n::t!("export_output_selected", path = path).to_string(),
        None => rust_i18n::t!("export_output_not_selected").to_string(),
    };
    let output = Paragraph::new(output_text)
        .style(Style::default().fg(if app.export_output_dir.is_some() {
            Color::Green
        } else {
            Color::Red
        }))
        .alignment(Alignment::Center);
    frame.render_widget(output, chunks[3]);

    // Instructions
    let instructions = Text::from(vec![
        Line::from(vec![
            Span::styled("[F] ", Style::default().fg(Color::Cyan)),
            Span::raw(rust_i18n::t!("export_add_folder")),
        ]),
        Line::from(vec![
            Span::styled("[P] ", Style::default().fg(Color::Cyan)),
            Span::raw(rust_i18n::t!("export_select_plano")),
        ]),
        Line::from(vec![
            Span::styled("[O] ", Style::default().fg(Color::Cyan)),
            Span::raw(rust_i18n::t!("export_output_dir")),
        ]),
        Line::from(vec![
            Span::styled("[T] ", Style::default().fg(Color::Magenta)),
            Span::raw(rust_i18n::t!("export_select_preview_video")),
        ]),
        Line::from(vec![
            Span::styled("[V] ", Style::default().fg(Color::Cyan)),
            Span::raw(rust_i18n::t!("export_preview")),
        ]),
        Line::from(vec![
            Span::styled("[Enter] ", Style::default().fg(Color::Green)),
            Span::raw(rust_i18n::t!("export_start")),
        ]),
        Line::from(vec![
            Span::styled("[Esc] ", Style::default().fg(Color::Red)),
            Span::raw(rust_i18n::t!("back")),
        ]),
    ]);
    let instr = Paragraph::new(instructions)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(instr, chunks[4]);
}

fn render_export_select_folders(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" 📁 {} ", rust_i18n::t!("export_select_folders")));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(10),   // List of folders
            Constraint::Length(4), // Help
        ])
        .split(inner_area);

    // List of folders
    let items: Vec<ListItem> = app
        .export_clip_folders
        .iter()
        .enumerate()
        .map(|(i, folder)| {
            let style = if i == app.export_folder_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!(" 📂 {} ", folder)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", rust_i18n::t!("export_folders_title"))),
    );
    frame.render_widget(list, chunks[0]);

    // Help
    let help = Paragraph::new(rust_i18n::t!("export_folders_help"))
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[1]);
}

fn render_export_select_plano(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" 📋 {} ", rust_i18n::t!("export_select_plano")));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(4), // Current plano
            Constraint::Min(6),    // Options
            Constraint::Length(3), // Help
        ])
        .split(inner_area);

    // Current plano
    let current = match &app.export_plano_path {
        Some(path) => rust_i18n::t!("export_plano_current", path = path).to_string(),
        None => rust_i18n::t!("export_plano_none").to_string(),
    };
    let current_para = Paragraph::new(current)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    frame.render_widget(current_para, chunks[0]);

    // Options
    let options = Text::from(vec![
        Line::from(vec![
            Span::styled("[L] ", Style::default().fg(Color::Cyan)),
            Span::raw(rust_i18n::t!("export_plano_opt_load")),
        ]),
        Line::from(vec![
            Span::styled("[N] ", Style::default().fg(Color::Green)),
            Span::raw(rust_i18n::t!("export_plano_opt_new")),
        ]),
        Line::from(vec![
            Span::styled("[E] ", Style::default().fg(Color::Yellow)),
            Span::raw(rust_i18n::t!("export_plano_opt_edit")),
        ]),
    ]);
    let opts = Paragraph::new(options)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(opts, chunks[1]);

    // Help
    let help = Paragraph::new(rust_i18n::t!("export_plano_help"))
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(help, chunks[2]);
}

fn render_export_preview(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(format!(" 👁 {} ", rust_i18n::t!("export_preview_title")));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Status
            Constraint::Min(10),   // Plano layers
            Constraint::Length(3), // Actions
        ])
        .split(inner_area);

    // Status
    let status = if app.export_plano.is_empty() {
        rust_i18n::t!("export_preview_status_none").to_string()
    } else {
        rust_i18n::t!("export_preview_status_ok", count = app.export_plano.len()).to_string()
    };
    let status_para = Paragraph::new(status)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    frame.render_widget(status_para, chunks[0]);

    // Plano layers list
    let items: Vec<ListItem> = app
        .export_plano
        .iter()
        .enumerate()
        .map(|(i, obj)| {
            let (icon, name) = match obj {
                crate::exporter::PlanoObject::Clip { .. } => ("🎬", "Clip"),
                crate::exporter::PlanoObject::Image { path, .. } => ("🖼️", path.as_str()),
                crate::exporter::PlanoObject::Shader { .. } => ("✨", "Shader"),
                crate::exporter::PlanoObject::Video { path, .. } => ("📹", path.as_str()),
            };
            ListItem::new(format!(
                " {} Capa {}: {} - {} ",
                icon,
                i,
                match obj {
                    crate::exporter::PlanoObject::Clip { .. } =>
                        rust_i18n::t!("export_preview_layer_clip"),
                    crate::exporter::PlanoObject::Image { .. } =>
                        rust_i18n::t!("export_preview_layer_image"),
                    crate::exporter::PlanoObject::Shader { .. } =>
                        rust_i18n::t!("export_preview_layer_shader"),
                    crate::exporter::PlanoObject::Video { .. } =>
                        rust_i18n::t!("export_preview_layer_video"),
                },
                name
            ))
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
        " {} ",
        rust_i18n::t!("export_preview_layers_title")
    )));
    frame.render_widget(list, chunks[1]);

    // Actions
    let help_text = format!(
        "{}  [Enter] Volver  [Esc] Volver",
        rust_i18n::t!("export_preview_help_generate")
    );
    let actions = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(actions, chunks[2]);
}
