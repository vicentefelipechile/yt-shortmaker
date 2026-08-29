// =================================================================================================
// app::ui — Central routing and views
// =================================================================================================

pub mod components;

use eframe::egui;

use crate::state::{AppState, NewProjectStep, Route};
use yt_shortmaker_core::media::ytdlp;
use yt_shortmaker_core::plano::schema::{create_default_plano, PlanoObject};

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

fn show_new_project(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("New Project");
    ui.separator();

    ui.horizontal(|ui| {
        for (label, step) in [
            ("1 Ingest", NewProjectStep::Ingest),
            ("2 Analyze", NewProjectStep::Analyze),
            ("3 Review", NewProjectStep::Review),
            ("4 Export", NewProjectStep::Export),
        ] {
            let selected = state.new_project_step == step;
            if ui.selectable_label(selected, label).clicked() {
                state.new_project_step = step;
            }
        }
    });
    ui.separator();

    match state.new_project_step {
        NewProjectStep::Ingest => show_ingest(ui, state),
        NewProjectStep::Analyze => show_analyze(ui, state),
        NewProjectStep::Review => show_review(ui, state),
        NewProjectStep::Export => show_export(ui, state),
    }
}

fn show_ingest(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("Paste a YouTube URL:");
    ui.text_edit_singleline(&mut state.url_input);
    if let Some(err) = &state.url_error {
        ui.colored_label(egui::Color32::RED, err);
    }
    if let Some(title) = &state.video_title {
        ui.label(format!("Title: {title}"));
    }
    if ui.button("Fetch Info").clicked() {
        match ytdlp::validate_media_url(&state.url_input) {
            Ok(()) => {
                state.url_error = None;
                state.video_title = Some("Video title (mock)".into());
                state.log.push(format!("Fetched info for {}", state.url_input));
                state.new_project_step = NewProjectStep::Analyze;
            }
            Err(e) => state.url_error = Some(e.to_string()),
        }
    }
}

fn show_analyze(ui: &mut egui::Ui, state: &mut AppState) {
    if state.url_input.is_empty() {
        ui.label("No URL — go back to Ingest.");
        return;
    }
    ui.label(format!("Analyzing: {}", state.url_input));
    ui.add(egui::ProgressBar::new(state.progress).show_percentage());
    for line in &state.log {
        ui.label(line);
    }
    ui.horizontal(|ui| {
        let label = if state.analyzing { "Analyzing..." } else { "Start Analyze" };
        if ui.add_enabled(!state.analyzing, egui::Button::new(label)).clicked() {
            state.analyzing = true;
            state.progress = 0.3;
            state.log.push("Downloading low-res...".into());
            state.moments = vec![
                yt_shortmaker_core::types::VideoMoment {
                    start_time: "00:00:05".into(),
                    end_time: "00:00:15".into(),
                    category: "hook".into(),
                    description: "Strong opening".into(),
                    dialogue: vec![],
                },
                yt_shortmaker_core::types::VideoMoment {
                    start_time: "00:01:20".into(),
                    end_time: "00:01:35".into(),
                    category: "funny".into(),
                    description: "Funny moment".into(),
                    dialogue: vec![],
                },
            ];
            state.progress = 1.0;
            state.analyzing = false;
            state.log.push("Analyze complete".into());
        }
        if ui.button("Cancel").clicked() {
            state.analyzing = false;
            state.progress = 0.0;
        }
        if !state.moments.is_empty() && ui.button("Go to Review").clicked() {
            state.new_project_step = NewProjectStep::Review;
        }
    });
}

fn show_review(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading(format!("Review ({} moments)", state.moments.len()));
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut to_delete: Option<usize> = None;
        for (idx, m) in state.moments.iter_mut().enumerate() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("#{idx}"));
                    ui.text_edit_singleline(&mut m.start_time);
                    ui.label("->");
                    ui.text_edit_singleline(&mut m.end_time);
                    egui::ComboBox::from_id_salt(format!("cat{idx}"))
                        .selected_text(&m.category)
                        .show_ui(ui, |ui| {
                            for cat in ["hook", "funny", "emotional", "other"] {
                                if ui.selectable_label(m.category == cat, cat).clicked() {
                                    m.category = cat.to_owned();
                                }
                            }
                        });
                    if ui.small_button("Delete").clicked() {
                        to_delete = Some(idx);
                    }
                });
                ui.text_edit_multiline(&mut m.description);
            });
        }
        if let Some(idx) = to_delete {
            state.moments.remove(idx);
        }
    });

    ui.horizontal(|ui| {
        if ui.button("Add moment").clicked() {
            state.moments.push(yt_shortmaker_core::types::VideoMoment {
                start_time: "00:00:00".into(),
                end_time: "00:00:10".into(),
                category: "other".into(),
                description: "New moment".into(),
                dialogue: vec![],
            });
        }
        if !state.moments.is_empty() && ui.button("Continue to Export").clicked() {
            state.new_project_step = NewProjectStep::Export;
        }
    });
}

fn show_export(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Plano Editor & Export");
    ui.separator();

    if state.plano.is_empty() {
        state.plano = create_default_plano();
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label("Layers");
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut to_remove: Option<usize> = None;
                let mut to_move_up: Option<usize> = None;
                let mut to_move_down: Option<usize> = None;
                for (idx, obj) in state.plano.iter().enumerate() {
                    let label = match obj {
                        PlanoObject::Clip { .. } => format!("{idx}: Clip"),
                        PlanoObject::Shader { .. } => format!("{idx}: Shader"),
                        PlanoObject::Image { path, .. } => format!("{idx}: Image {path}"),
                        PlanoObject::Video { path, .. } => format!("{idx}: Video {path}"),
                    };
                    let selected = state.selected_layer == Some(idx);
                    ui.horizontal(|ui| {
                        if ui.selectable_label(selected, label).clicked() {
                            state.selected_layer = Some(idx);
                        }
                        if ui.small_button("↑").clicked() {
                            to_move_up = Some(idx);
                        }
                        if ui.small_button("↓").clicked() {
                            to_move_down = Some(idx);
                        }
                        if ui.small_button("✕").clicked() {
                            to_remove = Some(idx);
                        }
                    });
                }
                if let Some(idx) = to_move_up {
                    if idx > 0 {
                        state.plano.swap(idx, idx - 1);
                    }
                }
                if let Some(idx) = to_move_down {
                    if idx + 1 < state.plano.len() {
                        state.plano.swap(idx, idx + 1);
                    }
                }
                if let Some(idx) = to_remove {
                    state.plano.remove(idx);
                    state.selected_layer = None;
                }
            });
            if ui.button("Add Clip layer").clicked() {
                let pos = yt_shortmaker_core::plano::schema::Position {
                    x: yt_shortmaker_core::plano::schema::PositionValue::Pixels(0),
                    y: yt_shortmaker_core::plano::schema::PositionValue::Keyword("center".into()),
                    width: yt_shortmaker_core::plano::schema::SizeValue::Keyword("full".into()),
                    height: yt_shortmaker_core::plano::schema::SizeValue::Pixels(800),
                };
                state.plano.push(PlanoObject::Clip {
                    position: pos,
                    crop: None,
                    fit: yt_shortmaker_core::plano::schema::Fit::Cover,
                    comment: None,
                });
            }
        });

        ui.separator();

        ui.vertical(|ui| {
            ui.label("Inspector");
            if let Some(idx) = state.selected_layer {
                if let Some(obj) = state.plano.get(idx) {
                    ui.label(format!("Selected: {idx}"));
                    ui.label(format!("{obj:?}"));
                }
            } else {
                ui.label("Select a layer");
            }
            ui.separator();
            ui.label("Preview");
            let preview = yt_shortmaker_core::plano::preview::preview_info(&state.plano);
            ui.label(preview);
            ui.label("1080x1920 • 9:16 • embedded preview placeholder");
        });
    });

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Queue:");
        ui.label(format!("{} clips", state.moments.len()));
        if ui.button("Start Export").clicked() {
            state.export_progress = Some(0.0);
            state.status_msg = Some("Export started (mock)".into());
        }
        if let Some(p) = state.export_progress {
            ui.add(egui::ProgressBar::new(p));
        }
    });
    if let Some(msg) = &state.status_msg {
        ui.label(msg);
    }
}

fn show_export_standalone(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Export Standalone");
    ui.separator();
    ui.label("Select clips and a plano to export without AI.");
    if state.plano.is_empty() {
        state.plano = create_default_plano();
    }
    show_export(ui, state);
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
        let mut dir = state.config.output_dir.clone().unwrap_or_default();
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
        state.status_msg = Some("Key management expands next milestone".into());
    }
}
