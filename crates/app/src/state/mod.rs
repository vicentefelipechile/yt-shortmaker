// =================================================================================================
// app::state — Application state and navigation
// =================================================================================================

use yt_shortmaker_core::config::AppConfig;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    NewProject,
    ExportStandalone,
    Settings,
    Keys,
}

#[derive(Debug)]
pub struct AppState {
    pub route: Route,
    pub config: AppConfig,
    pub status_msg: Option<String>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            route: Route::Home,
            config,
            status_msg: None,
        }
    }

    pub fn navigate(&mut self, route: Route) {
        self.route = route;
    }
}
