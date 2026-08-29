// =================================================================================================
// app::main — Slint native entry point
// =================================================================================================

rust_i18n::i18n!("../../locales", fallback = "en");

use rust_i18n::t;
use tracing_subscriber::EnvFilter;
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
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = yt_shortmaker_core::config::store::load().unwrap_or_default();
    rust_i18n::set_locale(&config.language);

    let app = MainWindow::new()?;
    app.set_core_version(yt_shortmaker_core::version().into());
    apply_translations(&app);

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
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_status_msg(t!("status_settings_saved").to_string().into());
            }
        }
    });

    app.run()?;
    Ok(())
}
