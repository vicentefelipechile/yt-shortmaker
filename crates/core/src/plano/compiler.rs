// =================================================================================================
// plano::compiler — FFmpeg filter compiler
// =================================================================================================

use std::collections::HashMap;
use std::path::Path;

use super::schema::{Fit, Plano, PlanoObject, ShaderEffect, OUTPUT_HEIGHT, OUTPUT_WIDTH};

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

struct FilterContext {
    filters: Vec<String>,
    label_counter: usize,
}

impl FilterContext {
    fn new() -> Self {
        Self {
            filters: Vec::new(),
            label_counter: 0,
        }
    }

    fn next_label(&mut self) -> String {
        let label = format!("layer{}", self.label_counter);
        self.label_counter += 1;
        label
    }
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn build_ffmpeg_filter(plano: &Plano, clip_path: &str) -> (String, Vec<String>) {
    let mut ctx = FilterContext::new();
    let mut inputs: Vec<String> = vec![clip_path.to_string()];
    const CLIP_INPUT: usize = 0;

    let mut additional_inputs: Vec<(usize, String)> = Vec::new();
    for (idx, obj) in plano.iter().enumerate() {
        match obj {
            PlanoObject::Image { path, .. } if Path::new(path).exists() => {
                additional_inputs.push((idx, path.clone()));
            }
            PlanoObject::Video { path, .. } if Path::new(path).exists() => {
                additional_inputs.push((idx, path.clone()));
            }
            _ => {}
        }
    }

    for (_, path) in &additional_inputs {
        inputs.push(path.clone());
    }

    let mut input_mapping: HashMap<usize, usize> = HashMap::new();
    for (i, (plano_idx, _)) in additional_inputs.iter().enumerate() {
        input_mapping.insert(*plano_idx, i + 1);
    }

    let mut current_label = "base".to_string();
    ctx.filters.push(format!(
        "color=c=black:s={}x{}:r=60:d=36000[base]",
        OUTPUT_WIDTH, OUTPUT_HEIGHT
    ));

    for (idx, obj) in plano.iter().enumerate() {
        let next_label = if idx == plano.len() - 1 {
            "out".to_string()
        } else {
            ctx.next_label()
        };

        match obj {
            PlanoObject::Clip {
                position, crop, fit, ..
            } => {
                let w = position.width.resolve(OUTPUT_WIDTH);
                let h = position.height.resolve(OUTPUT_HEIGHT);
                let mut base_filter = format!("[{}:v]setpts=PTS-STARTPTS,", CLIP_INPUT);

                if let Some(c) = crop {
                    if c.is_specified() {
                        let x_from = c.x_from.unwrap_or(0);
                        let x_to = c.x_to.unwrap_or(0);
                        let y_from = c.y_from.unwrap_or(0);
                        let y_to = c.y_to.unwrap_or(0);
                        if x_to > x_from {
                            let crop_w = x_to - x_from;
                            base_filter = format!("{}crop={}:ih:{}:0,", base_filter, crop_w, x_from);
                        }
                        if y_to > y_from {
                            let crop_h = y_to - y_from;
                            base_filter =
                                format!("{}crop=iw:{}:{}:{},", base_filter, crop_h, 0, y_from);
                        }
                    }
                }

                let scale_filter = match fit {
                    Fit::Cover => format!(
                        "{}scale={}:{}:force_original_aspect_ratio=increase,crop={}:{}",
                        base_filter, w, h, w, h
                    ),
                    Fit::Contain => format!(
                        "{}scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1",
                        base_filter, w, h, w, h
                    ),
                    Fit::Stretch => format!("{}scale={}:{},setsar=1", base_filter, w, h),
                };

                let x = position.x.resolve(OUTPUT_WIDTH, w);
                let y = position.y.resolve(OUTPUT_HEIGHT, h);
                ctx.filters.push(format!("{}[tmp{}]", scale_filter, idx));
                ctx.filters.push(format!(
                    "[{}][tmp{}]overlay={}:{}[{}]",
                    current_label, idx, x, y, next_label
                ));
                current_label = next_label;
            }
            PlanoObject::Shader { effect, .. } => match effect {
                ShaderEffect::Blur { intensity } => {
                    ctx.filters.push(format!(
                        "[{}]boxblur={}:{}[{}]",
                        current_label, intensity, intensity, next_label
                    ));
                    current_label = next_label;
                }
            },
            PlanoObject::Image {
                position, opacity, ..
            } => {
                if let Some(&input_idx) = input_mapping.get(&idx) {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    let x = position.x.resolve(OUTPUT_WIDTH, w);
                    let y = position.y.resolve(OUTPUT_HEIGHT, h);
                    let mut img_filter = format!("[{}:v]scale={}:{}", input_idx, w, h);
                    if *opacity < 1.0 {
                        img_filter = format!("{},format=rgba,colorchannelmixer=aa={}", img_filter, opacity);
                    }
                    ctx.filters.push(format!("{}[img{}]", img_filter, idx));
                    ctx.filters.push(format!(
                        "[{}][img{}]overlay={}:{}[{}]",
                        current_label, idx, x, y, next_label
                    ));
                    current_label = next_label;
                }
            }
            PlanoObject::Video {
                position,
                loop_video,
                keep_last_frame,
                opacity,
                fit,
                ..
            } => {
                if let Some(&input_idx) = input_mapping.get(&idx) {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    let x = position.x.resolve(OUTPUT_WIDTH, w);
                    let y = position.y.resolve(OUTPUT_HEIGHT, h);
                    let mut base_filter = format!("[{}:v]setpts=PTS-STARTPTS,", input_idx);
                    if *loop_video {
                        base_filter = format!("{}loop=loop=-1:size=32767:start=0,", base_filter);
                    } else if *keep_last_frame {
                        base_filter =
                            format!("{}tpad=stop_mode=clone:stop_duration=99999,", base_filter);
                    }
                    let scale_filter = match fit {
                        Fit::Cover => format!(
                            "{}scale={}:{}:force_original_aspect_ratio=increase,crop={}:{}",
                            base_filter, w, h, w, h
                        ),
                        Fit::Contain => format!(
                            "{}scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:-1:-1:color=0x00000000",
                            base_filter, w, h, w, h
                        ),
                        Fit::Stretch => format!("{}scale={}:{}", base_filter, w, h),
                    };
                    let mut vid_filter = format!("{},setsar=1", scale_filter);
                    if *opacity < 1.0 {
                        vid_filter = format!("{},format=rgba,colorchannelmixer=aa={}", vid_filter, opacity);
                    }
                    ctx.filters.push(format!("{}[vid{}]", vid_filter, idx));
                    ctx.filters.push(format!(
                        "[{}][vid{}]overlay={}:{}[{}]",
                        current_label, idx, x, y, next_label
                    ));
                    current_label = next_label;
                }
            }
        }
    }

    if plano.is_empty() {
        ctx.filters.push("[base]null[out]".to_string());
    }

    (ctx.filters.join(";"), inputs)
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plano::schema::{create_default_plano, Position, PositionValue, SizeValue};

    #[test]
    fn test_build_default() {
        let plano = create_default_plano();
        let (filter, inputs) = build_ffmpeg_filter(&plano, "test.mp4");
        assert!(filter.contains("[out]"));
        assert_eq!(inputs.len(), 1);
        assert!(filter.contains("boxblur"));
    }

    #[test]
    fn test_build_empty() {
        let empty: Plano = vec![];
        let (filter, inputs) = build_ffmpeg_filter(&empty, "clip.mp4");
        assert!(filter.contains("[base]null[out]"));
        assert_eq!(inputs, vec!["clip.mp4"]);
    }

    #[test]
    fn test_build_cover_vs_contain() {
        let plano_cover = vec![PlanoObject::Clip {
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Pixels(0),
                width: SizeValue::Pixels(1080),
                height: SizeValue::Pixels(1920),
            },
            crop: None,
            fit: Fit::Cover,
            comment: None,
        }];
        let (f, _) = build_ffmpeg_filter(&plano_cover, "c.mp4");
        assert!(f.contains("force_original_aspect_ratio=increase"));

        let plano_contain = vec![PlanoObject::Clip {
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Pixels(0),
                width: SizeValue::Pixels(1080),
                height: SizeValue::Pixels(1920),
            },
            crop: None,
            fit: Fit::Contain,
            comment: None,
        }];
        let (f2, _) = build_ffmpeg_filter(&plano_contain, "c.mp4");
        assert!(f2.contains("force_original_aspect_ratio=decrease"));
    }
}
