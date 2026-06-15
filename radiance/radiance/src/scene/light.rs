use crate::math::Vec3;

/// A single scene light. PAL3 scene lights (`.lgt`) are un-attenuated omni
/// point lights, which is all this struct currently models: a world-space
/// position and a (possibly > 1.0) linear RGB color.
#[derive(Debug, Clone, Copy)]
pub struct SceneLight {
    pub position: Vec3,
    pub color: [f32; 3],
}

impl SceneLight {
    pub fn new(position: Vec3, color: [f32; 3]) -> Self {
        Self { position, color }
    }
}

/// Per-scene lighting environment consumed by the renderer when shading
/// dynamically-lit objects (e.g. PAL3 actors). Static/baked geometry ignores
/// this and keeps its own (lightmap / vertex-color) path.
#[derive(Debug, Clone, Default)]
pub struct SceneLighting {
    /// Flat ambient term added to every lit fragment.
    pub ambient: [f32; 3],
    /// Active point lights.
    pub lights: Vec<SceneLight>,
}

impl SceneLighting {
    pub fn new(ambient: [f32; 3], lights: Vec<SceneLight>) -> Self {
        Self { ambient, lights }
    }
}
