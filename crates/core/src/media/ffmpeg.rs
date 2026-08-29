// =================================================================================================
// media::ffmpeg — Duration, resolution, and filter compiler stubs
// =================================================================================================

use anyhow::Result;

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn get_duration(_path: &std::path::Path) -> Result<f64> {
    // TODO(M1): ffprobe wrapper
    anyhow::bail!("not implemented")
}

pub fn get_resolution(_path: &std::path::Path) -> Result<(u32, u32)> {
    anyhow::bail!("not implemented")
}
