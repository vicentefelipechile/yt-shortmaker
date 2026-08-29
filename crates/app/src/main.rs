// =================================================================================================
// app::main — Native egui entry point
// =================================================================================================

mod actions;
mod state;
mod ui;

use eframe::egui;
use tracing_subscriber::EnvFilter;

use state::{AppState, Route};
use yt_shortmaker_core::config::store as config_store;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

struct App {
    state: AppState,
    core_version: &'static str,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = config_store::load().unwrap_or_default();
        Self {
            state: AppState::new(config),
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
            ui.heading("Navigate");
            ui.separator();
            for (label, route) in [
                ("Home", Route::Home),
                ("New Project", Route::NewProject),
                ("Export Standalone", Route::ExportStandalone),
                ("Settings", Route::Settings),
                ("Keys", Route::Keys),
            ] {
                let selected = self.state.route == route;
                if ui.selectable_label(selected, label).clicked() {
                    self.state.navigate(route);
                }
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui::show_central(ui, &mut self.state);
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
