//! Dashboard UI module for AutoShorts-Rust-CLI
//! Provides visual feedback with a clean, non-verbose interface

use console::{style, Term};
use std::io::Write;

use crate::types::{APP_NAME, APP_VERSION};

/// Dashboard manager for terminal UI
/// Uses a simpler approach that doesn't conflict with external command output
pub struct Dashboard {
    term: Term,
    start_time: std::time::Instant,
}

impl Dashboard {
    /// Initialize the dashboard with header
    pub fn init(output_dir: &str) -> Self {
        let term = Term::stdout();
        let _ = term.clear_screen();

        // Print Static Header
        println!(
            "{}",
            style("============================================")
                .cyan()
                .bold()
        );
        println!("   {} v{}", style(APP_NAME).magenta().bold(), APP_VERSION);
        println!("   Target: {}", style(output_dir).yellow());
        println!("   Status: {}", style("Active").green());
        println!(
            "{}",
            style("============================================")
                .cyan()
                .bold()
        );
        println!();

        Dashboard {
            term,
            start_time: std::time::Instant::now(),
        }
    }

    /// Get formatted uptime
    fn get_uptime(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let secs = elapsed.as_secs();
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }

    /// Update the status message (clears line and rewrites)
    pub fn set_status(&self, message: &str) {
        let status_line = format!(
            "⏱  [{}] {}",
            style(self.get_uptime()).dim(),
            style(message).cyan()
        );

        // Clear current line and print status
        let _ = self.term.clear_line();
        print!("\r{}", status_line);
        let _ = std::io::stdout().flush();
    }

    /// Mark task as successful
    pub fn success(&self, message: &str) {
        let _ = self.term.clear_line();
        println!(
            "\r✔  [{}] {}",
            style(self.get_uptime()).dim(),
            style(message).green()
        );
    }

    /// Mark task as failed
    pub fn error(&self, message: &str) {
        let _ = self.term.clear_line();
        println!(
            "\r✘  [{}] {}",
            style(self.get_uptime()).dim(),
            style(message).red()
        );
    }

    /// Print an info message
    pub fn info(&self, message: &str) {
        let _ = self.term.clear_line();
        println!("\r   ℹ️  {}", style(message).dim());
    }

    /// Print a warning message
    pub fn warn(&self, message: &str) {
        let _ = self.term.clear_line();
        println!("\r   ⚠️  {}", style(message).yellow());
    }
}
