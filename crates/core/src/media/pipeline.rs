// =================================================================================================
// media::pipeline — Stage state machine
// =================================================================================================

use tokio_util::sync::CancellationToken;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Stage {
    Downloading,
    Splitting,
    Analyzing { current: usize, total: usize },
    Extracting,
}

#[derive(Debug, Clone)]
pub struct PipelineCtx {
    pub url: String,
    pub video_id: String,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    StageChanged(Stage),
    Progress(f32, String),
    Log(String),
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub struct Pipeline;

impl Pipeline {
    pub fn new() -> Self {
        Self
    }

    pub async fn run<F>(&self, ctx: PipelineCtx, mut on_event: F) -> anyhow::Result<()>
    where
        F: FnMut(PipelineEvent) + Send + 'static,
    {
        if ctx.cancellation.is_cancelled() {
            anyhow::bail!("cancelled");
        }
        on_event(PipelineEvent::StageChanged(Stage::Downloading));
        on_event(PipelineEvent::Progress(0.1, "Downloading".into()));
        on_event(PipelineEvent::StageChanged(Stage::Splitting));
        on_event(PipelineEvent::Progress(0.4, "Splitting".into()));
        on_event(PipelineEvent::StageChanged(Stage::Analyzing { current: 0, total: 1 }));
        on_event(PipelineEvent::Progress(0.7, "Analyzing".into()));
        on_event(PipelineEvent::StageChanged(Stage::Extracting));
        on_event(PipelineEvent::Progress(1.0, "Done".into()));
        Ok(())
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
