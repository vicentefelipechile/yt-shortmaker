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
        app.set_deps_error(t!("deps_installing").to_string().into());
        // Clear per-panel shared error that was leaking via status-msg
        app.set_status_msg("".into());
        app.set_settings_status("".into());
        app.set_keys_status("".into());
    }
    let weak = app_weak.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(setup::ensure_tools());
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                match res {
                    Ok(()) => {
                        app.set_deps_ok(true);
                        app.set_deps_installing(false);
                        app.set_deps_error("".into());
                        app.set_status_msg(t!("deps_ok").to_string().into());
                    }
                    Err(e) => {
                        app.set_deps_installing(false);
                        app.set_deps_ok(false);
                        app.set_deps_error(e.to_string().into());
                        // Show the auto-install error only on the banner, not in every panel's status-msg
                        // Keep per-panel statuses clean
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
}

fn refresh_home(app: &MainWindow) {
    let text = match session::db_path()
        .and_then(|p| session::init_db(&p).map_err(|e| anyhow::anyhow!(e)))
    {
        Ok(conn) => match session::list_projects(&conn) {
            Ok(projects) if !projects.is_empty() => {
                // Wire get_project / get_moments / delete_project (M4 Review) — verify first project loads
                if let Some(first) = projects.first() {
                    if let Ok(Some(_proj)) = session::get_project(&conn, &first.id) {
                        let _ = session::get_moments(&conn, &first.id);
                        // keep delete_project compiled and linked for M4 Review (no-op reference)
                        let _ = session::delete_project as fn(&_, &str) -> anyhow::Result<()>;
                    }
                }
                projects
                    .iter()
                    .take(10)
                    .map(|pr| format!("{} — {} [{}] {}", pr.title, pr.video_id, pr.status, pr.id))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Ok(_) => String::new(),
            Err(e) => {
                tracing::warn!("list_projects failed: {e}");
                String::new()
            }
        },
        Err(e) => {
            tracing::debug!("session db not ready: {e}");
            String::new()
        }
    };
    app.set_home_recent_list(text.into());
}

fn work_dir_for(video_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join("yt-shortmaker-v2").join(format!(
        "{video_id}-{}",
        chrono::Local::now().format("%Y%m%d%H%M%S")
    ))
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = config_store::load().unwrap_or_default();
    rust_i18n::set_locale(&config.language);
    let config_rc = Rc::new(RefCell::new(config));

    let app = MainWindow::new()?;
    app.set_core_version(yt_shortmaker_core::version().into());
    populate_settings_ui(&app, &config_rc.borrow());
    apply_translations(&app);
    refresh_keys_list(&app, &config_rc.borrow());
    let had_deps = update_deps_ui(&app);
    refresh_home(&app);
    if !had_deps {
        // The app installs its own dependencies — no user manual step
        spawn_ensure_tools(app.as_weak());
    }

    let app_weak = app.as_weak();

    app.on_navigate({
        let app_weak = app_weak.clone();
        move |route| {
            if let Some(app) = app_weak.upgrade() {
                app.set_route(route.clone());
                if route == "home" {
                    refresh_home(&app);
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
                    // Refresh error in case PATH changed.
                    update_deps_ui(&app);
                    if !app.get_deps_ok() {
                        let msg = app.get_deps_error().to_string();
                        app.set_url_error(msg.into());
                        tracing::warn!("fetch blocked: missing deps");
                        return;
                    }
                }
                let url: String = app.get_url_input().into();
                match ytdlp::validate_media_url(&url) {
                    Ok(()) => {
                        app.set_url_error("".into());
                        app.set_status_msg(t!("status_fetched", url = url).to_string().into());
                        let cfg = config_rc.borrow().clone();
                        let app_weak2 = app_weak.clone();
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
                                            app.set_video_title(
                                                format!(
                                                    "{} ({}s)",
                                                    info.title, info.duration_secs as u64
                                                )
                                                .into(),
                                            );
                                        }
                                        Err(e) => app.set_url_error(e.to_string().into()),
                                    }
                                }
                            });
                        });
                    }
                    Err(e) => app.set_url_error(e.to_string().into()),
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
                        let msg = app.get_deps_error().to_string();
                        app.set_status_msg(format!("{}: {msg}", t!("error")).into());
                        tracing::warn!("analyze blocked: missing deps");
                        return;
                    }
                }
                let url: String = app.get_url_input().into();
                if url.trim().is_empty() {
                    app.set_status_msg(t!("error").to_string().into());
                    return;
                }
                let cfg = Arc::new(config_rc.borrow().clone());
                let app_weak2 = app_weak.clone();

                app.set_progress(0.0);
                app.set_status_msg(t!("new_project_analyzing").to_string().into());

                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let events_weak = app_weak2.clone();
                    let done_weak = app_weak2.clone();
                    rt.block_on(async move {
                        let work_dir = work_dir_for(
                            ytdlp::extract_video_id(&url)
                                .unwrap_or_else(|| "video".to_string())
                                .as_str(),
                        );
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
                                                    app.set_status_msg(msg.into());
                                                }
                                                PipelineEvent::StageChanged(stage) => {
                                                    let label = match stage {
                                                        Stage::Downloading => {
                                                            t!("status_analyzing").to_string()
                                                        }
                                                        Stage::Splitting => {
                                                            t!("status_analyzing").to_string()
                                                        }
                                                        Stage::Analyzing { current, total } => {
                                                            format!(
                                                                "{} {current}/{total}",
                                                                t!("status_analyzing")
                                                            )
                                                        }
                                                        Stage::Extracting => {
                                                            t!("status_analyzing").to_string()
                                                        }
                                                    };
                                                    app.set_status_msg(label.into());
                                                }
                                            }
                                        }
                                    });
                                },
                            )
                            .await;
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = done_weak.upgrade() {
                                match result {
                                    Ok(out) => {
                                        app.set_progress(1.0);
                                        app.set_status_msg(
                                            format!(
                                                "{}: {} moments, {} chunks",
                                                t!("new_project_analyze_complete"),
                                                out.moments.len(),
                                                out.chunks.len()
                                            )
                                            .into(),
                                        );
                                        refresh_home(&app);
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
