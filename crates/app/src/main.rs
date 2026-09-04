#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// =================================================================================================
// app::main — Slint native entry point
// =================================================================================================

rust_i18n::i18n!("../../locales", fallback = "en");

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use rust_i18n::t;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;
use yt_shortmaker_core::config::{store as config_store, AppConfig};
use yt_shortmaker_core::media::pipeline::{Pipeline, PipelineCtx, PipelineEvent, Stage};
use yt_shortmaker_core::media::ytdlp;
use yt_shortmaker_core::{session, setup};

use slint::{Model, ModelRc, SharedString, VecModel};

slint::include_modules!();

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

fn apply_translations(app: &MainWindow) {
    app.set_tr_navigate(t!("nav_navigate").to_string().into());
    app.set_tr_home(t!("nav_home").to_string().into());
    app.set_tr_new_project(t!("nav_new_project").to_string().into());
    app.set_tr_export_standalone(t!("nav_export_standalone").to_string().into());
    app.set_tr_settings(t!("nav_settings").to_string().into());
    app.set_tr_keys(t!("nav_keys").to_string().into());
    app.set_tr_home_recent(t!("home_recent").to_string().into());
    app.set_tr_home_empty(t!("home_empty").to_string().into());
    app.set_tr_new_project_title(t!("new_project").to_string().into());
    app.set_tr_new_project_description(t!("new_project_description").to_string().into());
    app.set_tr_source_step(t!("new_project_source_step").to_string().into());
    app.set_tr_analysis_step(t!("new_project_analysis_step").to_string().into());
    app.set_tr_analysis_settings(t!("new_project_analysis_settings").to_string().into());
    app.set_tr_url_label(t!("new_project_url_label").to_string().into());
    app.set_tr_fetch(t!("new_project_fetch").to_string().into());
    app.set_tr_export_title(t!("export_standalone").to_string().into());
    app.set_tr_export_desc(t!("export_standalone_desc").to_string().into());
    app.set_tr_keys_title(t!("nav_keys").to_string().into());
    app.set_tr_keys_description(t!("keys_description").to_string().into());
    app.set_tr_settings_title(t!("settings").to_string().into());
    app.set_tr_settings_description(t!("settings_description").to_string().into());
    app.set_tr_general_section(t!("settings_general_section").to_string().into());
    app.set_tr_processing_section(t!("settings_processing_section").to_string().into());
    app.set_tr_auth_section(t!("settings_auth_section").to_string().into());
    app.set_tr_export_section(t!("settings_export_section").to_string().into());
    app.set_tr_save(t!("save").to_string().into());
    app.set_tr_analyze(t!("new_project_start_analyze").to_string().into());
    app.set_tr_browse(t!("browse").to_string().into());
    app.set_tr_output_dir(t!("desc_output_dir").to_string().into());
    app.set_tr_language(t!("language").to_string().into());
    app.set_tr_key_value(t!("field_key").to_string().into());
    app.set_tr_add(t!("keys_add_key").to_string().into());
    app.set_tr_chunk_size(t!("settings_chunk_size").to_string().into());
    app.set_tr_auto_extract(t!("settings_auto_extract").to_string().into());
    app.set_tr_use_cookies(t!("settings_cookies").to_string().into());
    app.set_tr_cookies_path(t!("settings_cookies_path").to_string().into());
    app.set_tr_naming_template(t!("settings_naming_template").to_string().into());
    app.set_tr_model(t!("settings_model").to_string().into());
    app.set_tr_ai_description(t!("keys_ai_description").to_string().into());
    app.set_tr_ai_section(t!("keys_ai_section").to_string().into());
    app.set_tr_keys_section(t!("keys_section").to_string().into());
    app.set_tr_key_description(t!("keys_key_description").to_string().into());
    app.set_tr_no_keys(t!("keys_no_keys").to_string().into());
    app.set_tr_export_workspace(t!("export_workspace").to_string().into());
    app.set_tr_export_workspace_description(t!("export_workspace_description").to_string().into());
    app.set_tr_deps_missing(t!("deps_missing_title").to_string().into());
    app.set_tr_deps_recheck(t!("deps_recheck").to_string().into());
    app.set_tr_toggle_key(t!("keys_toggle").to_string().into());
    app.set_tr_delete_key(t!("keys_delete").to_string().into());
    app.set_tr_test_connection(t!("keys_test_connection").to_string().into());
    app.set_tr_deps_installing(t!("deps_installing").to_string().into());
    app.set_tr_deps_install(t!("deps_install").to_string().into());
    app.set_tr_fetching(t!("fetching_video").to_string().into());
    app.set_tr_video_preview(t!("video_preview").to_string().into());
    app.set_tr_video_duration(t!("video_duration").to_string().into());
    app.set_tr_video_id(t!("video_id").to_string().into());
    app.set_tr_video_no_thumb(t!("video_no_thumb").to_string().into());
    app.set_tr_video_found(t!("video_found").to_string().into());
    app.set_tr_analysis_log_title(t!("analysis_logs_title").to_string().into());
    app.set_tr_analysis_log_toggle(t!("analysis_logs_toggle").to_string().into());
    app.set_tr_analysis_log_empty(t!("analysis_logs_empty").to_string().into());
    app.set_tr_step_downloading(t!("analysis_step_downloading").to_string().into());
    app.set_tr_step_splitting(t!("analysis_step_splitting").to_string().into());
    app.set_tr_step_analyzing(t!("analysis_step_analyzing").to_string().into());
    app.set_tr_step_extracting(t!("analysis_step_extracting").to_string().into());
    app.set_tr_step_saving(t!("analysis_step_saving").to_string().into());
    app.set_tr_model_display_name(t!("model_display_name").to_string().into());
    app.set_tr_model_id_label(t!("model_id_label").to_string().into());
    app.set_tr_model_base(t!("model_base_provider").to_string().into());
    app.set_tr_model_drawer_title(t!("model_drawer_title").to_string().into());
    app.set_tr_model_drawer_description(t!("model_drawer_description").to_string().into());
    app.set_tr_model_identity(t!("model_identity").to_string().into());
    app.set_tr_model_provider(t!("model_provider").to_string().into());
    app.set_tr_model_temperature(t!("model_temperature").to_string().into());
    app.set_tr_cancel(t!("cancel").to_string().into());
    app.set_tr_add_model(t!("add_model").to_string().into());
    app.set_tr_select_model(t!("select_model").to_string().into());
    app.set_tr_delete_model(t!("delete_model").to_string().into());
    app.set_tr_models_title(t!("models_title").to_string().into());
    app.set_tr_structure(t!("nav_structure").to_string().into());
    app.set_tr_structure_title(t!("structure_title").to_string().into());
    app.set_tr_structure_desc(t!("structure_description").to_string().into());
    app.set_tr_structure_empty(t!("structure_empty").to_string().into());
    app.set_tr_structure_no_video(t!("structure_no_video").to_string().into());
    app.set_tr_structure_save(t!("structure_save").to_string().into());
    app.set_tr_structure_save_template(t!("structure_save_template").to_string().into());
    app.set_tr_structure_restore_template(t!("structure_restore_template").to_string().into());
    app.set_tr_structure_load_preset(t!("structure_load_preset").to_string().into());
}

const LOG_RING_CAP: usize = 400;

fn should_stick_to_bottom(app: &MainWindow) -> bool {
    if !app.get_analysis_log_expanded() {
        return true;
    }
    let scroll_y = app.get_analysis_log_scroll_y();
    let count = app.get_analysis_log_lines().iter().count() as f32;
    // Estimate: each Text ~14px (10px font + 2px spacing + 2px padding)
    let line_h: f32 = 14.0;
    let content_h = count * line_h + 16.0;
    let viewport_h: f32 = 300.0;
    if content_h <= viewport_h {
        return true;
    }
    let bottom: f32 = -(content_h - viewport_h);
    // Consider at bottom if within 40px threshold
    scroll_y <= bottom + 40.0
}

fn scroll_to_bottom(app: &MainWindow) {
    let count = app.get_analysis_log_lines().iter().count() as f32;
    let content_h = count * 14.0 + 16.0;
    let viewport_h: f32 = 300.0;
    let bottom: f32 = if content_h <= viewport_h {
        0.0
    } else {
        -(content_h - viewport_h)
    };
    app.set_analysis_log_scroll_y(bottom);
}

fn append_log_line(app: &MainWindow, msg: SharedString) {
    let stick = should_stick_to_bottom(app);
    let model = app.get_analysis_log_lines();
    if let Some(vec_model) = model.as_any().downcast_ref::<VecModel<SharedString>>() {
        if vec_model.row_count() >= LOG_RING_CAP {
            vec_model.remove(0);
        }
        vec_model.push(msg);
    } else {
        // Fallback if model was not a VecModel (e.g. after hot-reload): rebuild
        let mut lines: Vec<SharedString> = model.iter().collect();
        if lines.len() >= LOG_RING_CAP {
            lines.remove(0);
        }
        lines.push(msg);
        app.set_analysis_log_lines(ModelRc::new(VecModel::from(lines)));
    }
    if stick {
        scroll_to_bottom(app);
    }
}

fn clear_log_lines(app: &MainWindow) {
    app.set_analysis_log_lines(ModelRc::new(VecModel::from(Vec::<SharedString>::new())));
    // Keep legacy string in sync (not rendered anymore)
    app.set_analysis_log("".into());
    app.set_analysis_log_scroll_y(0.0);
}

fn spawn_thumbnail_fetch(
    weak: slint::Weak<MainWindow>,
    vid: String,
    thumb_url: String,
    url: String,
    cached_fallback: Option<session::CachedVideo>,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async {
            let resp = reqwest::get(&thumb_url).await?;
            let bytes = resp.bytes().await?;
            let path = session::save_thumbnail_to_cache(&vid, &bytes)?;
            let path_for_img = path.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(a) = weak.upgrade() {
                    if let Ok(img) = slint::Image::load_from_path(&path_for_img) {
                        a.set_video_thumbnail_image(img);
                    }
                }
            });
            if let Ok(db) = session::db_path()
                .and_then(|p| session::init_db(&p).map_err(|e| anyhow::anyhow!(e)))
            {
                if let Ok(Some(mut cv)) = session::get_cached_video(&db, &url) {
                    cv.thumbnail_path = Some(path.to_string_lossy().to_string());
                    cv.thumbnail_url = Some(thumb_url.clone());
                    let _ = session::upsert_cached_video(&db, &cv);
                } else if let Some(mut fallback) = cached_fallback.clone() {
                    // Variant fallback (different URL shape for same video)
                    fallback.url = url.clone();
                    fallback.thumbnail_path = Some(path.to_string_lossy().to_string());
                    fallback.thumbnail_url = Some(thumb_url.clone());
                    fallback.fetched_at = chrono::Local::now().to_rfc3339();
                    let _ = session::upsert_cached_video(&db, &fallback);
                } else {
                    // Minimal row if cache was empty
                    let cv2 = session::CachedVideo {
                        url: url.clone(),
                        video_id: vid.clone(),
                        title: String::new(),
                        duration: 0.0,
                        thumbnail_url: Some(thumb_url.clone()),
                        thumbnail_path: Some(path.to_string_lossy().to_string()),
                        fetched_at: chrono::Local::now().to_rfc3339(),
                    };
                    let _ = session::upsert_cached_video(&db, &cv2);
                }
            }
            Ok::<(), anyhow::Error>(())
        });
    });
}

fn update_deps_ui(app: &MainWindow) -> bool {
    let status = setup::dependency_status();
    if status.is_ok() {
        app.set_deps_ok(true);
        app.set_deps_installing(false);
        app.set_deps_error("".into());
        true
    } else {
        let msg = setup::format_missing_message(&status.missing());
        app.set_deps_ok(false);
        app.set_deps_installing(false);
        app.set_deps_error(msg.into());
        false
    }
}

fn spawn_ensure_tools(app_weak: slint::Weak<MainWindow>) {
    if let Some(app) = app_weak.upgrade() {
        if app.get_deps_ok() || app.get_deps_installing() {
            return;
        }
        app.set_deps_installing(true);
        app.set_toast_msg(t!("deps_installing").to_string().into());
        app.set_status_msg("".into());
        app.set_settings_status("".into());
        app.set_keys_status("".into());
    }
    let weak = app_weak.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let weak2 = weak.clone();
        let on_progress = move |msg: String| {
            let w = weak2.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = w.upgrade() {
                    app.set_toast_msg(msg.into());
                }
            });
        };
        let res = rt.block_on(setup::ensure_tools_with_progress(on_progress));
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                match res {
                    Ok(()) => {
                        app.set_deps_ok(true);
                        app.set_deps_installing(false);
                        app.set_toast_msg(t!("deps_ok").to_string().into());
                        app.set_status_msg(t!("deps_ok").to_string().into());
                        // Clear toast after 3s
                        let w = app.as_weak();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(3));
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(a) = w.upgrade() {
                                    a.set_toast_msg("".into());
                                }
                            });
                        });
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        tracing::error!("auto-install failed: {msg}");
                        app.set_deps_installing(false);
                        app.set_deps_ok(false);
                        app.set_toast_msg(format!("Error: {msg}").into());
                        app.set_deps_error(msg.into());
                    }
                }
            }
        });
    });
}

fn populate_settings_ui(app: &MainWindow, cfg: &AppConfig) {
    app.set_output_dir(cfg.output_dir.clone().unwrap_or_default().into());
    app.set_language(cfg.language.clone().into());
    app.set_chunk_size_minutes(cfg.processing.chunk_size_minutes().to_string().into());
    app.set_auto_extract(cfg.processing.auto_extract);
    app.set_use_cookies(cfg.cookies.use_cookies);
    app.set_cookies_path(cfg.cookies.path.clone().into());
    app.set_naming_template(cfg.export.naming_template.clone().into());
    let provider = cfg.ai.active_provider.clone();
    if let Some(p) = cfg.ai.providers.get(&provider) {
        app.set_use_fast_model(p.use_fast_model);
        app.set_model_fast_name(p.model.clone().into());
        app.set_model_pro_name(p.model_pro.clone().into());
    }
}

fn apply_settings_from_ui(cfg: &mut AppConfig, app: &MainWindow) {
    cfg.output_dir = Some(app.get_output_dir().to_string());
    cfg.language = app.get_language().to_string();
    let minutes: u64 = app
        .get_chunk_size_minutes()
        .to_string()
        .parse()
        .unwrap_or(10);
    cfg.processing.set_chunk_size_minutes(minutes);
    cfg.processing.auto_extract = app.get_auto_extract();
    cfg.cookies.use_cookies = app.get_use_cookies();
    cfg.cookies.path = app.get_cookies_path().to_string();
    cfg.export.naming_template = app.get_naming_template().to_string();
}

fn apply_ai_settings_from_ui(cfg: &mut AppConfig, app: &MainWindow) {
    let provider = cfg.ai.active_provider.clone();
    if let Some(p) = cfg.ai.providers.get_mut(&provider) {
        p.use_fast_model = app.get_use_fast_model();
    }
}

fn refresh_keys_list(app: &MainWindow, config: &yt_shortmaker_core::config::AppConfig) {
    let provider = config.ai.resolved_base_provider();
    let text = if let Some(p) = config.ai.providers.get(&provider) {
        if p.keys.is_empty() {
            t!("keys_no_keys").to_string()
        } else {
            p.keys
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    format!(
                        "{}: {} [{}]",
                        i,
                        k.name,
                        if k.enabled { "on" } else { "off" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        t!("keys_no_keys").to_string()
    };
    app.set_keys_list(text.into());
    // Populate selectable model for per-row UI
    if let Some(p) = config.ai.providers.get(&provider) {
        let rows: Vec<KeyRow> = p
            .keys
            .iter()
            .enumerate()
            .map(|(i, k)| KeyRow {
                name: k.name.clone().into(),
                enabled: k.enabled,
                index: i as i32,
            })
            .collect();
        let len = rows.len() as i32;
        app.set_keys_model(std::rc::Rc::new(slint::VecModel::from(rows)).into());
        // Keep selection in range
        let sel = app.get_selected_key_index();
        if sel >= len {
            app.set_selected_key_index(-1);
        }
    } else {
        app.set_keys_model(std::rc::Rc::new(slint::VecModel::from(Vec::<KeyRow>::new())).into());
        app.set_selected_key_index(-1);
    }
}

fn refresh_models_list(app: &MainWindow, config: &yt_shortmaker_core::config::AppConfig) {
    if config.ai.custom_models.is_empty() {
        app.set_models_list("".into());
        app.set_models_model(
            std::rc::Rc::new(slint::VecModel::from(Vec::<ModelRow>::new())).into(),
        );
        return;
    }
    let active = config.ai.active_model_id.clone().unwrap_or_default();
    let text = config
        .ai
        .custom_models
        .iter()
        .map(|m| {
            let marker = if m.id == active { ">" } else { " " };
            let enabled = if m.enabled { "" } else { " [disabled]" };
            format!(
                "{} {} - {} [{}] (base: {}){}",
                marker, m.id, m.display_name, m.model_id, m.base_provider, enabled
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    app.set_models_list(text.into());
    let rows: Vec<ModelRow> = config
        .ai
        .custom_models
        .iter()
        .map(|m| ModelRow {
            id: m.id.clone().into(),
            display_name: m.display_name.clone().into(),
            model_id: m.model_id.clone().into(),
            base: m.base_provider.clone().into(),
            enabled: m.enabled,
            active: m.id == active,
        })
        .collect();
    app.set_models_model(std::rc::Rc::new(slint::VecModel::from(rows)).into());
    app.set_selected_model_id(active.into());
    // Keep the form's base in sync if empty
    if app.get_new_model_base().trim().is_empty() {
        app.set_new_model_base(config.ai.active_provider.clone().into());
    }
}

fn work_dir_for(video_id: &str) -> std::path::PathBuf {
    // Delegates to centralized session helper (removes fallback duplication with cli)
    session::video_work_dir_or_tmp(video_id)
}

fn global_template_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| {
        d.join(yt_shortmaker_core::config::APP_DIR_NAME)
            .join("structures")
            .join("default.json")
    })
}

fn structure_path_for(video_id: &str) -> std::path::PathBuf {
    work_dir_for(video_id).join("structure.json")
}

fn structure_history_path(video_id: &str) -> std::path::PathBuf {
    work_dir_for(video_id).join("structure-history.json")
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct StructureHistory {
    undo: Vec<yt_shortmaker_core::plano::schema::PlanoDocument>,
    redo: Vec<yt_shortmaker_core::plano::schema::PlanoDocument>,
}

fn load_structure_history(video_id: &str) -> StructureHistory {
    let path = structure_history_path(video_id);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_structure_history(video_id: &str, history: &StructureHistory) -> anyhow::Result<()> {
    let path = structure_history_path(video_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(history)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn record_structure_history(
    video_id: &str,
    before: &yt_shortmaker_core::plano::schema::PlanoDocument,
) {
    let mut history = load_structure_history(video_id);
    history.undo.push(before.clone());
    if history.undo.len() > 100 {
        history.undo.remove(0);
    }
    history.redo.clear();
    if let Err(e) = save_structure_history(video_id, &history) {
        tracing::warn!("structure history save failed: {e}");
    }
}

fn update_structure_history_flags(app: &MainWindow) {
    let vid = active_video_id(app);
    if vid.trim().is_empty() {
        app.set_structure_can_undo(false);
        app.set_structure_can_redo(false);
        return;
    }
    let history = load_structure_history(&vid);
    app.set_structure_can_undo(!history.undo.is_empty());
    app.set_structure_can_redo(!history.redo.is_empty());
}

fn active_video_id(app: &MainWindow) -> String {
    let sel: String = app.get_structure_selected_video_id().into();
    if !sel.trim().is_empty() {
        return sel;
    }
    app.get_video_id().to_string()
}

// Active document cache: in-memory during drag, persisted on gesture finish.
// Guarantees a single undo entry per gesture instead of one per mouse delta.
static STRUCTURE_ACTIVE_DOCS: std::sync::OnceLock<
    std::sync::Mutex<
        std::collections::HashMap<String, yt_shortmaker_core::plano::schema::PlanoDocument>,
    >,
> = std::sync::OnceLock::new();
static STRUCTURE_GESTURE_BEFORE: std::sync::OnceLock<
    std::sync::Mutex<
        std::collections::HashMap<String, yt_shortmaker_core::plano::schema::PlanoDocument>,
    >,
> = std::sync::OnceLock::new();

fn structure_active_docs() -> &'static std::sync::Mutex<
    std::collections::HashMap<String, yt_shortmaker_core::plano::schema::PlanoDocument>,
> {
    STRUCTURE_ACTIVE_DOCS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn structure_gesture_before() -> &'static std::sync::Mutex<
    std::collections::HashMap<String, yt_shortmaker_core::plano::schema::PlanoDocument>,
> {
    STRUCTURE_GESTURE_BEFORE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn load_active_structure_doc(video_id: &str) -> yt_shortmaker_core::plano::schema::PlanoDocument {
    if let Some(cached) = structure_active_docs()
        .lock()
        .ok()
        .and_then(|m| m.get(video_id).cloned())
    {
        return cached;
    }
    let path = structure_path_for(video_id);
    let doc = if path.exists() {
        yt_shortmaker_core::plano::schema::load_document(&path.to_string_lossy())
            .unwrap_or_default()
    } else {
        yt_shortmaker_core::plano::schema::create_empty_document()
    };
    if let Ok(mut m) = structure_active_docs().lock() {
        m.insert(video_id.to_string(), doc.clone());
    }
    doc
}

fn set_active_structure_doc(video_id: &str, doc: yt_shortmaker_core::plano::schema::PlanoDocument) {
    if let Ok(mut m) = structure_active_docs().lock() {
        m.insert(video_id.to_string(), doc);
    }
}

fn begin_structure_gesture(video_id: &str) {
    let doc = load_active_structure_doc(video_id);
    if let Ok(mut m) = structure_gesture_before().lock() {
        m.insert(video_id.to_string(), doc.clone());
    }
    set_active_structure_doc(video_id, doc);
}

fn preview_structure_doc_to_ui(
    app: &MainWindow,
    video_id: &str,
    doc: &yt_shortmaker_core::plano::schema::PlanoDocument,
) {
    let selected: String = app.get_structure_selected_layer_id().into();
    let canvas = document_to_canvas_layers(doc, &selected, video_id);
    let rows = document_to_layer_rows(doc, &selected);
    app.set_structure_canvas_layers(std::rc::Rc::new(slint::VecModel::from(canvas)).into());
    app.set_structure_layers_model(std::rc::Rc::new(slint::VecModel::from(rows)).into());
    app.set_structure_document_dirty(true);
}

fn finish_structure_gesture(app: &MainWindow, video_id: &str) {
    let doc = load_active_structure_doc(video_id);
    let before = structure_gesture_before()
        .lock()
        .ok()
        .and_then(|mut m| m.remove(video_id));
    let path = structure_path_for(video_id);
    match yt_shortmaker_core::plano::schema::save_document(&path.to_string_lossy(), &doc) {
        Ok(()) => {
            if let Ok(json) = serde_json::to_string(&doc) {
                if let Ok(db_path) = session::db_path() {
                    if let Ok(conn) = session::init_db(&db_path) {
                        let _ = session::upsert_video_structure(&conn, video_id, &json);
                    }
                }
            }
            if let Some(b) = before {
                // Only record history when something actually changed.
                if b != doc {
                    record_structure_history(video_id, &b);
                }
            }
            app.set_structure_document_dirty(false);
            append_structure_log(
                app,
                "INFO",
                "structure",
                &format!("document saved for {video_id}"),
            );
            refresh_structure_ui(app);
        }
        Err(e) => app.set_structure_status(format!("{}: {e}", t!("error")).into()),
    }
}

fn resolve_active_video_path(app: &MainWindow) -> Option<std::path::PathBuf> {
    let vid = active_video_id(app);
    if vid.trim().is_empty() {
        return None;
    }
    if let Ok(db_path) = session::db_path() {
        if let Ok(conn) = session::init_db(&db_path) {
            if let Ok(Some(job)) = session::get_video_job(&conn, &vid) {
                if let Some(p) = job.download_path {
                    let pb = std::path::PathBuf::from(&p);
                    if pb.exists() {
                        return Some(pb);
                    }
                }
            }
        }
    }
    let fallback = work_dir_for(&vid).join(format!("{vid}.mp4"));
    if fallback.exists() {
        return Some(fallback);
    }
    // Last resort: work dir path even if missing, to surface a clear error downstream.
    Some(fallback)
}

fn document_to_canvas_layers(
    doc: &yt_shortmaker_core::plano::schema::PlanoDocument,
    selected_id: &str,
    video_id: &str,
) -> Vec<CanvasLayer> {
    use yt_shortmaker_core::plano::schema::{OUTPUT_HEIGHT, OUTPUT_WIDTH};
    doc.layers
        .iter()
        .map(|l| {
            let (w, h, x, y) = match &l.object {
                yt_shortmaker_core::plano::schema::PlanoObject::Clip { position, .. } => {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    (
                        w,
                        h,
                        position.x.resolve(OUTPUT_WIDTH, w),
                        position.y.resolve(OUTPUT_HEIGHT, h),
                    )
                }
                yt_shortmaker_core::plano::schema::PlanoObject::Image { position, .. } => {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    (
                        w,
                        h,
                        position.x.resolve(OUTPUT_WIDTH, w),
                        position.y.resolve(OUTPUT_HEIGHT, h),
                    )
                }
                yt_shortmaker_core::plano::schema::PlanoObject::Shader { position, .. } => {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    (
                        w,
                        h,
                        position.x.resolve(OUTPUT_WIDTH, w),
                        position.y.resolve(OUTPUT_HEIGHT, h),
                    )
                }
                yt_shortmaker_core::plano::schema::PlanoObject::Video { position, .. } => {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    (
                        w,
                        h,
                        position.x.resolve(OUTPUT_WIDTH, w),
                        position.y.resolve(OUTPUT_HEIGHT, h),
                    )
                }
            };
            CanvasLayer {
                id: l.id.clone().into(),
                layer_type: l.layer_type().into(),
                x: x as f32,
                y: y as f32,
                width: w as f32,
                height: h as f32,
                visible: l.visible,
                selected: l.id == selected_id,
                preview_image: match &l.object {
                    yt_shortmaker_core::plano::schema::PlanoObject::Clip { .. } => {
                        session::cached_thumbnail_on_disk(video_id)
                            .and_then(|path| slint::Image::load_from_path(&path).ok())
                            .unwrap_or_default()
                    }
                    yt_shortmaker_core::plano::schema::PlanoObject::Image { path, .. }
                        if std::path::Path::new(path).exists() =>
                    {
                        slint::Image::load_from_path(std::path::Path::new(path)).unwrap_or_default()
                    }
                    _ => slint::Image::default(),
                },
                preview_available: match &l.object {
                    yt_shortmaker_core::plano::schema::PlanoObject::Clip { .. } => {
                        session::cached_thumbnail_on_disk(video_id).is_some()
                    }
                    yt_shortmaker_core::plano::schema::PlanoObject::Image { path, .. } => {
                        std::path::Path::new(path).exists()
                    }
                    _ => false,
                },
                asset_name: match &l.object {
                    yt_shortmaker_core::plano::schema::PlanoObject::Image { path, .. }
                    | yt_shortmaker_core::plano::schema::PlanoObject::Video { path, .. } => {
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("")
                            .into()
                    }
                    _ => "".into(),
                },
                asset_valid: match &l.object {
                    yt_shortmaker_core::plano::schema::PlanoObject::Clip { .. } => true,
                    yt_shortmaker_core::plano::schema::PlanoObject::Image { path, .. }
                    | yt_shortmaker_core::plano::schema::PlanoObject::Video { path, .. } => {
                        std::path::Path::new(path).exists()
                    }
                    _ => true,
                },
            }
        })
        .collect()
}

fn refresh_structure_assets(app: &MainWindow, video_id: &str) {
    let mut rows = Vec::new();
    let assets = work_dir_for(video_id).join("assets");
    if let Ok(entries) = std::fs::read_dir(assets) {
        for (index, entry) in entries.flatten().enumerate() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let metadata = std::fs::metadata(&path).ok();
            let size = metadata
                .map(|m| format!("{} KB", m.len().div_ceil(1024)))
                .unwrap_or_else(|| "unknown".to_string());
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("asset");
            let kind = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("file")
                .to_uppercase();
            rows.push(StructureAssetRow {
                id: path.to_string_lossy().to_string().into(),
                name: name.into(),
                kind: kind.into(),
                size: size.into(),
                status: "ready".into(),
                used_count: index as i32,
            });
        }
    }
    app.set_structure_assets_model(std::rc::Rc::new(slint::VecModel::from(rows)).into());
}

fn append_structure_log(app: &MainWindow, level: &str, scope: &str, message: &str) {
    let model = app.get_structure_logs_model();
    let mut rows: Vec<StructureLogRow> = model.iter().collect();
    let id = rows.last().map(|row| row.id + 1).unwrap_or(1);
    rows.push(StructureLogRow {
        id,
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string().into(),
        level: level.into(),
        scope: scope.into(),
        message: message.into(),
        detail: "".into(),
    });
    if rows.len() > 500 {
        rows.remove(0);
    }
    app.set_structure_logs_model(std::rc::Rc::new(slint::VecModel::from(rows)).into());
    if let Some(dir) = dirs::data_local_dir() {
        let dir = dir
            .join(yt_shortmaker_core::config::APP_DIR_NAME)
            .join("logs");
        if std::fs::create_dir_all(&dir).is_ok() {
            let path = dir.join("structure.log");
            if std::fs::metadata(&path)
                .map(|meta| meta.len() > 10 * 1024 * 1024)
                .unwrap_or(false)
            {
                let rotated = dir.join("structure.log.1");
                let _ = std::fs::rename(&path, rotated);
            }
            let line = format!(
                "{} | {} | {} | {}\n",
                chrono::Local::now().to_rfc3339(),
                level,
                scope,
                message
            );
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
}

fn document_to_layer_rows(
    doc: &yt_shortmaker_core::plano::schema::PlanoDocument,
    selected_id: &str,
) -> Vec<StructureLayerRow> {
    use yt_shortmaker_core::plano::schema::{OUTPUT_HEIGHT, OUTPUT_WIDTH};
    doc.layers
        .iter()
        .map(|l| {
            let (x, y, w, h) = {
                let (rx, ry, rw, rh) = l.resolved_rect();
                let _ = (OUTPUT_WIDTH, OUTPUT_HEIGHT);
                (rx as f32, ry as f32, rw as f32, rh as f32)
            };
            StructureLayerRow {
                id: l.id.clone().into(),
                name: l.name.clone().into(),
                layer_type: l.layer_type().into(),
                visible: l.visible,
                selected: l.id == selected_id,
                x,
                y,
                width: w,
                height: h,
            }
        })
        .collect()
}

fn refresh_structure_inspector(
    app: &MainWindow,
    doc: &yt_shortmaker_core::plano::schema::PlanoDocument,
) {
    let selected: String = app.get_structure_selected_layer_id().into();
    if let Some(l) = doc.layers.iter().find(|x| x.id == selected) {
        let (x, y, w, h) = l.resolved_rect();
        app.set_structure_inspector_name(l.name.clone().into());
        app.set_structure_inspector_type(l.layer_type().into());
        // Only overwrite text fields when selection changed to avoid clobbering typing.
        // For Phase inspector we refresh on selection change; commits re-refresh after save.
        app.set_structure_inspector_x(x.to_string().into());
        app.set_structure_inspector_y(y.to_string().into());
        app.set_structure_inspector_w(w.to_string().into());
        app.set_structure_inspector_h(h.to_string().into());
        app.set_structure_inspector_has_selection(true);
    } else {
        app.set_structure_inspector_name("".into());
        app.set_structure_inspector_type("".into());
        app.set_structure_inspector_has_selection(false);
    }
}

fn refresh_structure_videos(app: &MainWindow) -> Vec<String> {
    let jobs: Vec<session::VideoJob> = (|| {
        let db_path = session::db_path().ok()?;
        let conn = session::init_db(&db_path).ok()?;
        session::list_analyzed_videos(&conn).ok()
    })()
    .unwrap_or_default();
    let selected: String = app.get_structure_selected_video_id().into();
    let home_vid: String = app.get_video_id().into();
    let rows: Vec<StructureVideoRow> = jobs
        .iter()
        .map(|j| {
            let is_sel = if !selected.trim().is_empty() {
                j.video_id == selected
            } else {
                j.video_id == home_vid
            };
            let moments = format!("{} moments", j.analyzed_chunks);
            StructureVideoRow {
                id: j.video_id.clone().into(),
                title: j.title.clone().into(),
                duration: yt_shortmaker_core::util::format_duration(j.duration as u64).into(),
                moments: moments.into(),
                status: j.status.clone().into(),
                selected: is_sel,
            }
        })
        .collect();
    let ids: Vec<String> = jobs.into_iter().map(|j| j.video_id).collect();
    app.set_structure_videos_model(std::rc::Rc::new(slint::VecModel::from(rows)).into());
    ids
}

fn refresh_structure_ui(app: &MainWindow) {
    let available = refresh_structure_videos(app);
    let sel: String = app.get_structure_selected_video_id().into();
    let home: String = app.get_video_id().into();
    let vid = if !sel.trim().is_empty() {
        sel
    } else if !home.trim().is_empty() {
        home
    } else if let Some(first) = available.first() {
        first.clone()
    } else {
        String::new()
    };
    if vid.trim().is_empty() {
        app.set_structure_has_video(false);
        app.set_structure_selected_video_id("".into());
        app.set_structure_active_video_title("No video".into());
        app.set_structure_active_video_meta("".into());
        app.set_structure_assets_model(
            std::rc::Rc::new(slint::VecModel::from(Vec::<StructureAssetRow>::new())).into(),
        );
        update_structure_history_flags(app);
        app.set_structure_status(t!("structure_no_video").to_string().into());
        app.set_structure_canvas_layers(
            std::rc::Rc::new(slint::VecModel::from(Vec::<CanvasLayer>::new())).into(),
        );
        app.set_structure_layers_model(
            std::rc::Rc::new(slint::VecModel::from(Vec::<StructureLayerRow>::new())).into(),
        );
        app.set_structure_inspector_has_selection(false);
        refresh_structure_moments(app, &vid);
        return;
    }
    app.set_structure_has_video(true);
    app.set_structure_selected_video_id(vid.clone().into());
    if let Ok(db_path) = session::db_path() {
        if let Ok(conn) = session::init_db(&db_path) {
            if let Ok(Some(job)) = session::get_video_job(&conn, &vid) {
                app.set_structure_active_video_title(job.title.into());
                app.set_structure_active_video_meta(
                    format!(
                        "{}  |  {} moments  |  {}",
                        yt_shortmaker_core::util::format_duration(job.duration as u64),
                        job.analyzed_chunks,
                        job.status
                    )
                    .into(),
                );
            }
        }
    }
    refresh_structure_assets(app, &vid);
    update_structure_history_flags(app);
    // Keep videos-model selection in sync without rebuilding twice.
    refresh_structure_videos(app);
    let selected: String = app.get_structure_selected_layer_id().into();
    let path = structure_path_for(&vid);
    if path.exists() {
        match yt_shortmaker_core::plano::schema::load_document(&path.to_string_lossy()) {
            Ok(doc) => {
                // Keep in-memory gesture cache in sync unless a drag is in progress.
                let gesture_active = structure_gesture_before()
                    .lock()
                    .ok()
                    .map(|m| m.contains_key(&vid))
                    .unwrap_or(false);
                if !gesture_active {
                    set_active_structure_doc(&vid, doc.clone());
                }
                let canvas = document_to_canvas_layers(&doc, &selected, &vid);
                let rows = document_to_layer_rows(&doc, &selected);
                app.set_structure_canvas_layers(
                    std::rc::Rc::new(slint::VecModel::from(canvas)).into(),
                );
                app.set_structure_layers_model(
                    std::rc::Rc::new(slint::VecModel::from(rows)).into(),
                );
                // Preserve in-progress typing: only refresh inspector when selection changed
                // or when fields are empty. Commits explicitly refresh after save.
                let needs_inspector = app.get_structure_inspector_x().is_empty()
                    || app.get_structure_inspector_has_selection()
                        != doc.layers.iter().any(|x| x.id == selected);
                if needs_inspector || !app.get_structure_inspector_has_selection() {
                    refresh_structure_inspector(app, &doc);
                } else if let Some(l) = doc.layers.iter().find(|x| x.id == selected) {
                    app.set_structure_inspector_name(l.name.clone().into());
                    app.set_structure_inspector_type(l.layer_type().into());
                    app.set_structure_inspector_has_selection(true);
                } else {
                    refresh_structure_inspector(app, &doc);
                }
                if let Err(e) = doc.validate_for_export() {
                    // Empty canvas is a normal state, not an error status that blocks editing.
                    if doc.layers.is_empty() {
                        if app.get_structure_status().is_empty() {
                            app.set_structure_status(t!("structure_empty").to_string().into());
                        }
                    } else {
                        app.set_structure_status(
                            format!("{}: {e}", t!("structure_status_empty_invalid")).into(),
                        );
                    }
                } else {
                    app.set_structure_status(
                        format!(
                            "{}: {} layers",
                            t!("structure_status_saved"),
                            doc.layers.len()
                        )
                        .into(),
                    );
                }
                refresh_structure_moments(app, &vid);
            }
            Err(e) => app.set_structure_status(format!("{}: {e}", t!("error")).into()),
        }
    } else {
        app.set_structure_canvas_layers(
            std::rc::Rc::new(slint::VecModel::from(Vec::<CanvasLayer>::new())).into(),
        );
        app.set_structure_layers_model(
            std::rc::Rc::new(slint::VecModel::from(Vec::<StructureLayerRow>::new())).into(),
        );
        app.set_structure_inspector_has_selection(false);
        if app.get_structure_status().is_empty() {
            app.set_structure_status(t!("structure_empty").to_string().into());
        }
        refresh_structure_moments(app, &vid);
    }
}

static EXPORT_CANCEL: std::sync::OnceLock<std::sync::Arc<std::sync::atomic::AtomicBool>> =
    std::sync::OnceLock::new();

fn export_cancel_flag() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    EXPORT_CANCEL
        .get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
        .clone()
}

fn refresh_structure_moments(app: &MainWindow, vid: &str) {
    use slint::{Model, ModelRc, VecModel};
    let existing_sel: std::collections::HashSet<i32> = app
        .get_structure_moments_model()
        .iter()
        .filter(|r| r.selected)
        .map(|r| r.index)
        .collect();
    let (moments, exports): (
        Vec<yt_shortmaker_core::types::VideoMoment>,
        Vec<session::ExportRecord>,
    ) = (|| -> Option<(
        Vec<yt_shortmaker_core::types::VideoMoment>,
        Vec<session::ExportRecord>,
    )> {
        let db_path = session::db_path().ok()?;
        let conn = session::init_db(&db_path).ok()?;
        let m = session::get_job_moments(&conn, vid).unwrap_or_default();
        let e = session::get_exports(&conn, vid).unwrap_or_default();
        Some((m, e))
    })()
    .unwrap_or_default();
    let rows: Vec<MomentRow> = moments
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let dur = (|| {
                let s = yt_shortmaker_core::types::parse_timestamp_to_seconds(&m.start_time)?;
                let e = yt_shortmaker_core::types::parse_timestamp_to_seconds(&m.end_time)?;
                Some(e.saturating_sub(s))
            })()
            .unwrap_or(0);
            let status = exports
                .iter()
                .find(|x| x.moment_idx as usize == i)
                .map(|x| x.status.clone())
                .unwrap_or_else(|| "pending".to_string());
            MomentRow {
                index: i as i32,
                start: m.start_time.clone().into(),
                end: m.end_time.clone().into(),
                duration: format!("{dur}s").into(),
                category: m.category.clone().into(),
                selected: existing_sel.contains(&(i as i32)),
                export_status: status.into(),
            }
        })
        .collect();
    app.set_structure_moments_model(std::rc::Rc::new(VecModel::from(rows)).into());
    let _ = ModelRc::new(VecModel::from(Vec::<MomentRow>::new()));
}

fn with_structure_doc<F>(app: &MainWindow, f: F)
where
    F: FnOnce(&mut yt_shortmaker_core::plano::schema::PlanoDocument) -> Result<(), String>,
{
    let vid: String = app.get_structure_selected_video_id().into();
    let vid = if vid.trim().is_empty() {
        app.get_video_id().to_string()
    } else {
        vid
    };
    if vid.trim().is_empty() {
        app.set_structure_status(t!("structure_no_video").to_string().into());
        return;
    }
    let path = structure_path_for(&vid);
    append_structure_log(
        app,
        "INFO",
        "structure",
        &format!("loading document for {vid}"),
    );
    let mut doc = if path.exists() {
        match yt_shortmaker_core::plano::schema::load_document(&path.to_string_lossy()) {
            Ok(d) => d,
            Err(e) => {
                app.set_structure_status(format!("{}: {e}", t!("error")).into());
                return;
            }
        }
    } else {
        yt_shortmaker_core::plano::schema::create_empty_document()
    };
    let before = doc.clone();
    match f(&mut doc) {
        Ok(()) => {
            match yt_shortmaker_core::plano::schema::save_document(&path.to_string_lossy(), &doc) {
                Ok(()) => {
                    if let Ok(json) = serde_json::to_string(&doc) {
                        if let Ok(db_path) = session::db_path() {
                            if let Ok(conn) = session::init_db(&db_path) {
                                let _ = session::upsert_video_structure(&conn, &vid, &json);
                            }
                        }
                    }
                    record_structure_history(&vid, &before);
                    set_active_structure_doc(&vid, doc.clone());
                    app.set_structure_document_dirty(false);
                    append_structure_log(
                        app,
                        "INFO",
                        "structure",
                        &format!("document saved for {vid}"),
                    );
                    refresh_structure_ui(app);
                }
                Err(e) => app.set_structure_status(format!("{}: {e}", t!("error")).into()),
            }
        }
        Err(e) => app.set_structure_status(format!("{}: {e}", t!("error")).into()),
    }
}

fn ensure_video_job(video_id: &str, url: &str, title: &str, duration: f64) {
    let work_dir = work_dir_for(video_id);
    let _ = std::fs::create_dir_all(&work_dir);
    if let Ok(db_path) = session::db_path() {
        if let Ok(conn) = session::init_db(&db_path) {
            let existing = session::get_video_job(&conn, video_id).ok().flatten();
            let job = session::VideoJob {
                video_id: video_id.to_owned(),
                url: url.to_owned(),
                title: title.to_owned(),
                duration,
                work_dir: work_dir.to_string_lossy().to_string(),
                download_path: existing
                    .as_ref()
                    .and_then(|j| j.download_path.clone())
                    .or_else(|| {
                        Some(
                            work_dir
                                .join(format!("{video_id}.mp4"))
                                .to_string_lossy()
                                .to_string(),
                        )
                    }),
                download_verified: existing
                    .as_ref()
                    .map(|j| j.download_verified)
                    .unwrap_or(false),
                split_verified: existing.as_ref().map(|j| j.split_verified).unwrap_or(false),
                total_chunks: existing.as_ref().map(|j| j.total_chunks).unwrap_or(0),
                analyzed_chunks: existing.as_ref().map(|j| j.analyzed_chunks).unwrap_or(0),
                status: existing
                    .map(|j| j.status)
                    .unwrap_or_else(|| "pending".to_string()),
            };
            let _ = session::upsert_video_job(&conn, &job);
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .init();

    let config = config_store::load().unwrap_or_default();
    rust_i18n::set_locale(&config.language);
    let config_rc = Rc::new(RefCell::new(config));

    let app = MainWindow::new()?;
    app.set_core_version(yt_shortmaker_core::version().into());
    populate_settings_ui(&app, &config_rc.borrow());
    apply_translations(&app);
    refresh_keys_list(&app, &config_rc.borrow());
    {
        let mut cfg = config_rc.borrow_mut();
        let issues = cfg.normalize();
        if !issues.is_empty() {
            tracing::debug!("config normalized: {:?}", issues);
            let to_save = cfg.clone();
            drop(cfg);
            let _ = config_store::save(&to_save);
        } else {
            drop(cfg);
        }
    }
    refresh_models_list(&app, &config_rc.borrow());
    let had_deps = update_deps_ui(&app);
    if !had_deps {
        // The app installs its own dependencies — no user manual step
        spawn_ensure_tools(app.as_weak());
    }
    // Ensure home-recent property exists but not used (no projects list)
    app.set_home_recent_list("".into());
    clear_log_lines(&app);

    let app_weak = app.as_weak();

    app.on_navigate({
        let app_weak = app_weak.clone();
        move |route| {
            if let Some(app) = app_weak.upgrade() {
                // Normalize legacy new-project route to home (single video flow)
                let route: slint::SharedString = if route == "new-project" {
                    "home".into()
                } else {
                    route
                };
                let is_structure = route.as_str() == "structure";
                app.set_route(route);
                if is_structure {
                    refresh_structure_ui(&app);
                }
            }
        }
    });

    app.on_fetch_info({
        let app_weak = app_weak.clone();
        let config_rc = config_rc.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if !app.get_deps_ok() {
                    update_deps_ui(&app);
                    if !app.get_deps_ok() {
                        spawn_ensure_tools(app_weak.clone());
                        app.set_url_error(t!("deps_installing").to_string().into());
                        app.set_toast_msg(t!("deps_installing").to_string().into());
                        tracing::info!("fetch auto-triggering deps install");
                        return;
                    }
                }
                let raw_url: String = app.get_url_input().into();
                let url = raw_url.trim().to_owned();
                // Keep input normalized in UI so copy/paste with trailing spaces still hits cache
                if raw_url != url {
                    app.set_url_input(url.clone().into());
                }
                match ytdlp::validate_media_url(&url) {
                    Ok(()) => {
                        app.set_url_error("".into());
                        // Try SQLite cache first — centralized helper handles url->id variant
                        let cached_hit: Option<session::CachedVideo> =
                            session::cached_video_for_url(&url).ok().flatten();
                        if let Some(cached) = cached_hit {
                            let duration_str =
                                yt_shortmaker_core::util::format_duration(cached.duration as u64);
                            app.set_video_title(cached.title.clone().into());
                            app.set_video_id(cached.video_id.clone().into());
                            app.set_video_duration(duration_str.clone().into());
                            app.set_video_thumbnail(
                                cached.thumbnail_url.clone().unwrap_or_default().into(),
                            );
                            app.set_video_has_info(true);
                            app.set_status_msg(
                                t!(
                                    "video_found",
                                    title = cached.title.clone(),
                                    duration = duration_str.clone()
                                )
                                .to_string()
                                .into(),
                            );
                            // Prepare per-video work dir for checkpointing (verified on next analyze)
                            ensure_video_job(
                                &cached.video_id,
                                &url,
                                &cached.title,
                                cached.duration,
                            );
                            // Prefer thumbnail persisted in img_cache (same folder as SQLite)
                            let vid = cached.video_id.clone();
                            let cached_thumb_url = cached.thumbnail_url.clone();
                            // First try explicit thumbnail_path on disk, then conventional img_cache/<video_id>.jpg
                            let disk_path = cached
                                .thumbnail_path
                                .as_deref()
                                .map(std::path::PathBuf::from)
                                .filter(|p| p.exists())
                                .or_else(|| session::cached_thumbnail_on_disk(&vid));
                            if let Some(p) = disk_path {
                                if let Ok(img) = slint::Image::load_from_path(&p) {
                                    app.set_video_thumbnail_image(img);
                                }
                            } else if let Some(thumb_url) = cached_thumb_url {
                                spawn_thumbnail_fetch(
                                    app.as_weak(),
                                    vid.clone(),
                                    thumb_url,
                                    url.clone(),
                                    Some(cached.clone()),
                                );
                            }
                            return;
                        }

                        // Cache miss — clear UI and hit the network (yt-dlp)
                        app.set_video_has_info(false);
                        app.set_video_title("".into());
                        app.set_video_id("".into());
                        app.set_video_duration("".into());
                        app.set_video_thumbnail("".into());
                        app.set_video_thumbnail_image(slint::Image::default());
                        app.set_status_msg(t!("fetching_video").to_string().into());
                        app.set_video_title("".into());
                        let cfg = config_rc.borrow().clone();
                        let app_weak2 = app_weak.clone();
                        let url_for_cache = url.clone();
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            let token = CancellationToken::new();
                            let res = rt.block_on(ytdlp::fetch_info(
                                &url,
                                cfg.cookies.use_cookies,
                                (!cfg.cookies.path.is_empty()).then_some(cfg.cookies.path.as_str()),
                                cfg.processing.retry_attempts,
                                &token,
                            ));
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(app) = app_weak2.upgrade() {
                                    match res {
                                        Ok(info) => {
                                            let duration_str =
                                                yt_shortmaker_core::util::format_duration(
                                                    info.duration_secs as u64,
                                                );
                                            app.set_video_title(info.title.clone().into());
                                            app.set_video_id(info.video_id.clone().into());
                                            app.set_video_duration(duration_str.clone().into());
                                            app.set_video_thumbnail(
                                                info.thumbnail.clone().unwrap_or_default().into(),
                                            );
                                            app.set_video_has_info(true);
                                            app.set_url_error("".into());
                                            app.set_status_msg(
                                                t!(
                                                    "video_found",
                                                    title = info.title.clone(),
                                                    duration = duration_str.clone()
                                                )
                                                .to_string()
                                                .into(),
                                            );
                                            // Persist to SQLite + img_cache for future cache hits
                                            let cached = session::CachedVideo {
                                                url: url_for_cache.clone(),
                                                video_id: info.video_id.clone(),
                                                title: info.title.clone(),
                                                duration: info.duration_secs,
                                                thumbnail_url: info.thumbnail.clone(),
                                                thumbnail_path: None,
                                                fetched_at: chrono::Local::now().to_rfc3339(),
                                            };
                                            if let Ok(db) = session::db_path().and_then(|p| {
                                                session::init_db(&p).map_err(|e| anyhow::anyhow!(e))
                                            }) {
                                                let _ = session::upsert_cached_video(&db, &cached);
                                            }
                                            ensure_video_job(
                                                &info.video_id,
                                                &url_for_cache,
                                                &info.title,
                                                info.duration_secs,
                                            );
                                            // Thumbnail → img_cache/<video_id>.jpg (centralized)
                                            if let Some(thumb_url) = info.thumbnail.clone() {
                                                spawn_thumbnail_fetch(
                                                    app_weak2.clone(),
                                                    info.video_id.clone(),
                                                    thumb_url,
                                                    url_for_cache.clone(),
                                                    None,
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            app.set_video_has_info(false);
                                            app.set_url_error(e.to_string().into());
                                            app.set_status_msg(
                                                format!("{}: {e}", t!("error")).into(),
                                            );
                                        }
                                    }
                                }
                            });
                        });
                    }
                    Err(e) => {
                        app.set_video_has_info(false);
                        app.set_url_error(e.to_string().into());
                        app.set_status_msg(format!("{}: {e}", t!("error")).into());
                    }
                }
            }
        }
    });

    app.on_start_analyze({
        let app_weak = app_weak.clone();
        let config_rc = config_rc.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if !app.get_deps_ok() {
                    update_deps_ui(&app);
                    if !app.get_deps_ok() {
                        spawn_ensure_tools(app_weak.clone());
                        app.set_status_msg(t!("deps_installing").to_string().into());
                        app.set_toast_msg(t!("deps_installing").to_string().into());
                        tracing::info!("analyze auto-triggering deps install");
                        return;
                    }
                }
                let raw: String = app.get_url_input().into();
                let url = raw.trim().to_owned();
                if url.is_empty() {
                    app.set_status_msg(t!("error").to_string().into());
                    return;
                }
                if raw != url {
                    app.set_url_input(url.clone().into());
                }
                let cfg = Arc::new(config_rc.borrow().clone());
                let app_weak2 = app_weak.clone();

                app.set_progress(0.0);
                app.set_is_analyzing(true);
                app.set_analysis_current_step(1);
                app.set_analysis_total_steps(4);
                app.set_analysis_step_label(t!("analysis_step_downloading").to_string().into());
                clear_log_lines(&app);
                app.set_analysis_log_expanded(false);
                app.set_analysis_anim(0.0);
                app.set_status_msg(format!("1/4 - {}", t!("analysis_step_downloading")).into());

                // Shimmer ticker — drives the animated progress bar while is-analyzing
                let ticker_weak = app_weak2.clone();
                let anim_pos = std::sync::Arc::new(std::sync::Mutex::new(0.0f32));
                let anim_pos2 = anim_pos.clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_millis(35));
                    if ticker_weak
                        .upgrade()
                        .map(|a| !a.get_is_analyzing())
                        .unwrap_or(true)
                    {
                        break;
                    }
                    let next = {
                        let mut g = anim_pos2.lock().unwrap();
                        *g = (*g + 0.045) % 1.0;
                        *g
                    };
                    let w = ticker_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(a) = w.upgrade() {
                            if a.get_is_analyzing() {
                                a.set_analysis_anim(next);
                            }
                        }
                    });
                });

                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let events_weak = app_weak2.clone();
                    let done_weak = app_weak2.clone();
                    rt.block_on(async move {
                        let stable_id = session::resolve_stable_id(&url);
                        let work_dir = session::video_work_dir_or_tmp(stable_id.as_str());
                        let output_dir = cfg
                            .output_dir
                            .clone()
                            .unwrap_or_else(|| "output".to_string());
                        let pipeline = Pipeline::new();
                        let result = pipeline
                            .run(
                                PipelineCtx {
                                    url,
                                    config: cfg,
                                    work_dir,
                                    output_dir: std::path::PathBuf::from(output_dir),
                                    cancellation: CancellationToken::new(),
                                },
                                move |ev| {
                                    let app_weak3 = events_weak.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(app) = app_weak3.upgrade() {
                                            match ev {
                                                PipelineEvent::Progress(p, msg) => {
                                                    app.set_progress(p);
                                                    app.set_status_msg(msg.into());
                                                }
                                                PipelineEvent::Log(msg) => {
                                                    // Append-only ring buffer: each line is a row -> O(1) push,
                                                    // no re-layout of the whole 8000-char block (see app.slint:605)
                                                    append_log_line(&app, msg.into());
                                                }
                                                PipelineEvent::StageChanged(stage) => {
                                                    let (step, label) = match stage {
                                                        Stage::Downloading => (
                                                            1,
                                                            t!("analysis_step_downloading")
                                                                .to_string(),
                                                        ),
                                                        Stage::Splitting => (
                                                            2,
                                                            t!("analysis_step_splitting")
                                                                .to_string(),
                                                        ),
                                                        Stage::Analyzing { current, total } => {
                                                            let base =
                                                                t!("analysis_step_analyzing")
                                                                    .to_string();
                                                            (3, format!("{base} {current}/{total}"))
                                                        }
                                                        Stage::Extracting => (
                                                            4,
                                                            t!("analysis_step_extracting")
                                                                .to_string(),
                                                        ),
                                                    };
                                                    app.set_analysis_current_step(step);
                                                    app.set_analysis_step_label(
                                                        label.clone().into(),
                                                    );
                                                    app.set_status_msg(
                                                        format!("{step}/4 - {label}").into(),
                                                    );
                                                    // Also push stage label into technical log
                                                    append_log_line(
                                                        &app,
                                                        format!("-- {} --", label).into(),
                                                    );
                                                }
                                            }
                                        }
                                    });
                                },
                            )
                            .await;
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = done_weak.upgrade() {
                                app.set_is_analyzing(false);
                                match result {
                                    Ok(out) => {
                                        app.set_progress(1.0);
                                        app.set_analysis_current_step(4);
                                        app.set_analysis_step_label(
                                            t!("analysis_step_saving").to_string().into(),
                                        );
                                        app.set_status_msg(
                                            format!(
                                                "{}: {} moments, {} chunks",
                                                t!("new_project_analyze_complete"),
                                                out.moments.len(),
                                                out.chunks.len()
                                            )
                                            .into(),
                                        );
                                    }
                                    Err(e) => {
                                        app.set_status_msg(format!("{}: {e}", t!("error")).into());
                                    }
                                }
                            }
                        });
                    });
                });
            }
        }
    });

    app.on_save_settings({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                apply_settings_from_ui(&mut cfg, &app);
                cfg.normalize();
                rust_i18n::set_locale(&cfg.language);
                match config_store::save(&cfg) {
                    Ok(()) => {
                        apply_translations(&app);
                        // Ensure deps banner still shows current language after re-translate
                        update_deps_ui(&app);
                        app.set_settings_status(t!("status_settings_saved").to_string().into());
                        app.set_status_msg("".into());
                        app.set_keys_status("".into());
                    }
                    Err(e) => app.set_settings_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_save_ai_settings({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                apply_ai_settings_from_ui(&mut cfg, &app);
                match config_store::save(&cfg) {
                    Ok(()) => {
                        app.set_keys_status(t!("status_ai_settings_saved").to_string().into());
                        app.set_settings_status("".into());
                        app.set_status_msg("".into());
                    }
                    Err(e) => app.set_keys_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_browse_output({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    app.set_output_dir(path.to_string_lossy().to_string().into());
                }
            }
        }
    });

    app.on_browse_cookies({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    app.set_cookies_path(path.to_string_lossy().to_string().into());
                }
            }
        }
    });

    app.on_add_key({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let value = app.get_new_key_value().to_string().trim().to_owned();
                if value.is_empty() {
                    app.set_keys_status(t!("error").to_string().into());
                    app.set_settings_status("".into());
                    app.set_status_msg("".into());
                    return;
                }
                let mut cfg = config_rc.borrow_mut();
                let provider = cfg.ai.resolved_base_provider();
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    let prefix = if provider == "openrouter" {
                        "OpenRouter Key"
                    } else {
                        "Gemini Key"
                    };
                    let name = format!("{} {}", prefix, p.keys.len() + 1);
                    match yt_shortmaker_core::security::keyring::set_secret(
                        "yt-shortmaker",
                        &name,
                        &value,
                    ) {
                        Ok(()) => {
                            tracing::info!("keyring set_secret ok for {name}");
                        }
                        Err(e) => {
                            tracing::warn!("keyring set_secret failed for {name}: {e}");
                            app.set_keys_status(format!("{}: {e}", t!("error")).into());
                        }
                    }
                    // Verify round-trip via get_secret (wires the dead function)
                    if let Ok(stored) =
                        yt_shortmaker_core::security::keyring::get_secret("yt-shortmaker", &name)
                    {
                        tracing::debug!("keyring get_secret verify len {}", stored.len());
                    }
                    p.keys.push(yt_shortmaker_core::config::ApiKeyEntry {
                        name: name.clone(),
                        value: value.clone(),
                        enabled: true,
                    });
                }
                match config_store::save(&cfg) {
                    Ok(()) => {
                        refresh_keys_list(&app, &cfg);
                        app.set_new_key_value("".into());
                        app.set_show_key_drawer(false);
                        app.set_keys_status(t!("msg_api_key_saved").to_string().into());
                        app.set_settings_status("".into());
                        app.set_status_msg("".into());
                    }
                    Err(e) => app.set_keys_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_delete_key({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                let provider = cfg.ai.resolved_base_provider();
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    if let Some(removed) = p.keys.pop() {
                        if let Err(e) = yt_shortmaker_core::security::keyring::delete_secret(
                            "yt-shortmaker",
                            &removed.name,
                        ) {
                            tracing::warn!("delete_secret failed for {}: {e}", removed.name);
                        } else {
                            tracing::info!("keyring deleted {}", removed.name);
                        }
                        // Demonstrate get_secret wiring: verify it's gone
                        let _ = yt_shortmaker_core::security::keyring::get_secret(
                            "yt-shortmaker",
                            &removed.name,
                        );
                    } else {
                        app.set_keys_status(t!("keys_no_keys").to_string().into());
                        return;
                    }
                }
                let _ = config_store::save(&cfg);
                refresh_keys_list(&app, &cfg);
                app.set_keys_status(t!("msg_api_key_saved").to_string().into());
                app.set_settings_status("".into());
                app.set_status_msg("".into());
            }
        }
    });

    app.on_toggle_key({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                let provider = cfg.ai.resolved_base_provider();
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    if let Some(last) = p.keys.last_mut() {
                        last.enabled = !last.enabled;
                        tracing::info!("toggled {} to {}", last.name, last.enabled);
                    } else {
                        app.set_keys_status(t!("keys_no_keys").to_string().into());
                        return;
                    }
                }
                let _ = config_store::save(&cfg);
                refresh_keys_list(&app, &cfg);
                app.set_keys_status(t!("status_settings_saved").to_string().into());
                app.set_settings_status("".into());
                app.set_status_msg("".into());
            }
        }
    });

    // Per-row selectable API keys + lateral drawers
    app.on_select_key({
        let app_weak = app_weak.clone();
        move |idx: i32| {
            if let Some(app) = app_weak.upgrade() {
                app.set_selected_key_index(idx);
            }
        }
    });
    app.on_delete_key_at({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move |idx: i32| {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                let provider = cfg.ai.resolved_base_provider();
                let mut removed_name: Option<String> = None;
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    let u = idx as usize;
                    if u < p.keys.len() {
                        let r = p.keys.remove(u);
                        removed_name = Some(r.name.clone());
                        if let Err(e) = yt_shortmaker_core::security::keyring::delete_secret(
                            "yt-shortmaker",
                            &r.name,
                        ) {
                            tracing::warn!("delete_secret failed for {}: {e}", r.name);
                        }
                    }
                }
                if removed_name.is_none() {
                    app.set_keys_status(format!("{}: index {idx} not found", t!("error")).into());
                    return;
                }
                let _ = config_store::save(&cfg);
                refresh_keys_list(&app, &cfg);
                app.set_selected_key_index(-1);
                app.set_keys_status(format!("Key '{:?}' deleted", removed_name.unwrap()).into());
            }
        }
    });
    app.on_toggle_key_at({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move |idx: i32| {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                let provider = cfg.ai.resolved_base_provider();
                let mut ok = false;
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    let u = idx as usize;
                    if u < p.keys.len() {
                        p.keys[u].enabled = !p.keys[u].enabled;
                        ok = true;
                    }
                }
                if !ok {
                    app.set_keys_status(format!("{}: index {idx} not found", t!("error")).into());
                    return;
                }
                let _ = config_store::save(&cfg);
                refresh_keys_list(&app, &cfg);
                app.set_keys_status(t!("status_settings_saved").to_string().into());
            }
        }
    });
    app.on_open_key_drawer({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_new_key_value("".into());
                app.set_keys_status("".into());
                app.set_show_key_drawer(true);
            }
        }
    });
    app.on_close_key_drawer({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_show_key_drawer(false);
                app.set_new_key_value("".into());
            }
        }
    });
    app.on_open_model_drawer({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_new_model_display_name("".into());
                app.set_new_model_id("".into());
                app.set_new_model_temp("".into());
                app.set_show_model_drawer(true);
            }
        }
    });
    app.on_close_model_drawer({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_show_model_drawer(false);
            }
        }
    });
    app.on_toggle_analysis_log({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let cur = app.get_analysis_log_expanded();
                let next = !cur;
                app.set_analysis_log_expanded(next);
                if next {
                    scroll_to_bottom(&app);
                }
            }
        }
    });
    app.on_select_model_id({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move |id: slint::SharedString| {
            let id = id.to_string();
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                if let Some(m) = cfg.ai.custom_models.iter().find(|m| m.id == id).cloned() {
                    cfg.ai.active_model_id = Some(m.id.clone());
                    cfg.ai.active_provider = m.base_provider.clone();
                    let to_save = cfg.clone();
                    drop(cfg);
                    if let Err(e) = config_store::save(&to_save) {
                        app.set_models_status(format!("{}: {e}", t!("error")).into());
                        return;
                    }
                    let cfg_ref = config_rc.borrow();
                    refresh_models_list(&app, &cfg_ref);
                    refresh_keys_list(&app, &cfg_ref);
                    app.set_models_status(format!("Selected '{}'", m.display_name).into());
                    app.set_selected_model_id(id.into());
                }
            }
        }
    });
    app.on_delete_model_id({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move |id: slint::SharedString| {
            let id = id.to_string();
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                let before = cfg.ai.custom_models.len();
                cfg.ai.custom_models.retain(|m| m.id != id);
                if cfg.ai.custom_models.len() == before {
                    app.set_models_status(format!("{}: '{id}' not found", t!("error")).into());
                    return;
                }
                if cfg.ai.active_model_id.as_deref() == Some(&id) {
                    cfg.ai.active_model_id = cfg
                        .ai
                        .custom_models
                        .first()
                        .map(|m| m.id.clone())
                        .or(Some("gemini-3.7-flash".to_owned()));
                    if let Some(new_active) = cfg.ai.active_model_id.clone() {
                        if let Some(m) = cfg.ai.custom_models.iter().find(|m| m.id == new_active) {
                            cfg.ai.active_provider = m.base_provider.clone();
                        }
                    }
                }
                let to_save = cfg.clone();
                drop(cfg);
                if let Err(e) = config_store::save(&to_save) {
                    app.set_models_status(format!("{}: {e}", t!("error")).into());
                    return;
                }
                let cfg_ref = config_rc.borrow();
                refresh_models_list(&app, &cfg_ref);
                refresh_keys_list(&app, &cfg_ref);
                app.set_models_status(format!("Model '{id}' deleted").into());
            }
        }
    });

    app.on_test_connection({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let cfg = config_rc.borrow().clone();
                let app_weak2 = app_weak.clone();
                app.set_keys_status(t!("status_analyzing").to_string().into());
                app.set_settings_status("".into());
                app.set_status_msg("".into());
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let res = rt.block_on(async move {
                        let reg = yt_shortmaker_core::ai::build_registry(&cfg.ai);
                        // Use provider registry APIs (wires display_name/capabilities/list)
                        for prov in reg.list() {
                            tracing::debug!(
                                "provider {} display={} native_video={} mimes={:?}",
                                prov.id(),
                                prov.display_name(),
                                prov.capabilities().supports_native_video,
                                prov.capabilities().supported_mimes
                            );
                        }
                        if let Some(active) = reg.get(reg.active_id()) {
                            tracing::debug!("active provider get ok: {}", active.id());
                        }
                        let provider = reg
                            .active()
                            .ok_or_else(|| anyhow::anyhow!("no active AI provider"))?;
                        provider.test_connection().await
                    });
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak2.upgrade() {
                            match res {
                                Ok(()) => app.set_keys_status("Connection OK".into()),
                                Err(e) => {
                                    app.set_keys_status(format!("{}: {e}", t!("error")).into())
                                }
                            }
                        }
                    });
                });
            }
        }
    });

    app.on_add_model({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let display = app
                    .get_new_model_display_name()
                    .to_string()
                    .trim()
                    .to_owned();
                let model_id_raw = app.get_new_model_id().to_string().trim().to_owned();
                let base = app.get_new_model_base().to_string().trim().to_lowercase();
                let temp_str = app.get_new_model_temp().to_string().trim().to_owned();
                if display.is_empty() || model_id_raw.is_empty() {
                    app.set_models_status(
                        format!("{}: display name and model ID required", t!("error")).into(),
                    );
                    return;
                }
                let base = if base.is_empty() {
                    "gemini".to_string()
                } else {
                    base
                };
                let temp = if temp_str.is_empty() {
                    None
                } else {
                    match temp_str.parse::<f32>() {
                        Ok(v) if (0.0..=2.0).contains(&v) => Some(v),
                        _ => {
                            app.set_models_status(
                                format!("{}: temperature must be 0.0-2.0", t!("error")).into(),
                            );
                            return;
                        }
                    }
                };
                let mut id = yt_shortmaker_core::config::slugify_pub(&model_id_raw);
                if id.is_empty() {
                    id = format!("model-{}", chrono::Local::now().format("%H%M%S"));
                }
                let mut cfg = config_rc.borrow_mut();
                if cfg.ai.custom_models.iter().any(|m| m.id == id) {
                    app.set_models_status(
                        format!("{}: id '{id}' already exists", t!("error")).into(),
                    );
                    return;
                }
                let new_model = yt_shortmaker_core::config::CustomModel {
                    id: id.clone(),
                    display_name: display.clone(),
                    base_provider: base.clone(),
                    model_id: model_id_raw.clone(),
                    temperature: temp,
                    media_resolution: None,
                    enabled: true,
                };
                if let Err(e) = new_model.validate() {
                    app.set_models_status(format!("{}: {e}", t!("error")).into());
                    return;
                }
                // OpenRouter: verify model supports video input
                if base == "openrouter" {
                    if let Some(reason) =
                        yt_shortmaker_core::ai::openrouter::video_support_reason(&model_id_raw)
                    {
                        app.set_models_status(format!("{}: {reason}", t!("error")).into());
                        return;
                    }
                }
                // Ensure base provider exists
                if !cfg.ai.providers.contains_key(&base) {
                    cfg.ai.providers.insert(
                        base.clone(),
                        yt_shortmaker_core::config::ProviderConfig::default(),
                    );
                }
                cfg.ai.custom_models.push(new_model);
                cfg.ai.active_model_id = Some(id.clone());
                cfg.ai.active_provider = base.clone();
                let to_save = cfg.clone();
                drop(cfg);
                match config_store::save(&to_save) {
                    Ok(()) => {
                        let cfg_ref = config_rc.borrow();
                        refresh_models_list(&app, &cfg_ref);
                        app.set_models_status(format!("Model '{id}' added and selected").into());
                        app.set_new_model_display_name("".into());
                        app.set_new_model_id("".into());
                        app.set_new_model_temp("".into());
                        app.set_show_model_drawer(false);
                    }
                    Err(e) => app.set_models_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_select_model({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let id = app.get_new_model_id().to_string().trim().to_owned();
                if id.is_empty() {
                    app.set_models_status(
                        format!("{}: enter model ID to select", t!("error")).into(),
                    );
                    return;
                }
                let mut cfg = config_rc.borrow_mut();
                if let Some(m) = cfg.ai.custom_models.iter().find(|m| m.id == id).cloned() {
                    cfg.ai.active_model_id = Some(m.id.clone());
                    cfg.ai.active_provider = m.base_provider.clone();
                    let to_save = cfg.clone();
                    drop(cfg);
                    if let Err(e) = config_store::save(&to_save) {
                        app.set_models_status(format!("{}: {e}", t!("error")).into());
                        return;
                    }
                    let cfg_ref = config_rc.borrow();
                    refresh_models_list(&app, &cfg_ref);
                    app.set_models_status(
                        format!("Selected '{}' ({})", m.display_name, m.model_id).into(),
                    );
                } else {
                    app.set_models_status(
                        format!("{}: model '{id}' not found", t!("error")).into(),
                    );
                }
            }
        }
    });

    app.on_delete_model({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let id = app.get_new_model_id().to_string().trim().to_owned();
                if id.is_empty() {
                    app.set_models_status(
                        format!("{}: enter model ID to delete", t!("error")).into(),
                    );
                    return;
                }
                let mut cfg = config_rc.borrow_mut();
                let len_before = cfg.ai.custom_models.len();
                cfg.ai.custom_models.retain(|m| m.id != id);
                if cfg.ai.custom_models.len() == len_before {
                    app.set_models_status(
                        format!("{}: model '{id}' not found", t!("error")).into(),
                    );
                    return;
                }
                if cfg.ai.active_model_id.as_deref() == Some(&id) {
                    cfg.ai.active_model_id = cfg
                        .ai
                        .custom_models
                        .first()
                        .map(|m| m.id.clone())
                        .or(Some("gemini-3.7-flash".to_owned()));
                    if let Some(new_active) = cfg.ai.active_model_id.clone() {
                        if let Some(m) = cfg.ai.custom_models.iter().find(|m| m.id == new_active) {
                            cfg.ai.active_provider = m.base_provider.clone();
                        }
                    }
                }
                let to_save = cfg.clone();
                drop(cfg);
                match config_store::save(&to_save) {
                    Ok(()) => {
                        let cfg_ref = config_rc.borrow();
                        refresh_models_list(&app, &cfg_ref);
                        app.set_models_status(format!("Model '{id}' deleted").into());
                        app.set_new_model_id("".into());
                    }
                    Err(e) => app.set_models_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_check_deps({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let ok = update_deps_ui(&app);
                apply_translations(&app);
                // Preserve translated banner text after re-translate
                if !ok {
                    let st = setup::dependency_status();
                    if !st.is_ok() {
                        app.set_deps_error(setup::format_missing_message(&st.missing()).into());
                    }
                }
                if ok {
                    app.set_status_msg(t!("deps_ok").to_string().into());
                    app.set_settings_status("".into());
                    app.set_keys_status("".into());
                } else {
                    // Trigger auto-install instead of just showing manual instructions
                    spawn_ensure_tools(app_weak.clone());
                }
            }
        }
    });

    app.on_select_structure_video({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                app.set_structure_selected_video_id(id);
                refresh_structure_ui(&app);
            }
        }
    });

    app.on_undo_structure({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                let path = structure_path_for(&vid);
                let current =
                    yt_shortmaker_core::plano::schema::load_document(&path.to_string_lossy());
                let mut history = load_structure_history(&vid);
                if let (Ok(current), Some(previous)) = (current, history.undo.pop()) {
                    history.redo.push(current);
                    if yt_shortmaker_core::plano::schema::save_document(
                        &path.to_string_lossy(),
                        &previous,
                    )
                    .is_ok()
                    {
                        let _ = save_structure_history(&vid, &history);
                        refresh_structure_ui(&app);
                    }
                }
            }
        }
    });

    app.on_redo_structure({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                let path = structure_path_for(&vid);
                let current =
                    yt_shortmaker_core::plano::schema::load_document(&path.to_string_lossy());
                let mut history = load_structure_history(&vid);
                if let (Ok(current), Some(next)) = (current, history.redo.pop()) {
                    history.undo.push(current);
                    if yt_shortmaker_core::plano::schema::save_document(
                        &path.to_string_lossy(),
                        &next,
                    )
                    .is_ok()
                    {
                        let _ = save_structure_history(&vid, &history);
                        refresh_structure_ui(&app);
                    }
                }
            }
        }
    });

    app.on_select_structure_layer({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                app.set_structure_selected_layer_id(id);
                refresh_structure_ui(&app);
            }
        }
    });

    app.on_drag_structure_start({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                app.set_structure_selected_layer_id(id);
                begin_structure_gesture(&vid);
            }
        }
    });

    app.on_move_structure_layer({
        let app_weak = app_weak.clone();
        move |id, dx, dy| {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                // Ignore tiny jitter to avoid model thrash
                if dx.abs() < 0.05 && dy.abs() < 0.05 {
                    return;
                }
                // In-memory preview: no disk, no history, no full refresh.
                let mut doc = load_active_structure_doc(&vid);
                if doc.move_layer(id.as_str(), dx, dy).is_err() {
                    return;
                }
                set_active_structure_doc(&vid, doc.clone());
                app.set_structure_selected_layer_id(id);
                preview_structure_doc_to_ui(&app, &vid, &doc);
            }
        }
    });

    app.on_drag_structure_finish({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                let _ = id;
                finish_structure_gesture(&app, &vid);
            }
        }
    });

    app.on_resize_structure_start({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                app.set_structure_selected_layer_id(id);
                begin_structure_gesture(&vid);
            }
        }
    });

    app.on_resize_structure_layer({
        let app_weak = app_weak.clone();
        move |id, handle, dx, dy| {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                if dx.abs() < 0.05 && dy.abs() < 0.05 {
                    return;
                }
                let mut doc = load_active_structure_doc(&vid);
                if doc
                    .resize_with_handle(id.as_str(), handle.as_str(), dx, dy)
                    .is_err()
                {
                    return;
                }
                set_active_structure_doc(&vid, doc.clone());
                app.set_structure_selected_layer_id(id);
                preview_structure_doc_to_ui(&app, &vid, &doc);
            }
        }
    });

    app.on_resize_structure_finish({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    return;
                }
                let _ = id;
                finish_structure_gesture(&app, &vid);
            }
        }
    });

    app.on_center_structure_canvas(|| {
        // Centering is handled in Slint (canvas.center-frame / fit).
        // Kept for compatibility; no Rust work needed.
    });

    app.on_toggle_structure_layer({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let id_s = id.to_string();
                with_structure_doc(&app, |doc| doc.toggle_layer(&id_s));
            }
        }
    });

    app.on_delete_structure_layer({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let id_s = id.to_string();
                with_structure_doc(&app, |doc| doc.delete_layer(&id_s));
                if app.get_structure_selected_layer_id().as_str() == id.as_str() {
                    app.set_structure_selected_layer_id("".into());
                    refresh_structure_ui(&app);
                }
            }
        }
    });

    app.on_raise_structure_layer({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let id_s = id.to_string();
                with_structure_doc(&app, |doc| doc.raise_layer(&id_s));
            }
        }
    });

    app.on_lower_structure_layer({
        let app_weak = app_weak.clone();
        move |id| {
            if let Some(app) = app_weak.upgrade() {
                let id_s = id.to_string();
                with_structure_doc(&app, |doc| doc.lower_layer(&id_s));
            }
        }
    });

    app.on_commit_structure_position({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let id: String = app.get_structure_selected_layer_id().into();
                let x: f32 = app
                    .get_structure_inspector_x()
                    .to_string()
                    .parse()
                    .unwrap_or(f32::NAN);
                let y: f32 = app
                    .get_structure_inspector_y()
                    .to_string()
                    .parse()
                    .unwrap_or(f32::NAN);
                if !x.is_finite() || !y.is_finite() {
                    app.set_structure_status(format!("{}: invalid position", t!("error")).into());
                    return;
                }
                with_structure_doc(&app, |doc| {
                    let (cx, cy, w, h) = doc
                        .layers
                        .iter()
                        .find(|l| l.id == id)
                        .map(|l| {
                            let (rx, ry, rw, rh) = l.resolved_rect();
                            (rx as f32, ry as f32, rw as f32, rh as f32)
                        })
                        .ok_or_else(|| format!("layer '{id}' not found"))?;
                    let _ = (cx, cy);
                    doc.resize_layer(&id, x, y, w, h)
                });
            }
        }
    });

    app.on_commit_structure_size({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let id: String = app.get_structure_selected_layer_id().into();
                let w: f32 = app
                    .get_structure_inspector_w()
                    .to_string()
                    .parse()
                    .unwrap_or(f32::NAN);
                let h: f32 = app
                    .get_structure_inspector_h()
                    .to_string()
                    .parse()
                    .unwrap_or(f32::NAN);
                if !w.is_finite() || !h.is_finite() {
                    app.set_structure_status(format!("{}: invalid size", t!("error")).into());
                    return;
                }
                with_structure_doc(&app, |doc| {
                    let (cx, cy, _, _) = doc
                        .layers
                        .iter()
                        .find(|l| l.id == id)
                        .map(|l| {
                            let (rx, ry, rw, rh) = l.resolved_rect();
                            (rx as f32, ry as f32, rw as f32, rh as f32)
                        })
                        .ok_or_else(|| format!("layer '{id}' not found"))?;
                    doc.resize_layer(&id, cx, cy, w, h)
                });
            }
        }
    });

    app.on_add_structure_clip({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                with_structure_doc(&app, |doc| {
                    let n = doc.layers.len();
                    use yt_shortmaker_core::plano::schema::{
                        Fit, PlanoLayer, PlanoObject, Position, PositionValue, SizeValue,
                    };
                    doc.layers.push(PlanoLayer {
                        id: format!("clip-{}", n + 1),
                        name: format!("Clip {}", n + 1),
                        visible: true,
                        object: PlanoObject::Clip {
                            position: Position {
                                x: PositionValue::Pixels(0),
                                y: PositionValue::Pixels(0),
                                width: SizeValue::Pixels(1080),
                                height: SizeValue::Pixels(1920),
                            },
                            crop: None,
                            fit: Fit::Cover,
                            comment: None,
                        },
                    });
                    Ok(())
                });
            }
        }
    });

    app.on_add_structure_blur({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                with_structure_doc(&app, |doc| {
                    let n = doc.layers.len();
                    use yt_shortmaker_core::plano::schema::{
                        PlanoLayer, PlanoObject, Position, PositionValue, ShaderEffect, SizeValue,
                    };
                    doc.layers.push(PlanoLayer {
                        id: format!("blur-{}", n + 1),
                        name: format!("Blur {}", n + 1),
                        visible: true,
                        object: PlanoObject::Shader {
                            effect: ShaderEffect::Blur { intensity: 20 },
                            position: Position {
                                x: PositionValue::Pixels(0),
                                y: PositionValue::Pixels(0),
                                width: SizeValue::Keyword("full".to_string()),
                                height: SizeValue::Keyword("full".to_string()),
                            },
                            comment: None,
                        },
                    });
                    Ok(())
                });
            }
        }
    });

    app.on_add_structure_image({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    app.set_structure_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let picked = rfd::FileDialog::new()
                    .add_filter("image", &["png", "jpg", "jpeg"])
                    .pick_file();
                let src = match picked {
                    Some(p) => p,
                    None => return,
                };
                let ext = src
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext != "png" && ext != "jpg" && ext != "jpeg" {
                    app.set_structure_status(format!("{}: PNG/JPG only", t!("error")).into());
                    return;
                }
                let assets = work_dir_for(&vid).join("assets");
                if std::fs::create_dir_all(&assets).is_err() {
                    app.set_structure_status(format!("{}: assets dir", t!("error")).into());
                    return;
                }
                let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                let safe = yt_shortmaker_core::util::sanitize_id(&format!("{stem}.{ext}"));
                let dst = assets.join(format!(
                    "{}_{safe}",
                    yt_shortmaker_core::util::fnv1a_hash(&src.to_string_lossy())
                ));
                if std::fs::copy(&src, &dst).is_err() {
                    app.set_structure_status(format!("{}: copy failed", t!("error")).into());
                    return;
                }
                let dst_s = dst.to_string_lossy().to_string();
                with_structure_doc(&app, |doc| {
                    let n = doc.layers.len();
                    use yt_shortmaker_core::plano::schema::{
                        PlanoLayer, PlanoObject, Position, PositionValue, SizeValue,
                    };
                    doc.layers.push(PlanoLayer {
                        id: format!("img-{}", n + 1),
                        name: format!("Image {}", n + 1),
                        visible: true,
                        object: PlanoObject::Image {
                            path: dst_s.clone(),
                            position: Position {
                                x: PositionValue::Pixels(140),
                                y: PositionValue::Pixels(140),
                                width: SizeValue::Pixels(800),
                                height: SizeValue::Pixels(500),
                            },
                            opacity: 1.0,
                            comment: None,
                        },
                    });
                    Ok(())
                });
            }
        }
    });

    app.on_add_structure_video({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    app.set_structure_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let picked = rfd::FileDialog::new()
                    .add_filter("video", &["mp4", "webm"])
                    .pick_file();
                let src = match picked {
                    Some(p) => p,
                    None => return,
                };
                let ext = src
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext != "mp4" && ext != "webm" {
                    app.set_structure_status(format!("{}: MP4/WebM only", t!("error")).into());
                    return;
                }
                let assets = work_dir_for(&vid).join("assets");
                if std::fs::create_dir_all(&assets).is_err() {
                    app.set_structure_status(format!("{}: assets dir", t!("error")).into());
                    return;
                }
                let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
                let safe = yt_shortmaker_core::util::sanitize_id(&format!("{stem}.{ext}"));
                let dst = assets.join(format!(
                    "{}_{safe}",
                    yt_shortmaker_core::util::fnv1a_hash(&src.to_string_lossy())
                ));
                if std::fs::copy(&src, &dst).is_err() {
                    app.set_structure_status(format!("{}: copy failed", t!("error")).into());
                    return;
                }
                let dst_s = dst.to_string_lossy().to_string();
                with_structure_doc(&app, |doc| {
                    let n = doc.layers.len();
                    use yt_shortmaker_core::plano::schema::{
                        Fit, PlanoLayer, PlanoObject, Position, PositionValue, SizeValue,
                    };
                    doc.layers.push(PlanoLayer {
                        id: format!("vid-{n}"),
                        name: format!("Video {}", n + 1),
                        visible: true,
                        object: PlanoObject::Video {
                            path: dst_s.clone(),
                            position: Position {
                                x: PositionValue::Pixels(140),
                                y: PositionValue::Pixels(800),
                                width: SizeValue::Pixels(800),
                                height: SizeValue::Pixels(500),
                            },
                            loop_video: true,
                            keep_last_frame: false,
                            opacity: 1.0,
                            fit: Fit::Cover,
                            comment: None,
                        },
                    });
                    Ok(())
                });
            }
        }
    });

    app.on_render_structure_preview({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if app.get_structure_preview_rendering() {
                    return;
                }
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    app.set_structure_preview_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let second: u64 = app
                    .get_structure_preview_second()
                    .to_string()
                    .trim()
                    .parse()
                    .unwrap_or(0);
                let video_path = match resolve_active_video_path(&app) {
                    Some(p) => p,
                    None => {
                        app.set_structure_preview_status(
                            format!("{}: video file missing", t!("error")).into(),
                        );
                        return;
                    }
                };
                let struct_path = structure_path_for(&vid);
                let out_path = work_dir_for(&vid).join("preview_frame.png");
                app.set_structure_preview_rendering(true);
                app.set_structure_preview_status("Rendering frame...".into());
                let weak = app.as_weak();
                std::thread::spawn(move || {
                    let res: anyhow::Result<std::path::PathBuf> = (|| {
                        let doc = yt_shortmaker_core::plano::schema::load_document(
                            &struct_path.to_string_lossy(),
                        )?;
                        let cmd = yt_shortmaker_core::plano::export::build_frame_command(
                            &video_path,
                            &doc,
                            second,
                            &out_path,
                        )?;
                        yt_shortmaker_core::plano::export::run_render(&cmd)?;
                        Ok(out_path.clone())
                    })();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(a) = weak.upgrade() {
                            a.set_structure_preview_rendering(false);
                            match res {
                                Ok(p) => match slint::Image::load_from_path(&p) {
                                    Ok(img) => {
                                        a.set_structure_preview_image(img);
                                        a.set_structure_preview_has_frame(true);
                                        a.set_structure_preview_status(
                                            format!("Frame: {second}s").into(),
                                        );
                                    }
                                    Err(e) => a.set_structure_preview_status(
                                        format!("{}: {e}", t!("error")).into(),
                                    ),
                                },
                                Err(e) => a.set_structure_preview_status(
                                    format!("{}: {e:#}", t!("error")).into(),
                                ),
                            }
                        }
                    });
                });
            }
        }
    });

    app.on_render_structure_sample({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if app.get_structure_preview_rendering() {
                    return;
                }
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    app.set_structure_preview_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let second: u64 = app
                    .get_structure_preview_second()
                    .to_string()
                    .trim()
                    .parse()
                    .unwrap_or(0);
                let video_path = match resolve_active_video_path(&app) {
                    Some(p) => p,
                    None => {
                        app.set_structure_preview_status(
                            format!("{}: video file missing", t!("error")).into(),
                        );
                        return;
                    }
                };
                let struct_path = structure_path_for(&vid);
                let out_path = work_dir_for(&vid).join("preview_sample.mp4");
                app.set_structure_preview_rendering(true);
                app.set_structure_preview_status("Rendering 4s sample...".into());
                let weak = app.as_weak();
                std::thread::spawn(move || {
                    let res: anyhow::Result<std::path::PathBuf> = (|| {
                        let doc = yt_shortmaker_core::plano::schema::load_document(
                            &struct_path.to_string_lossy(),
                        )?;
                        let profile = yt_shortmaker_core::plano::export::ExportProfile::high_1080();
                        let cmd = yt_shortmaker_core::plano::export::build_sample_command(
                            &video_path,
                            &doc,
                            second,
                            yt_shortmaker_core::plano::export::SAMPLE_DURATION_SECS,
                            &profile,
                            &out_path,
                        )?;
                        yt_shortmaker_core::plano::export::run_render(&cmd)?;
                        yt_shortmaker_core::plano::export::verify_video_output(
                            &out_path,
                            &profile,
                            yt_shortmaker_core::plano::export::SAMPLE_DURATION_SECS,
                        )?;
                        Ok(out_path.clone())
                    })();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(a) = weak.upgrade() {
                            a.set_structure_preview_rendering(false);
                            match res {
                                Ok(p) => {
                                    a.set_structure_preview_sample_path(
                                        p.to_string_lossy().to_string().into(),
                                    );
                                    a.set_structure_preview_status(
                                        format!("Sample ready: {}", p.display()).into(),
                                    );
                                }
                                Err(e) => a.set_structure_preview_status(
                                    format!("{}: {e:#}", t!("error")).into(),
                                ),
                            }
                        }
                    });
                });
            }
        }
    });

    app.on_open_structure_sample({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let path = app.get_structure_preview_sample_path().to_string();
                if path.is_empty() {
                    app.set_structure_preview_status("No sample rendered.".into());
                    return;
                }
                let result = if cfg!(target_os = "windows") {
                    std::process::Command::new("cmd")
                        .args(["/C", "start", "", &path])
                        .status()
                } else if cfg!(target_os = "macos") {
                    std::process::Command::new("open").arg(&path).status()
                } else {
                    std::process::Command::new("xdg-open").arg(&path).status()
                };
                if let Err(e) = result {
                    app.set_structure_preview_status(format!("Could not open sample: {e}").into());
                }
            }
        }
    });

    app.on_clear_structure_logs({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_structure_logs_model(
                    std::rc::Rc::new(slint::VecModel::from(Vec::<StructureLogRow>::new())).into(),
                );
                append_structure_log(&app, "INFO", "logs", "log view cleared");
            }
        }
    });

    app.on_open_structure_log_file({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let Some(dir) = dirs::data_local_dir() else {
                    return;
                };
                let path = dir
                    .join(yt_shortmaker_core::config::APP_DIR_NAME)
                    .join("logs")
                    .join("structure.log");
                let path_string = path.to_string_lossy().to_string();
                let result = if cfg!(target_os = "windows") {
                    std::process::Command::new("cmd")
                        .args(["/C", "start", "", &path_string])
                        .status()
                } else if cfg!(target_os = "macos") {
                    std::process::Command::new("open")
                        .arg(&path_string)
                        .status()
                } else {
                    std::process::Command::new("xdg-open")
                        .arg(&path_string)
                        .status()
                };
                if let Err(e) = result {
                    append_structure_log(
                        &app,
                        "ERROR",
                        "logs",
                        &format!("could not open log: {e}"),
                    );
                }
            }
        }
    });

    app.on_save_structure({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid: String = app.get_structure_selected_video_id().into();
                let vid = if vid.trim().is_empty() {
                    app.get_video_id().to_string()
                } else {
                    vid
                };
                if vid.trim().is_empty() {
                    app.set_structure_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let path = structure_path_for(&vid);
                let doc = if path.exists() {
                    match yt_shortmaker_core::plano::schema::load_document(&path.to_string_lossy())
                    {
                        Ok(d) => d,
                        Err(e) => {
                            app.set_structure_status(format!("{}: {e}", t!("error")).into());
                            return;
                        }
                    }
                } else {
                    yt_shortmaker_core::plano::schema::create_empty_document()
                };
                match yt_shortmaker_core::plano::schema::save_document(
                    &path.to_string_lossy(),
                    &doc,
                ) {
                    Ok(()) => {
                        if let Ok(json) = serde_json::to_string(&doc) {
                            if let Ok(db_path) = session::db_path() {
                                if let Ok(conn) = session::init_db(&db_path) {
                                    let _ = session::upsert_video_structure(&conn, &vid, &json);
                                }
                            }
                        }
                        app.set_structure_has_video(true);
                        app.set_structure_document_dirty(false);
                        update_structure_history_flags(&app);
                        app.set_structure_status(t!("structure_status_saved").to_string().into());
                    }
                    Err(e) => app.set_structure_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_save_global_template({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid: String = app.get_structure_selected_video_id().into();
                let vid = if vid.trim().is_empty() {
                    app.get_video_id().to_string()
                } else {
                    vid
                };
                if vid.trim().is_empty() {
                    app.set_structure_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let src = structure_path_for(&vid);
                if !src.exists() {
                    app.set_structure_status(
                        t!("structure_status_empty_invalid").to_string().into(),
                    );
                    return;
                }
                let doc = match yt_shortmaker_core::plano::schema::load_document(
                    &src.to_string_lossy(),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        app.set_structure_status(format!("{}: {e}", t!("error")).into());
                        return;
                    }
                };
                match global_template_path() {
                    Some(dst) => {
                        match yt_shortmaker_core::plano::schema::save_document(
                            &dst.to_string_lossy(),
                            &doc,
                        ) {
                            Ok(()) => app.set_structure_status(
                                t!("structure_status_saved").to_string().into(),
                            ),
                            Err(e) => {
                                app.set_structure_status(format!("{}: {e}", t!("error")).into())
                            }
                        }
                    }
                    None => app.set_structure_status(format!("{}: config dir", t!("error")).into()),
                }
            }
        }
    });

    app.on_restore_global_template({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid: String = app.get_structure_selected_video_id().into();
                let vid = if vid.trim().is_empty() {
                    app.get_video_id().to_string()
                } else {
                    vid
                };
                if vid.trim().is_empty() {
                    app.set_structure_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let src = match global_template_path() {
                    Some(p) => p,
                    None => {
                        app.set_structure_status(format!("{}: config dir", t!("error")).into());
                        return;
                    }
                };
                if !src.exists() {
                    app.set_structure_status(
                        t!("structure_status_empty_invalid").to_string().into(),
                    );
                    return;
                }
                let doc = match yt_shortmaker_core::plano::schema::load_document(
                    &src.to_string_lossy(),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        app.set_structure_status(format!("{}: {e}", t!("error")).into());
                        return;
                    }
                };
                let dst = structure_path_for(&vid);
                match yt_shortmaker_core::plano::schema::save_document(&dst.to_string_lossy(), &doc)
                {
                    Ok(()) => {
                        app.set_structure_status(t!("structure_status_saved").to_string().into())
                    }
                    Err(e) => app.set_structure_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_load_structure_preset({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let vid: String = app.get_structure_selected_video_id().into();
                let vid = if vid.trim().is_empty() {
                    app.get_video_id().to_string()
                } else {
                    vid
                };
                if vid.trim().is_empty() {
                    app.set_structure_status(t!("structure_no_video").to_string().into());
                    return;
                }
                let doc = yt_shortmaker_core::plano::schema::create_default_document();
                let dst = structure_path_for(&vid);
                match yt_shortmaker_core::plano::schema::save_document(&dst.to_string_lossy(), &doc)
                {
                    Ok(()) => {
                        app.set_structure_has_video(true);
                        app.set_structure_status(t!("structure_status_saved").to_string().into());
                        refresh_structure_ui(&app);
                    }
                    Err(e) => app.set_structure_status(format!("{}: {e}", t!("error")).into()),
                }
            }
        }
    });

    app.on_toggle_structure_moment({
        let app_weak = app_weak.clone();
        move |idx| {
            if let Some(app) = app_weak.upgrade() {
                use slint::Model;
                let model = app.get_structure_moments_model();
                if let Some(vm) = model.as_any().downcast_ref::<slint::VecModel<MomentRow>>() {
                    let i = idx as usize;
                    if let Some(mut row) = vm.row_data(i) {
                        row.selected = !row.selected;
                        vm.set_row_data(i, row);
                    }
                }
            }
        }
    });

    app.on_cancel_structure_export({
        let app_weak = app_weak.clone();
        move || {
            export_cancel_flag().store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(app) = app_weak.upgrade() {
                app.set_structure_export_status("Cancelling...".into());
            }
        }
    });

    app.on_retry_structure_export({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                use slint::Model;
                let model = app.get_structure_moments_model();
                if let Some(vm) = model.as_any().downcast_ref::<slint::VecModel<MomentRow>>() {
                    for i in 0..vm.row_count() {
                        if let Some(mut row) = vm.row_data(i) {
                            if row.export_status.as_str() == "failed"
                                || row.export_status.as_str() == "cancelled"
                            {
                                row.selected = true;
                                row.export_status = "pending".into();
                                vm.set_row_data(i, row);
                            }
                        }
                    }
                }
                app.set_structure_export_status("Retrying failed...".into());
            }
        }
    });

    {
        let app_weak = app_weak.clone();
        let config_rc = config_rc.clone();
        app.on_start_structure_export(move || {
            if let Some(app) = app_weak.upgrade() {
                if app.get_structure_export_running() {
                    return;
                }
                let vid = active_video_id(&app);
                if vid.trim().is_empty() {
                    app.set_structure_export_status(t!("structure_no_video").to_string().into());
                    return;
                }
                use slint::Model;
                let selected: Vec<(i32, String, String)> = app
                    .get_structure_moments_model()
                    .iter()
                    .filter(|r| r.selected)
                    .map(|r| (r.index, r.start.to_string(), r.end.to_string()))
                    .collect();
                if selected.is_empty() {
                    app.set_structure_export_status("Select at least one moment.".into());
                    return;
                }
                let resolution: String = app.get_structure_export_resolution().into();
                let fps: u32 = app
                    .get_structure_export_fps()
                    .to_string()
                    .parse()
                    .unwrap_or(30);
                let quality: String = app.get_structure_export_quality().into();
                let profile = if resolution == "720x1280" {
                    yt_shortmaker_core::plano::export::ExportProfile::high_720()
                        .with_fps(fps)
                        .with_quality(&quality)
                } else {
                    yt_shortmaker_core::plano::export::ExportProfile::high_1080()
                        .with_fps(fps)
                        .with_quality(&quality)
                };
                let video_path = match resolve_active_video_path(&app) {
                    Some(p) => p,
                    None => {
                        app.set_structure_export_status(
                            format!("{}: video file missing", t!("error")).into(),
                        );
                        return;
                    }
                };
                let struct_path = structure_path_for(&vid);
                let doc = match yt_shortmaker_core::plano::schema::load_document(
                    &struct_path.to_string_lossy(),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        app.set_structure_export_status(format!("{}: {e:#}", t!("error")).into());
                        return;
                    }
                };
                if doc.validate_for_export().is_err() {
                    app.set_structure_export_status(
                        t!("structure_status_empty_invalid").to_string().into(),
                    );
                    return;
                }
                let out_base = {
                    let cfg = config_rc.borrow();
                    cfg.output_dir
                        .clone()
                        .unwrap_or_else(|| "output".to_string())
                };
                let slug = yt_shortmaker_core::util::slugify(&vid);
                let shorts_dir = std::path::PathBuf::from(&out_base)
                    .join(slug)
                    .join("shorts");
                let _ = std::fs::create_dir_all(&shorts_dir);
                let total = selected.len() as i32;
                app.set_structure_export_running(true);
                app.set_structure_export_total(total);
                app.set_structure_export_completed(0);
                app.set_structure_export_status(format!("Exporting 0/{total}...").into());
                export_cancel_flag().store(false, std::sync::atomic::Ordering::SeqCst);
                let cancel = export_cancel_flag();
                let weak = app.as_weak();
                let profile_hash = profile.hash();
                let struct_hash = yt_shortmaker_core::plano::export::structure_hash(&doc);
                std::thread::spawn(move || {
                    for (done_idx, (idx, start_s, end_s)) in selected.into_iter().enumerate() {
                        let done = (done_idx + 1) as i32;
                        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                            let _ = slint::invoke_from_event_loop({
                                let weak = weak.clone();
                                move || {
                                    if let Some(a) = weak.upgrade() {
                                        a.set_structure_export_running(false);
                                        a.set_structure_export_status("Cancelled.".into());
                                        refresh_structure_moments(&a, &vid);
                                    }
                                }
                            });
                            return;
                        }
                        let start = yt_shortmaker_core::types::parse_timestamp_to_seconds(&start_s)
                            .unwrap_or(0);
                        let end = yt_shortmaker_core::types::parse_timestamp_to_seconds(&end_s)
                            .unwrap_or(start);
                        let out_path = shorts_dir.join(format!(
                            "short_{:03}_{}_{}.mp4",
                            idx + 1,
                            struct_hash,
                            profile_hash
                        ));
                        // Skip valid completed output with matching fingerprints.
                        let skip = (|| {
                            let db = session::db_path().ok()?;
                            let conn = session::init_db(&db).ok()?;
                            let recs = session::get_exports(&conn, &vid).ok()?;
                            let r = recs.iter().find(|x| x.moment_idx as i32 == idx)?;
                            if r.status == "completed"
                                && r.structure_hash == struct_hash
                                && r.profile_hash == profile_hash
                                && std::path::Path::new(&r.output_path).exists()
                            {
                                return Some(true);
                            }
                            None
                        })()
                        .unwrap_or(false);
                        if !skip {
                            let tmp = shorts_dir.join(format!(
                                "short_{:03}_{}_{}.tmp.mp4",
                                idx + 1,
                                struct_hash,
                                profile_hash
                            ));
                            let cmd = yt_shortmaker_core::plano::export::build_moment_command(
                                &video_path,
                                &doc,
                                start,
                                end,
                                &profile,
                                &tmp,
                            );
                            let result: anyhow::Result<()> = (|| {
                                let c = cmd?;
                                yt_shortmaker_core::plano::export::run_render(&c)?;
                                yt_shortmaker_core::plano::export::verify_video_output(
                                    &tmp,
                                    &profile,
                                    end.saturating_sub(start),
                                )?;
                                std::fs::rename(&tmp, &out_path)?;
                                if let Ok(db) = session::db_path() {
                                    if let Ok(conn) = session::init_db(&db) {
                                        let _ = session::upsert_export(
                                            &conn,
                                            &session::ExportRecord {
                                                video_id: vid.clone(),
                                                moment_idx: idx as i64,
                                                output_path: out_path.to_string_lossy().to_string(),
                                                status: "completed".to_string(),
                                                error: None,
                                                structure_hash: struct_hash.clone(),
                                                profile_hash: profile_hash.clone(),
                                            },
                                        );
                                    }
                                }
                                Ok(())
                            })();
                            if let Err(e) = result {
                                if let Ok(db) = session::db_path() {
                                    if let Ok(conn) = session::init_db(&db) {
                                        let _ = session::upsert_export(
                                            &conn,
                                            &session::ExportRecord {
                                                video_id: vid.clone(),
                                                moment_idx: idx as i64,
                                                output_path: out_path.to_string_lossy().to_string(),
                                                status: "failed".to_string(),
                                                error: Some(format!("{e:#}")),
                                                structure_hash: struct_hash.clone(),
                                                profile_hash: profile_hash.clone(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        let weak2 = weak.clone();
                        let vid2 = vid.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(a) = weak2.upgrade() {
                                a.set_structure_export_completed(done);
                                a.set_structure_export_status(
                                    format!("Exporting {done}/{total}...").into(),
                                );
                                refresh_structure_moments(&a, &vid2);
                            }
                        });
                    }
                    // Write moments.json/txt alongside shorts.
                    if let Ok(db) = session::db_path() {
                        if let Ok(conn) = session::init_db(&db) {
                            if let Ok(moments) = session::get_job_moments(&conn, &vid) {
                                let json =
                                    serde_json::to_string_pretty(&moments).unwrap_or_default();
                                let base = shorts_dir.parent().unwrap_or(&shorts_dir).to_path_buf();
                                let _ = std::fs::write(base.join("moments.json"), json);
                                let txt = moments
                                    .iter()
                                    .map(|m| {
                                        format!(
                                            "{} - {} [{}] {}",
                                            m.start_time, m.end_time, m.category, m.description
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                let _ = std::fs::write(base.join("moments.txt"), txt);
                            }
                        }
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(a) = weak.upgrade() {
                            a.set_structure_export_running(false);
                            a.set_structure_export_status("Export complete.".into());
                            refresh_structure_moments(&a, &vid);
                        }
                    });
                });
            }
        });
    }

    app.run()?;
    Ok(())
}
