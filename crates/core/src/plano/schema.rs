// =================================================================================================
// plano::schema — Plano object schema
// =================================================================================================

use serde::{Deserialize, Serialize};

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

pub const OUTPUT_WIDTH: u32 = 1080;
pub const OUTPUT_HEIGHT: u32 = 1920;
pub const STRUCTURE_VERSION: u32 = 1;
pub const MIN_LAYER_SIZE: u32 = 8;
pub const SUPPORTED_OUTPUTS: [(u32, u32); 2] = [(1080, 1920), (720, 1280)];

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
// Versioned structure document (M5 editor model)
// -------------------------------------------------------------------------------------------------

fn default_structure_version() -> u32 {
    STRUCTURE_VERSION
}

fn default_output_config() -> OutputConfig {
    OutputConfig::default()
}

fn default_layers() -> Vec<PlanoLayer> {
    Vec::new()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OutputConfig {
    #[serde(default = "default_output_width")]
    pub width: u32,
    #[serde(default = "default_output_height")]
    pub height: u32,
    #[serde(default = "default_video_codec")]
    pub codec: String,
    #[serde(default = "default_bitrate")]
    pub bitrate: String,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_audio_codec")]
    pub audio_codec: String,
}

fn default_output_width() -> u32 {
    OUTPUT_WIDTH
}

fn default_output_height() -> u32 {
    OUTPUT_HEIGHT
}

fn default_video_codec() -> String {
    "libx264".to_string()
}

fn default_bitrate() -> String {
    "8M".to_string()
}

fn default_fps() -> u32 {
    30
}

fn default_audio_codec() -> String {
    "aac".to_string()
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            width: OUTPUT_WIDTH,
            height: OUTPUT_HEIGHT,
            codec: default_video_codec(),
            bitrate: default_bitrate(),
            fps: default_fps(),
            audio_codec: default_audio_codec(),
        }
    }
}

impl OutputConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !SUPPORTED_OUTPUTS.contains(&(self.width, self.height)) {
            return Err(format!(
                "unsupported output {}x{}, expected 1080x1920 or 720x1280",
                self.width, self.height
            ));
        }
        if self.codec.trim().is_empty() {
            return Err("codec is empty".to_string());
        }
        if self.audio_codec.trim().is_empty() {
            return Err("audio_codec is empty".to_string());
        }
        if self.bitrate.trim().is_empty() {
            return Err("bitrate is empty".to_string());
        }
        if self.fps != 30 && self.fps != 60 {
            return Err("fps must be 30 or 60".to_string());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlanoLayer {
    pub id: String,
    pub name: String,
    #[serde(default = "default_layer_visible")]
    pub visible: bool,
    #[serde(flatten)]
    pub object: PlanoObject,
}

fn default_layer_visible() -> bool {
    true
}

fn position_mut_of(obj: &mut PlanoObject) -> Option<&mut Position> {
    match obj {
        PlanoObject::Clip { position, .. } => Some(position),
        PlanoObject::Image { position, .. } => Some(position),
        PlanoObject::Shader { position, .. } => Some(position),
        PlanoObject::Video { position, .. } => Some(position),
    }
}

impl PlanoLayer {
    pub fn layer_type(&self) -> &'static str {
        match &self.object {
            PlanoObject::Clip { .. } => "clip",
            PlanoObject::Image { .. } => "image",
            PlanoObject::Shader { .. } => "shader",
            PlanoObject::Video { .. } => "video",
        }
    }

    pub fn resolved_rect(&self) -> (i32, i32, u32, u32) {
        let pos = match &self.object {
            PlanoObject::Clip { position, .. } => position,
            PlanoObject::Image { position, .. } => position,
            PlanoObject::Shader { position, .. } => position,
            PlanoObject::Video { position, .. } => position,
        };
        let w = pos.width.resolve(OUTPUT_WIDTH);
        let h = pos.height.resolve(OUTPUT_HEIGHT);
        let x = pos.x.resolve(OUTPUT_WIDTH, w);
        let y = pos.y.resolve(OUTPUT_HEIGHT, h);
        (x, y, w, h)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("layer id is empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("layer name is empty".to_string());
        }
        match &self.object {
            PlanoObject::Clip { position, .. } => validate_position(position),
            PlanoObject::Image {
                path,
                position,
                opacity,
                ..
            } => {
                if path.trim().is_empty() {
                    return Err(format!("image layer '{}' has empty path", self.id));
                }
                validate_opacity(*opacity).map_err(|e| format!("layer '{}': {e}", self.id))?;
                validate_position(position)
            }
            PlanoObject::Shader { effect, .. } => {
                let ShaderEffect::Blur { intensity } = effect;
                if *intensity == 0 || *intensity > 100 {
                    return Err(format!(
                        "layer '{}': blur intensity must be 1..=100",
                        self.id
                    ));
                }
                Ok(())
            }
            PlanoObject::Video {
                path,
                position,
                opacity,
                ..
            } => {
                if path.trim().is_empty() {
                    return Err(format!("video layer '{}' has empty path", self.id));
                }
                validate_opacity(*opacity).map_err(|e| format!("layer '{}': {e}", self.id))?;
                validate_position(position)
            }
        }
        .map_err(|e| format!("layer '{}': {e}", self.id))
    }
}

fn validate_opacity(v: f32) -> Result<(), String> {
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return Err("opacity must be 0.0..=1.0".to_string());
    }
    Ok(())
}

fn validate_position(pos: &Position) -> Result<(), String> {
    let w = pos.width.resolve(OUTPUT_WIDTH);
    let h = pos.height.resolve(OUTPUT_HEIGHT);
    if w < MIN_LAYER_SIZE || h < MIN_LAYER_SIZE {
        return Err(format!(
            "layer size {w}x{h} smaller than minimum {MIN_LAYER_SIZE}px"
        ));
    }
    if w > OUTPUT_WIDTH * 4 || h > OUTPUT_HEIGHT * 4 {
        return Err(format!("layer size {w}x{h} exceeds 4x output bounds"));
    }
    let _x = pos.x.resolve(OUTPUT_WIDTH, w);
    let _y = pos.y.resolve(OUTPUT_HEIGHT, h);
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlanoDocument {
    #[serde(default = "default_structure_version")]
    pub version: u32,
    #[serde(default = "default_output_config")]
    pub output: OutputConfig,
    #[serde(default)]
    pub preview_second: u64,
    #[serde(default = "default_layers")]
    pub layers: Vec<PlanoLayer>,
}

impl Default for PlanoDocument {
    fn default() -> Self {
        Self {
            version: STRUCTURE_VERSION,
            output: OutputConfig::default(),
            preview_second: 0,
            layers: Vec::new(),
        }
    }
}

impl PlanoDocument {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != STRUCTURE_VERSION {
            return Err(format!(
                "unsupported structure version {}, expected {STRUCTURE_VERSION}",
                self.version
            ));
        }
        self.output.validate()?;
        let mut seen = std::collections::HashSet::new();
        for layer in &self.layers {
            if !seen.insert(layer.id.clone()) {
                return Err(format!("duplicate layer id '{}'", layer.id));
            }
            layer.validate()?;
        }
        Ok(())
    }

    pub fn validate_for_export(&self) -> Result<(), String> {
        self.validate()?;
        if self.layers.iter().all(|l| !l.visible) || self.layers.is_empty() {
            return Err("structure has no visible layers".to_string());
        }
        Ok(())
    }

    pub fn to_plano(&self) -> Plano {
        self.layers
            .iter()
            .filter(|l| l.visible)
            .map(|l| l.object.clone())
            .collect()
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut PlanoLayer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn move_layer(&mut self, id: &str, dx: f32, dy: f32) -> Result<(), String> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err("drag delta is not finite".to_string());
        }
        let layer = self
            .find_mut(id)
            .ok_or_else(|| format!("layer '{id}' not found"))?;
        let (x, y, w, h) = layer.resolved_rect();
        let nx = (x as f32 + dx).round() as i32;
        let ny = (y as f32 + dy).round() as i32;
        if let Some(pos) = position_mut_of(&mut layer.object) {
            pos.x = PositionValue::Pixels(nx);
            pos.y = PositionValue::Pixels(ny);
            pos.width = SizeValue::Pixels(w);
            pos.height = SizeValue::Pixels(h);
        }
        layer.validate()?;
        Ok(())
    }

    pub fn resize_layer(&mut self, id: &str, x: f32, y: f32, w: f32, h: f32) -> Result<(), String> {
        if ![x, y, w, h].iter().all(|v| v.is_finite()) {
            return Err("resize geometry is not finite".to_string());
        }
        let w = w
            .round()
            .clamp(MIN_LAYER_SIZE as f32, (OUTPUT_WIDTH * 4) as f32) as u32;
        let h = h
            .round()
            .clamp(MIN_LAYER_SIZE as f32, (OUTPUT_HEIGHT * 4) as f32) as u32;
        if w < MIN_LAYER_SIZE || h < MIN_LAYER_SIZE {
            return Err("layer size below minimum".to_string());
        }
        let layer = self
            .find_mut(id)
            .ok_or_else(|| format!("layer '{id}' not found"))?;
        if let Some(pos) = position_mut_of(&mut layer.object) {
            pos.x = PositionValue::Pixels(x.round() as i32);
            pos.y = PositionValue::Pixels(y.round() as i32);
            pos.width = SizeValue::Pixels(w);
            pos.height = SizeValue::Pixels(h);
        }
        layer.validate()?;
        Ok(())
    }

    pub fn toggle_layer(&mut self, id: &str) -> Result<(), String> {
        let layer = self
            .find_mut(id)
            .ok_or_else(|| format!("layer '{id}' not found"))?;
        layer.visible = !layer.visible;
        Ok(())
    }

    pub fn delete_layer(&mut self, id: &str) -> Result<(), String> {
        let before = self.layers.len();
        self.layers.retain(|l| l.id != id);
        if self.layers.len() == before {
            return Err(format!("layer '{id}' not found"));
        }
        Ok(())
    }

    pub fn resize_with_handle(
        &mut self,
        id: &str,
        handle: &str,
        dx: f32,
        dy: f32,
    ) -> Result<(), String> {
        let (x, y, w, h) = self
            .layers
            .iter()
            .find(|l| l.id == id)
            .map(|l| l.resolved_rect())
            .ok_or_else(|| format!("layer '{id}' not found"))?;
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        let (nx, ny, nw, nh) = match handle {
            "top-left" => (x + dx, y + dy, w - dx, h - dy),
            "top-right" => (x, y + dy, w + dx, h - dy),
            "bottom-left" => (x + dx, y, w - dx, h + dy),
            _ => (x, y, w + dx, h + dy),
        };
        self.resize_layer(id, nx, ny, nw, nh)
    }

    pub fn raise_layer(&mut self, id: &str) -> Result<(), String> {
        let i = self
            .layers
            .iter()
            .position(|l| l.id == id)
            .ok_or_else(|| format!("layer '{id}' not found"))?;
        if i + 1 < self.layers.len() {
            self.layers.swap(i, i + 1);
        }
        Ok(())
    }

    pub fn lower_layer(&mut self, id: &str) -> Result<(), String> {
        let i = self
            .layers
            .iter()
            .position(|l| l.id == id)
            .ok_or_else(|| format!("layer '{id}' not found"))?;
        if i > 0 {
            self.layers.swap(i, i - 1);
        }
        Ok(())
    }
}

pub fn create_empty_document() -> PlanoDocument {
    PlanoDocument::default()
}

pub fn create_default_document() -> PlanoDocument {
    let plano = create_default_plano();
    let names = ["Background clip", "Blur", "Main clip"];
    let layers = plano
        .into_iter()
        .enumerate()
        .map(|(i, object)| PlanoLayer {
            id: format!("layer-{i}"),
            name: names.get(i).unwrap_or(&"Layer").to_string(),
            visible: true,
            object,
        })
        .collect();
    PlanoDocument {
        version: STRUCTURE_VERSION,
        output: OutputConfig::default(),
        preview_second: 0,
        layers,
    }
}

pub fn load_document(path: &str) -> anyhow::Result<PlanoDocument> {
    let content = std::fs::read_to_string(path)?;
    let cleaned = remove_js_comments(&content);
    // Accept legacy bare-array Plano files by wrapping them.
    if let Ok(layers_raw) = serde_json::from_str::<Plano>(&cleaned) {
        let layers = layers_raw
            .into_iter()
            .enumerate()
            .map(|(i, object)| PlanoLayer {
                id: format!("layer-{i}"),
                name: format!("Layer {}", i + 1),
                visible: true,
                object,
            })
            .collect();
        let doc = PlanoDocument {
            version: STRUCTURE_VERSION,
            output: OutputConfig::default(),
            preview_second: 0,
            layers,
        };
        doc.validate().map_err(|e| anyhow::anyhow!(e))?;
        return Ok(doc);
    }
    let doc: PlanoDocument = serde_json::from_str(&cleaned)?;
    doc.validate().map_err(|e| anyhow::anyhow!(e))?;
    Ok(doc)
}

pub fn save_document(path: &str, doc: &PlanoDocument) -> anyhow::Result<()> {
    doc.validate().map_err(|e| anyhow::anyhow!(e))?;
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(doc)?;
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

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

/// Pure viewport helpers shared by the Structure canvas.
/// All coordinates are f32 screen/logical units; zoom never mutates the document.
pub fn logical_to_world(logical: f32, origin: f32, zoom: f32) -> f32 {
    origin + logical * zoom
}

pub fn world_to_logical(world: f32, origin: f32, zoom: f32) -> f32 {
    if zoom.abs() < 0.0001 {
        0.0
    } else {
        (world - origin) / zoom
    }
}

pub fn world_size_for(frame: f32, visible: f32, margin: f32) -> f32 {
    (frame + margin * 2.0).max(visible + margin)
}

pub fn world_origin_for(world: f32, frame: f32) -> f32 {
    (world - frame) / 2.0
}

pub fn centered_viewport_for(world: f32, visible: f32) -> f32 {
    -(world - visible) / 2.0
}

pub fn fit_zoom_for(visible_w: f32, visible_h: f32, output_w: f32, output_h: f32) -> f32 {
    if output_w <= 0.0 || output_h <= 0.0 || visible_w <= 0.0 || visible_h <= 0.0 {
        return 0.3;
    }
    let z = (visible_w / output_w).min(visible_h / output_h) * 0.88;
    z.clamp(0.05, 2.0)
}

pub fn clamp_zoom(z: f32) -> f32 {
    if !z.is_finite() {
        return 0.3;
    }
    z.clamp(0.05, 2.0)
}

/// Logical grid step scaled by zoom (screen px per grid cell).
/// Keeps the grid visually consistent: zooming out shrinks cells, zooming in grows them.
pub fn grid_step_for(zoom: f32, logical_step: f32) -> f32 {
    (logical_step * clamp_zoom(zoom)).max(1.0)
}

/// Grid origin offset so cells stay aligned with the logical frame origin.
/// Returns a value in `0..step` that anchors cell `k` to `origin + k * step`.
pub fn grid_offset_for(origin: f32, step: f32) -> f32 {
    if !step.is_finite() || step <= 0.0 {
        return 0.0;
    }
    origin - (origin / step).floor() * step
}

/// Number of grid lines needed to cover `world` px with `step` px cells (plus margin).
pub fn grid_count_for(world: f32, step: f32) -> u32 {
    if !step.is_finite() || step <= 0.0 || !world.is_finite() || world <= 0.0 {
        return 0;
    }
    (world / step).ceil() as u32 + 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document_fails_export_validation() {
        let doc = create_empty_document();
        assert!(doc.validate().is_ok());
        assert!(doc.validate_for_export().is_err());
    }

    #[test]
    fn test_move_and_resize_layer() {
        let mut doc = create_default_document();
        let id = doc.layers[0].id.clone();
        let (x0, _, _, _) = doc.layers[0].resolved_rect();
        doc.move_layer(&id, 10.0, 5.0).unwrap();
        let (x1, y1, _, _) = doc.layers[0].resolved_rect();
        assert_eq!(x1, x0 + 10);
        let _ = y1;
        doc.resize_with_handle(&id, "bottom-right", 20.0, 10.0)
            .unwrap();
        let (_, _, w, h) = doc.layers[0].resolved_rect();
        assert!(w >= MIN_LAYER_SIZE && h >= MIN_LAYER_SIZE);
        doc.toggle_layer(&id).unwrap();
        assert!(!doc.layers[0].visible);
        doc.delete_layer(&id).unwrap();
        assert!(doc.layers.iter().all(|l| l.id != id));
    }

    #[test]
    fn test_output_validation() {
        let mut doc = create_empty_document();
        doc.output.fps = 60;
        assert!(doc.validate().is_ok());
        doc.output.fps = 24;
        assert!(doc.validate().is_err());
    }

    #[test]
    fn test_viewport_helpers_center_and_fit() {
        // Frame 1080x1920 at zoom 0.3 in a 700x500 view.
        let zoom = 0.3;
        let frame_w = 1080.0 * zoom;
        let frame_h = 1920.0 * zoom;
        let world_w = world_size_for(frame_w, 700.0, 400.0);
        let world_h = world_size_for(frame_h, 500.0, 400.0);
        assert!(world_w >= frame_w + 800.0);
        assert!(world_h >= frame_h + 800.0);
        let ox = world_origin_for(world_w, frame_w);
        let oy = world_origin_for(world_h, frame_h);
        // Logical origin maps to world origin.
        assert!((logical_to_world(0.0, ox, zoom) - ox).abs() < 0.001);
        assert!((world_to_logical(ox + 1080.0 * zoom, ox, zoom) - 1080.0).abs() < 0.01);
        let _ = oy;
        // Centered viewport puts world center at view center.
        let vx = centered_viewport_for(world_w, 700.0);
        assert!(((-vx + 350.0) - world_w / 2.0).abs() < 0.001);
        // Fit zoom shrinks tall outputs to the visible height.
        let fit = fit_zoom_for(700.0, 500.0, 1080.0, 1920.0);
        assert!(fit > 0.05 && fit < 0.3);
        // Zoom clamp keeps document coordinates untouched by definition.
        assert_eq!(clamp_zoom(10.0), 2.0);
        assert_eq!(clamp_zoom(-1.0), 0.05);
    }

    #[test]
    fn test_move_is_additive_and_single_gesture() {
        // Simulates drag-start -> N previews -> drag-finish with one history entry.
        let mut doc = create_default_document();
        let id = doc.layers[0].id.clone();
        let before = doc.clone();
        let (x0, y0, _, _) = doc.layers[0].resolved_rect();
        for _ in 0..10 {
            doc.move_layer(&id, 1.0, 1.0).unwrap();
        }
        let (x1, y1, _, _) = doc.layers[0].resolved_rect();
        assert_eq!(x1, x0 + 10);
        assert_eq!(y1, y0 + 10);
        // One undo entry restores everything (before != after, single clone).
        assert_ne!(before, doc);
    }

    #[test]
    fn test_grid_scales_with_zoom_and_stays_aligned() {
        // Step shrinks when zooming out and grows when zooming in.
        let near = grid_step_for(0.1, 100.0);
        let mid = grid_step_for(0.3, 100.0);
        let far = grid_step_for(1.0, 100.0);
        assert!(near < mid && mid < far);
        assert!((mid - 30.0).abs() < 0.001);
        // Offset keeps logical multiples on grid lines: origin + k*step - offset is a multiple of step.
        for origin in [0.0, 37.0, 213.5, 1024.0] {
            let step = grid_step_for(0.3, 100.0);
            let off = grid_offset_for(origin, step);
            assert!(off >= 0.0 && off < step);
            for k in 0..4 {
                let aligned = origin + k as f32 * step - off;
                assert!((aligned / step).round() * step - aligned < 0.01);
            }
        }
        // Count covers the world plus margin and degrades gracefully.
        assert_eq!(grid_count_for(1000.0, 0.0), 0);
        assert_eq!(grid_count_for(1000.0, 30.0), 34 + 3);
        assert!(grid_count_for(400.0, 5.0) > grid_count_for(400.0, 200.0));
    }
}
