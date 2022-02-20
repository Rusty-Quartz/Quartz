use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use super::noise_settings::BlockState;

#[derive(Serialize, Deserialize)]
pub struct SurfaceBuilder {
    pub r#type: UnlocalizedName,
    pub config: SurfaceBuilderConfig,
}

#[derive(Serialize, Deserialize)]
pub struct SurfaceBuilderConfig {
    pub top_material: BlockState,
    pub under_material: BlockState,
    pub underwater_material: BlockState,
}
