// =================================================================================================
// app::main — Slint native entry point
// =================================================================================================

rust_i18n::i18n!("../../locales", fallback = "en");

use std::cell::RefCell;
use std::rc::Rc;

use rust_i18n::t;
use tracing_subscriber::EnvFilter;
use yt_shortmaker_core::config::store as config_store;
use yt_shortmaker_core::media::ytdlp;

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
    app.set_tr_url_label(t!("new_project_url_label").to_string().into());
    app.set_tr_fetch(t!("new_project_fetch").to_string().into());
    app.set_tr_export_title(t!("export_standalone").to_string().into());
    app.set_tr_export_desc(t!("export_standalone_desc").to_string().into());
    app.set_tr_keys_title(t!("nav_keys").to_string().into());
    app.set_tr_settings_title(t!("settings").to_string().into());
    app.set_tr_save(t!("save").to_string().into());
    app.set_tr_analyze(t!("new_project_start_analyze").to_string().into());
    app.set_tr_add_moment(t!("status_moment_added").to_string().into());
    app.set_tr_browse(t!("export_select_folders").to_string().into());
    app.set_tr_output_dir(t!("desc_output_dir").to_string().into());
    app.set_tr_language(t!("language").to_string().into());
    app.set_tr_key_name(t!("keys_add_title").to_string().into());
    app.set_tr_key_value(t!("keys_add_help").to_string().into());
    app.set_tr_add(t!("save").to_string().into());
    app.set_tr_delete(t!("cancel").to_string().into());
    app.set_tr_keys_hint(t!("keys_help").to_string().into());
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
                .map(|(i, k)| format!("{}: {} [{}]", i, k.name, if k.enabled { "on" } else { "off" }))
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        t!("keys_no_keys").to_string()
    };
    app.set_keys_list(text.into());
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = config_store::load().unwrap_or_default();
    rust_i18n::set_locale(&config.language);
    let config_rc = Rc::new(RefCell::new(config));

    let app = MainWindow::new()?;
    app.set_core_version(yt_shortmaker_core::version().into());
    app.set_output_dir(config_rc.borrow().output_dir.clone().unwrap_or_default().into());
    app.set_language(config_rc.borrow().language.clone().into());
    apply_translations(&app);
    refresh_keys_list(&app, &config_rc.borrow());

    let app_weak = app.as_weak();

    app.on_navigate({
        let app_weak = app_weak.clone();
        move |route| {
            if let Some(app) = app_weak.upgrade() {
                app.set_route(route);
            }
        }
    });

    app.on_fetch_info({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let url: String = app.get_url_input().into();
                match ytdlp::validate_media_url(&url) {
                    Ok(()) => {
                        app.set_url_error("".into());
                        app.set_video_title("Video title (mock)".into());
                        app.set_status_msg(t!("status_fetched", url = url).to_string().into());
                    }
                    Err(e) => app.set_url_error(e.to_string().into()),
                }
            }
        }
    });

    app.on_start_analyze({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_progress(0.35);
                app.set_status_msg(t!("status_analyzing").to_string().into());
                app.set_progress(1.0);
                app.set_status_msg(t!("new_project_analyze_complete").to_string().into());
            }
        }
    });

    app.on_add_moment({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_status_msg(t!("status_moment_added").to_string().into());
            }
        }
    });

    app.on_save_settings({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                cfg.output_dir = Some(app.get_output_dir().to_string());
                cfg.language = app.get_language().to_string();
                rust_i18n::set_locale(&cfg.language);
                match config_store::save(&cfg) {
                    Ok(()) => {
                        apply_translations(&app);
                        app.set_status_msg(t!("status_settings_saved").to_string().into());
                    }
                    Err(e) => app.set_status_msg(format!("{}: {e}", t!("error")).into()),
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

    app.on_add_key({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let name: String = app.get_new_key_name().to_string();
                let value: String = app.get_new_key_value().to_string();
                if name.is_empty() || value.is_empty() {
                    app.set_status_msg(t!("error").to_string().into());
                    return;
                }
                let mut cfg = config_rc.borrow_mut();
                let provider = cfg.ai.active_provider.clone();
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    p.keys.push(yt_shortmaker_core::config::ApiKeyEntry {
                        name: name.clone(),
                        value: value.clone(),
                        enabled: true,
                    });
                    let _ = yt_shortmaker_core::security::keyring::set_secret("yt-shortmaker", &name, &value);
                }
                let _ = config_store::save(&cfg);
                refresh_keys_list(&app, &cfg);
                app.set_new_key_name("".into());
                app.set_new_key_value("".into());
                app.set_status_msg(t!("msg_api_key_saved").to_string().into());
            }
        }
    });

    app.on_delete_key({
        let config_rc = config_rc.clone();
        let app_weak = app_weak.clone();
        move |idx| {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = config_rc.borrow_mut();
                let provider = cfg.ai.active_provider.clone();
                if let Some(p) = cfg.ai.providers.get_mut(&provider) {
                    let i = idx as usize;
                    if i < p.keys.len() {
                        let name = p.keys[i].name.clone();
                        let _ = yt_shortmaker_core::security::keyring::delete_secret("yt-shortmaker", &name);
                        p.keys.remove(i);
                    }
                }
                let _ = config_store::save(&cfg);
                refresh_keys_list(&app, &cfg);
            }
        }
    });

    app.run()?;
    Ok(())
}
