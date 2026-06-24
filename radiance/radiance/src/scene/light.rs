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

/// A single directional ("sun") light. Unlike [`SceneLight`], a directional
/// light has no position and no attenuation: every lit fragment receives the
/// same parallel light from `direction`. PAL5 maps ship exactly one such sun
/// per scene (`envinfo.env`), modeled here as a first-class light type rather
/// than a far-away point light.
#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    /// Unit direction **from the surface toward the light** (i.e. the
    /// direction the sun's rays come *from*), in world space.
    pub direction: Vec3,
    /// Linear RGB color / intensity of the sun.
    pub color: [f32; 3],
}

impl DirectionalLight {
    pub fn new(direction: Vec3, color: [f32; 3]) -> Self {
        Self { direction, color }
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
    /// Optional directional sun light. When present, lit shaders add a
    /// Lambert term for it with no attenuation, on top of ambient and the
    /// point lights.
    pub sun: Option<DirectionalLight>,
}

impl SceneLighting {
    pub fn new(ambient: [f32; 3], lights: Vec<SceneLight>) -> Self {
        Self {
            ambient,
            lights,
            sun: None,
        }
    }

    /// Build a lighting environment with a directional sun in addition to the
    /// ambient term and any point lights.
    pub fn with_sun(ambient: [f32; 3], lights: Vec<SceneLight>, sun: DirectionalLight) -> Self {
        Self {
            ambient,
            lights,
            sun: Some(sun),
        }
    }
}
