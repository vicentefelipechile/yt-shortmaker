// =================================================================================================
// plano::schema — Plano object schema stub
// =================================================================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plano {
    pub objects: Vec<PlanoObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlanoObject {
    Clip { path: String },
    Image { path: String },
    Shader { kind: String },
}
