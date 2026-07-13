//! Shared error and diagnostic types for glTF import and target-format
//! conversion.
//!
//! [`ImportError`] covers hard failures: malformed/unsupported glTF input,
//! and violations of a target format's topology/attribute/index/
//! quantization/animation-representation constraints. Soft issues (a
//! fallback or default value was substituted, precision was lost, etc.)
//! are collected into [`Diagnostics`] instead of failing the conversion.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("failed to read glTF file `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse glTF document: {0}")]
    Gltf(#[from] gltf::Error),

    #[error(
        "unsupported buffer/image source `{0}`: only relative file paths (and the embedded GLB BIN chunk) are supported"
    )]
    UnsupportedSource(String),

    #[error("the GLB file has no embedded BIN chunk, but buffer #{0} requires it")]
    MissingGlbBlob(usize),

    #[error(
        "unsafe glTF buffer/image URI `{uri}`: {reason} (only relative paths contained beneath the glTF file's directory are supported)"
    )]
    UnsafeExternalUri { uri: String, reason: &'static str },

    #[error(
        "buffer #{index} `{uri}` is {actual} bytes, but the glTF buffer declares byteLength {expected}"
    )]
    BufferLengthMismatch {
        index: usize,
        uri: String,
        expected: usize,
        actual: usize,
    },

    #[error(
        "primitive #{primitive} of mesh `{mesh}` uses topology {mode:?}; only indexed TRIANGLES primitives are supported"
    )]
    UnsupportedTopology {
        mesh: String,
        primitive: usize,
        mode: gltf::mesh::Mode,
    },

    #[error(
        "primitive #{primitive} of mesh `{mesh}` has no index buffer; only indexed primitives are supported"
    )]
    MissingIndices { mesh: String, primitive: usize },

    #[error("primitive #{primitive} of mesh `{mesh}` is missing required attribute `{attribute}`")]
    MissingAttribute {
        mesh: String,
        primitive: usize,
        attribute: &'static str,
    },

    #[error(
        "primitive #{primitive} of mesh `{mesh}` references vertex index {index}, which is out of bounds for {vertex_count} loaded position(s) (malformed glTF data)"
    )]
    PrimitiveIndexOutOfBounds {
        mesh: String,
        primitive: usize,
        index: u32,
        vertex_count: usize,
    },

    #[error(
        "mesh `{mesh}` primitive #{primitive} has {count} vertices, exceeding the u16 index limit of 65536"
    )]
    TooManyVertices {
        mesh: String,
        primitive: usize,
        count: usize,
    },

    #[error(
        "mesh `{mesh}` primitive #{primitive} has index {index} which doesn't fit in u16 (vertex count {vertex_count})"
    )]
    IndexOutOfRange {
        mesh: String,
        primitive: usize,
        index: u32,
        vertex_count: usize,
    },

    #[error(
        "mesh `{mesh}` primitive #{primitive} index remap overflowed (raw index {index} + base offset {base_index} exceeds u32::MAX); malformed or excessively large glTF data"
    )]
    IndexRemapOverflow {
        mesh: String,
        primitive: usize,
        index: u32,
        base_index: u32,
    },

    #[error(
        "mesh `{mesh}`: {component} component {value} (after scale {scale}) is {quantized}, which overflows the i16 quantization range [-32768, 32767]"
    )]
    QuantizationOverflow {
        mesh: String,
        component: &'static str,
        value: f32,
        scale: f32,
        quantized: f64,
    },

    #[error(
        "animation `{animation}` channel targeting node `{node}` uses interpolation {interpolation:?}, which {target} cannot represent; only LINEAR and STEP are supported"
    )]
    UnsupportedInterpolation {
        animation: String,
        node: String,
        interpolation: gltf::animation::Interpolation,
        target: &'static str,
    },

    #[error(
        "animation `{animation}` targets node `{node}` with property {property:?}, which {target} cannot represent"
    )]
    UnsupportedAnimationTarget {
        animation: String,
        node: String,
        property: gltf::animation::Property,
        target: &'static str,
    },

    #[error(
        "node `{node}` has a static transform (translation/rotation/non-uniform scale) that {target} cannot represent statically, and is not fully covered by an animation channel"
    )]
    UnsupportedStaticTransform { node: String, target: &'static str },

    #[error(
        "mesh-bearing node `{node}` is nested under another node, but {target} has no node hierarchy; only scene-root nodes may carry geometry"
    )]
    NestedMeshNode { node: String, target: &'static str },

    #[error(
        "node `{node}` mesh has {targets} morph target(s) but its weights animation has {expected} frame(s) of timing data (expected {targets_plus_one})"
    )]
    MorphTargetTimingMismatch {
        node: String,
        targets: usize,
        expected: usize,
        targets_plus_one: usize,
    },

    #[error(
        "node `{node}` mesh primitives disagree on morph target count ({a} vs {b}); every primitive of one mesh must share one frame timeline"
    )]
    MorphTargetCountMismatch { node: String, a: usize, b: usize },

    #[error("node `{node}` has {0} morph target frame(s) but no weights animation to time them", .frames)]
    MissingWeightsAnimation { node: String, frames: usize },

    #[error(
        "node `{node}` scale is non-uniform ({scale:?}); {target} only supports a single uniform scale factor"
    )]
    NonUniformScale {
        node: String,
        scale: [f32; 3],
        target: &'static str,
    },

    #[error(
        "mesh `{mesh}` texture name `{name}` is {actual} bytes when GBK-encoded, exceeding the {limit} byte capacity"
    )]
    TextureNameTooLong {
        mesh: String,
        name: String,
        actual: usize,
        limit: usize,
    },

    #[error(
        "mesh `{mesh}` action/animation name `{name}` is {actual} bytes when GBK-encoded, exceeding the {limit} byte capacity"
    )]
    NameTooLong {
        mesh: String,
        name: String,
        actual: usize,
        limit: usize,
    },

    #[error("failed to GBK-encode string `{0}`")]
    StringEncoding(String),

    #[error("no exportable geometry found for target format {0}")]
    NoGeometry(&'static str),

    #[error("failed to write output file: {0}")]
    Write(#[from] std::io::Error),

    #[error("failed to serialize mv3 file: {0}")]
    Mv3Write(#[from] fileformats::mv3::Mv3WriteError),

    #[error("failed to serialize pol file: {0}")]
    PolWrite(#[from] fileformats::pol::PolWriteError),

    #[error("failed to serialize cvd file: {0}")]
    CvdWrite(#[from] fileformats::pal3::cvd::CvdError),

    #[error("failed to parse replacement template: {0}")]
    TemplateRead(String),

    #[error("{0}")]
    Other(String),
}

/// A soft issue surfaced during conversion: a default/fallback value was
/// substituted, or some source data was intentionally dropped. Doesn't
/// stop the conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics(pub Vec<Diagnostic>);

impl Diagnostics {
    pub fn push(&mut self, message: impl Into<String>) {
        self.0.push(Diagnostic(message.into()));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn messages(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|d| d.0.as_str())
    }
}
