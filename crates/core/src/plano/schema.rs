// =================================================================================================
// plano::schema — Plano object schema
// =================================================================================================

use serde::{Deserialize, Serialize};

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

pub const OUTPUT_WIDTH: u32 = 1080;
pub const OUTPUT_HEIGHT: u32 = 1920;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum PositionValue {
    Pixels(i32),
    Keyword(String),
}

impl PositionValue {
    pub fn resolve(&self, container_size: u32, element_size: u32) -> i32 {
        match self {
            PositionValue::Pixels(px) => *px,
            PositionValue::Keyword(kw) => {
                let kw_lower = kw.to_lowercase();
                if kw_lower == "center" {
                    ((container_size - element_size) / 2) as i32
                } else if kw_lower.ends_with('%') {
                    if let Ok(pct) = kw_lower.trim_end_matches('%').parse::<f32>() {
                        ((container_size as f32) * (pct / 100.0)) as i32
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
        }
    }
}

impl Default for PositionValue {
    fn default() -> Self {
        Self::Pixels(0)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum SizeValue {
    Pixels(u32),
    Keyword(String),
}

impl SizeValue {
    pub fn resolve(&self, reference_size: u32) -> u32 {
        match self {
            SizeValue::Pixels(px) => *px,
            SizeValue::Keyword(kw) => {
                let kw_lower = kw.to_lowercase();
                if kw_lower == "full" {
                    reference_size
                } else if kw_lower.ends_with('%') {
                    if let Ok(pct) = kw_lower.trim_end_matches('%').parse::<f32>() {
                        ((reference_size as f32) * (pct / 100.0)) as u32
                    } else {
                        reference_size
                    }
                } else {
                    reference_size
                }
            }
        }
    }
}

impl Default for SizeValue {
    fn default() -> Self {
        Self::Keyword("full".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Position {
    #[serde(default)]
    pub x: PositionValue,
    #[serde(default)]
    pub y: PositionValue,
    #[serde(default)]
    pub width: SizeValue,
    #[serde(default)]
    pub height: SizeValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Crop {
    #[serde(default)]
    pub x_from: Option<i32>,
    #[serde(default)]
    pub x_to: Option<i32>,
    #[serde(default)]
    pub y_from: Option<i32>,
    #[serde(default)]
    pub y_to: Option<i32>,
}

impl Crop {
    pub fn is_specified(&self) -> bool {
        self.x_from.is_some() || self.x_to.is_some() || self.y_from.is_some() || self.y_to.is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ShaderEffect {
    Blur {
        #[serde(default = "default_blur_intensity")]
        intensity: u32,
    },
}

fn default_blur_intensity() -> u32 {
    20
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    Cover,
    Contain,
    Stretch,
}

fn default_fit() -> Fit {
    Fit::Stretch
}

fn default_opacity() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PlanoObject {
    Clip {
        position: Position,
        #[serde(default)]
        crop: Option<Crop>,
        #[serde(default = "default_fit")]
        fit: Fit,
        #[serde(default)]
        comment: Option<String>,
    },
    Image {
        path: String,
        position: Position,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default)]
        comment: Option<String>,
    },
    Shader {
        effect: ShaderEffect,
        position: Position,
        #[serde(default)]
        comment: Option<String>,
    },
    Video {
        path: String,
        position: Position,
        #[serde(default = "default_true")]
        loop_video: bool,
        #[serde(default)]
        keep_last_frame: bool,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default = "default_fit")]
        fit: Fit,
        #[serde(default)]
        comment: Option<String>,
    },
}

pub type Plano = Vec<PlanoObject>;

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

pub fn create_default_plano() -> Plano {
    vec![
        PlanoObject::Clip {
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Pixels(0),
                width: SizeValue::Keyword("full".to_string()),
                height: SizeValue::Keyword("full".to_string()),
            },
            crop: None,
            fit: Fit::Cover,
            comment: Some("Fondo desenfocado del clip original".to_string()),
        },
        PlanoObject::Shader {
            effect: ShaderEffect::Blur { intensity: 20 },
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Pixels(0),
                width: SizeValue::Keyword("full".to_string()),
                height: SizeValue::Keyword("full".to_string()),
            },
            comment: Some("Shader de blur sobre el fondo".to_string()),
        },
        PlanoObject::Clip {
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Keyword("center".to_string()),
                width: SizeValue::Keyword("full".to_string()),
                height: SizeValue::Pixels(1200),
            },
            crop: None,
            fit: Fit::Cover,
            comment: Some("Video principal del clip".to_string()),
        },
    ]
}

pub fn remove_js_comments(content: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' && !in_string {
            in_string = true;
            result.push(c);
        } else if c == '"' && in_string {
            let prev_backslashes = result.chars().rev().take_while(|&x| x == '\\').count();
            if prev_backslashes % 2 == 0 {
                in_string = false;
            }
            result.push(c);
        } else if !in_string && c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            while let Some(&next) = chars.peek() {
                if next == '\n' {
                    break;
                }
                chars.next();
            }
        } else {
            result.push(c);
        }
    }
    result
}

pub fn load_plano(path: &str) -> anyhow::Result<Plano> {
    let content = std::fs::read_to_string(path)?;
    let cleaned = remove_js_comments(&content);
    let plano: Plano = serde_json::from_str(&cleaned)?;
    Ok(plano)
}

pub fn save_plano(path: &str, plano: &Plano) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(plano)?;
    std::fs::write(path, json)?;
    Ok(())
}
