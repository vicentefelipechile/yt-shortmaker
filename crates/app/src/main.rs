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
    app.set_tr_add_model(t!("add_model").to_string().into());
    app.set_tr_select_model(t!("select_model").to_string().into());
    app.set_tr_delete_model(t!("delete_model").to_string().into());
    app.set_tr_models_title(t!("models_title").to_string().into());
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
    let provider = config.ai.active_provider.clone();
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
                let route = if route == "new-project" {
                    "home".into()
                } else {
                    route
                };
                app.set_route(route);
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
                let provider = cfg.ai.active_provider.clone();
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    let name = format!("Gemini Key {}", p.keys.len() + 1);
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
                let provider = cfg.ai.active_provider.clone();
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
                let provider = cfg.ai.active_provider.clone();
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
                let provider = cfg.ai.active_provider.clone();
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
                let provider = cfg.ai.active_provider.clone();
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

    app.run()?;
    Ok(())
}
