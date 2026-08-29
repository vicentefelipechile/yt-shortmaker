// =================================================================================================
// app::state — Application state and navigation
// =================================================================================================

use yt_shortmaker_core::config::AppConfig;
use yt_shortmaker_core::types::VideoMoment;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewProjectStep {
    Ingest,
    Analyze,
    Review,
}

#[derive(Debug)]
pub struct AppState {
    pub route: Route,
    pub new_project_step: NewProjectStep,
    pub config: AppConfig,
    pub url_input: String,
    pub url_error: Option<String>,
    pub video_title: Option<String>,
    pub analyzing: bool,
    pub progress: f32,
    pub log: Vec<String>,
    pub moments: Vec<VideoMoment>,
    pub status_msg: Option<String>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            route: Route::Home,
            new_project_step: NewProjectStep::Ingest,
            config,
            url_input: String::new(),
            url_error: None,
            video_title: None,
            analyzing: false,
            progress: 0.0,
            log: Vec::new(),
            moments: Vec::new(),
            status_msg: None,
        }
    }

    pub fn navigate(&mut self, route: Route) {
        self.route = route;
        if route == Route::NewProject {
            self.new_project_step = NewProjectStep::Ingest;
        }
    }
}
