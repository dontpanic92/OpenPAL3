use crate::math::Vec3;

/// A single scene light. PAL3 scene lights (`.lgt`) are omni point lights with
/// a world-space position, a (possibly > 1.0) linear RGB color, and an
/// inner/outer attenuation range. Most shipped lights carry `FLT_MAX` ranges
/// (effectively un-attenuated), but some — e.g. interior candle "key" lights —
/// use a finite `(inner, outer)` falloff so they only light their immediate
/// surroundings.
#[derive(Debug, Clone, Copy)]
pub struct SceneLight {
    pub position: Vec3,
    pub color: [f32; 3],
    /// `[inner, outer]` attenuation radii. Within `inner` the light is at full
    /// intensity; it falls linearly to zero at `outer`. A non-finite or very
    /// large value (>= [`SceneLight::NO_ATTENUATION`]) means "no attenuation".
    pub range: [f32; 2],
}

impl SceneLight {
    /// Range values at or above this are treated as un-attenuated. PAL3 ships
    /// `FLT_MAX` (≈ 3.4e38) for omni lights; this threshold also catches the
    /// `f32::MAX` round-trip artifacts seen in the corpus (~1.8e19).
    pub const NO_ATTENUATION: f32 = 1.0e18;

    pub fn new(position: Vec3, color: [f32; 3]) -> Self {
        Self::with_range(position, color, [f32::MAX, f32::MAX])
    }

    pub fn with_range(position: Vec3, color: [f32; 3], range: [f32; 2]) -> Self {
        Self {
            position,
            color,
            range,
        }
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
