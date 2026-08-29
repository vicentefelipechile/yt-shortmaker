// =================================================================================================
// app::main — Native egui entry point (M0 POC)
// =================================================================================================

mod actions;
mod state;
mod ui;

use eframe::egui;
use tracing_subscriber::EnvFilter;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

struct App {
    counter: usize,
    core_version: &'static str,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Basic theming hook — keep for future.
        let _ = cc;
        Self {
            counter: 0,
            core_version: yt_shortmaker_core::version(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("yt-shortmaker v2");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("core {}", self.core_version));
                });
            });
        });

        egui::Panel::left("side").show(ui, |ui| {
            ui.heading("Home");
            ui.separator();
            for item in ["Home", "New Project", "Export Standalone", "Settings", "Keys"] {
                if ui.selectable_label(false, item).clicked() {
                    // TODO(M3): routing
                }
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("M0 — POC window");
            ui.label("If you see this, eframe + egui are wired correctly.");
            ui.add_space(12.0);
            if ui.button(format!("Clicked {} times", self.counter)).clicked() {
                self.counter += 1;
            }
            ui.add_space(12.0);
            ui.label("Next: Core crate + pipeline + Analyze/Review flow.");
        });
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1024.0, 700.0])
        .with_min_inner_size([900.0, 600.0]);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "yt-shortmaker v2",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
