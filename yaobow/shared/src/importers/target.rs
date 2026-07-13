//! Target-format selection and per-target conversion options.

/// The PAL3 game-asset format a normalized glTF scene is converted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetFormat {
    /// Animated role/prop model (`.mv3`): one model per mesh-bearing node,
    /// one `Mv3Mesh` per primitive, one vertex-frame snapshot per morph
    /// target (see [`crate::exporters::gltf::mv3`] for the inverse
    /// direction).
    Mv3,
    /// Static, possibly multi-material mesh (`.pol`).
    Pol,
    /// Composite scene model: a node hierarchy where each part may carry
    /// its own morph-target animation and TRS keyframes (`.cvd`).
    Cvd,
}

impl TargetFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            TargetFormat::Mv3 => "mv3",
            TargetFormat::Pol => "pol",
            TargetFormat::Cvd => "cvd",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TargetFormat::Mv3 => "mv3",
            TargetFormat::Pol => "pol",
            TargetFormat::Cvd => "cvd",
        }
    }
}

/// Mv3-specific conversion knobs.
#[derive(Debug, Clone)]
pub struct Mv3Options {
    /// World-unit-per-i16-step scale applied when quantizing vertex
    /// positions, matching the engine's decode scale (see
    /// `MV3_VERTEX_SCALE` in `exporters::gltf::mv3`). Positions are
    /// divided by this value and rounded to the nearest i16.
    pub vertex_scale: f32,
    /// Ticks per second used to convert animation keyframe times (in
    /// seconds) back to the engine's integer tick timestamps (matches
    /// `MV3_TICKS_PER_SECOND` in `exporters::gltf::mv3`).
    pub ticks_per_second: f32,
}

impl Default for Mv3Options {
    fn default() -> Self {
        Self {
            vertex_scale: 0.01562,
            ticks_per_second: 4580.0,
        }
    }
}

/// Pol-specific conversion knobs.
#[derive(Debug, Clone, Default)]
pub struct PolOptions {
    /// Force `use_alpha` on every material regardless of the source
    /// glTF material's `alphaMode`.
    pub force_use_alpha: Option<bool>,
}

/// Cvd-specific conversion knobs.
#[derive(Debug, Clone)]
pub struct CvdOptions {
    /// Emit the legacy `cvdf` magic (`unknown_float` semantics `0.4`,
    /// no optional per-material trailer) rather than `cvds` (`0.5`).
    /// Round-tripped `cvds` inputs still convert (the trailer's content
    /// isn't preserved by the loader this crate reads from, so it's
    /// re-emitted as an empty trailer either way) — this only controls
    /// which magic bytes are written.
    pub legacy_magic: bool,
}

impl Default for CvdOptions {
    fn default() -> Self {
        Self { legacy_magic: true }
    }
}

/// Top-level options for [`crate::importers::mv3::convert`],
/// [`crate::importers::pol::convert`], and
/// [`crate::importers::cvd::convert`].
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    pub mv3: Mv3Options,
    pub pol: PolOptions,
    pub cvd: CvdOptions,
}
