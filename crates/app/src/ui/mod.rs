// =================================================================================================
// app::ui — Central routing and views
// =================================================================================================

pub mod components;

use eframe::egui;

use crate::state::{AppState, Route};

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn show_central(ui: &mut egui::Ui, state: &mut AppState) {
    match state.route {
        Route::Home => show_home(ui, state),
        Route::NewProject => show_new_project(ui, state),
        Route::ExportStandalone => show_export_standalone(ui, state),
        Route::Settings => show_settings(ui, state),
        Route::Keys => show_keys(ui, state),
    }
}

fn show_home(ui: &mut egui::Ui, _state: &mut AppState) {
    ui.heading("Recent projects");
    ui.separator();
    ui.label("No projects yet. Create a new project to start.");
}

fn show_new_project(ui: &mut egui::Ui, _state: &mut AppState) {
    ui.heading("New Project");
    ui.separator();
    ui.label("Paste a YouTube URL to begin.");
}

fn show_export_standalone(ui: &mut egui::Ui, _state: &mut AppState) {
    ui.heading("Export Standalone");
    ui.separator();
    ui.label("Select clips and a plano to export without AI.");
}

fn show_settings(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Settings");
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Language:");
        egui::ComboBox::from_id_salt("lang")
            .selected_text(&state.config.language)
            .show_ui(ui, |ui| {
                for lang in ["en", "es", "ru"] {
                    if ui.selectable_label(state.config.language == lang, lang).clicked() {
                        state.config.language = lang.to_owned();
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Output dir:");
        let mut dir = state
            .config
            .output_dir
            .clone()
            .unwrap_or_default();
        if ui.text_edit_singleline(&mut dir).changed() {
            state.config.output_dir = Some(dir);
        }
        if ui.button("Browse").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                state.config.output_dir = Some(path.to_string_lossy().to_string());
            }
        }
    });
    if ui.button("Save").clicked() {
        match yt_shortmaker_core::config::store::save(&state.config) {
            Ok(()) => state.status_msg = Some("Settings saved".into()),
            Err(e) => state.status_msg = Some(format!("Save failed: {e}")),
        }
    }
    if let Some(msg) = &state.status_msg {
        ui.label(msg);
    }
}

fn show_keys(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("API Keys");
    ui.separator();
    let active = state.config.ai.active_provider.clone();
    ui.label(format!("Active provider: {active}"));
    if let Some(provider) = state.config.ai.providers.get(&active) {
        if provider.keys.is_empty() {
            ui.label("No keys configured.");
        }
        for key in &provider.keys {
            ui.horizontal(|ui| {
                ui.label(&key.name);
                ui.label(if key.enabled { "enabled" } else { "disabled" });
            });
        }
    }
    if ui.button("Add key").clicked() {
        // Placeholder — full CRUD in M4
        state.status_msg = Some("Key management expands in next milestone".into());
    }
}
