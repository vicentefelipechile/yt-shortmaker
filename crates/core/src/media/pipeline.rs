// =================================================================================================
// media::pipeline — Stage state machine stub
// =================================================================================================

#[derive(Debug, Clone)]
pub enum Stage {
    Downloading,
    Splitting,
    Analyzing { current: usize, total: usize },
    Extracting,
}
