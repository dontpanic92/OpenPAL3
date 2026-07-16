//! Normalized, format-agnostic in-memory representation of an imported
//! glTF asset.
//!
//! [`crate::importers::loader::load_gltf_scene`] parses `.glb`/`.gltf`
//! (via the `gltf` crate) into an [`ImportedScene`]; the target-format
//! converters in [`crate::importers::mv3`], [`crate::importers::pol`], and
//! [`crate::importers::cvd`] consume this tree without touching the `gltf`
//! crate types directly, so all glTF-specific parsing concerns are
//! isolated to the loader.

use serde_json::Value;

/// A parsed glTF scene: a flattened node table (indices are stable and
/// referenced by [`ImportedNode::children`] and animation channels) plus
/// the animations and round-trip metadata that apply to it.
#[derive(Debug, Clone, Default)]
pub struct ImportedScene {
    /// All nodes in the document, in glTF node-index order.
    pub nodes: Vec<ImportedNode>,
    /// Indices into [`Self::nodes`] for the roots of the default scene (or
    /// of every scene, if the document doesn't set a default one).
    pub roots: Vec<usize>,
    pub animations: Vec<ImportedAnimation>,
    /// Skins referenced by [`ImportedNode::skin`], in glTF skin-index order.
    pub skins: Vec<ImportedSkin>,
    /// Parsed `asset.extras.yaobow` payload, if present.
    pub extras: Option<super::extras::YaobowExtras>,
    /// Referenced glTF images, converted to TGA and assigned deterministic
    /// model-relative paths under `_yaobow_import/`.
    pub textures: Vec<ImportedTexture>,
}

#[derive(Debug, Clone)]
pub struct ImportedTexture {
    pub image_index: usize,
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

/// One glTF node: a local TRS transform, optional mesh, and children.
#[derive(Debug, Clone)]
pub struct ImportedNode {
    pub name: String,
    pub children: Vec<usize>,
    /// Local translation (glTF right-handed, +Y up, meters).
    pub translation: [f32; 3],
    /// Local rotation quaternion, `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub mesh: Option<ImportedMesh>,
    /// Index into [`ImportedScene::skins`], when this node instantiates a
    /// skinned mesh.
    pub skin: Option<usize>,
    /// Initial morph-target weights from the node, falling back to the mesh's
    /// default weights.
    pub morph_weights: Vec<f32>,
    /// Raw `node.extras.yaobow` payload, if present.
    pub extras: Option<Value>,
}

impl ImportedNode {
    pub fn identity(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            children: Vec::new(),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            mesh: None,
            skin: None,
            morph_weights: Vec::new(),
            extras: None,
        }
    }

    /// Whether the node's local transform is the identity transform (no
    /// static translation/rotation, unit scale).
    pub fn is_identity_transform(&self) -> bool {
        self.translation == [0.0; 3]
            && self.rotation == [0.0, 0.0, 0.0, 1.0]
            && self.scale == [1.0; 3]
    }
}

#[derive(Debug, Clone)]
pub struct ImportedMesh {
    pub name: String,
    pub primitives: Vec<ImportedPrimitive>,
}

/// One indexed triangle-list primitive with its vertex attributes already
/// resolved to plain `f32` arrays (widened from whatever component type
/// the source accessor used).
#[derive(Debug, Clone)]
pub struct ImportedPrimitive {
    pub positions: Vec<[f32; 3]>,
    /// Empty when the primitive has no `NORMAL` attribute.
    pub normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    /// Base-color texture file name (relative URI, percent-decoded), if the
    /// primitive's material has one.
    pub material_texture: Option<String>,
    /// Whether the primitive's material uses alpha blending
    /// (`material.alphaMode == "BLEND"`).
    pub material_alpha_blend: bool,
    pub morph_targets: Vec<ImportedMorphTarget>,
    /// Per-vertex joint influences. Joint indices address the enclosing
    /// node's [`ImportedSkin::joints`] array.
    pub skin_influences: Option<Vec<Vec<ImportedJointInfluence>>>,
}

impl ImportedPrimitive {
    /// Whether `self` and `other` carry bit-identical vertex attribute
    /// data (positions/normals/uv0/morph target deltas), ignoring
    /// material/topology fields (`indices`, `material_texture`,
    /// `material_alpha_blend`).
    ///
    /// A Yaobow `.pol`/`.cvd` export puts every material's `Primitive`
    /// for one node on the *same* glTF vertex accessors (see
    /// `exporters::gltf::pol`/`exporters::gltf::cvd`), so re-reading each
    /// primitive's attributes (as [`super::loader::load_primitive`] does)
    /// yields the full, byte-identical shared array once per material
    /// instead of a distinct per-material slice. `pol`/`cvd` importers
    /// use this to pool that shared array once per node rather than
    /// duplicating it per material, while still appending a fresh block
    /// for primitives that carry genuinely distinct vertex data (plain,
    /// hand-authored glTF).
    pub fn shares_vertex_data(&self, other: &Self) -> bool {
        self.positions == other.positions
            && self.normals == other.normals
            && self.uv0 == other.uv0
            && self.morph_targets == other.morph_targets
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMorphTarget {
    /// Per-vertex position displacement relative to the base primitive.
    pub position_deltas: Vec<[f32; 3]>,
    pub normal_deltas: Option<Vec<[f32; 3]>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportedJointInfluence {
    pub joint: u16,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct ImportedSkin {
    /// glTF node indices for each skin-local joint index.
    pub joints: Vec<usize>,
    /// Column-major inverse-bind matrices, one per joint.
    pub inverse_bind_matrices: Vec<[[f32; 4]; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
}

#[derive(Debug, Clone)]
pub struct ImportedAnimation {
    pub name: String,
    pub weight_channels: Vec<ImportedWeightsChannel>,
    pub trs_channels: Vec<ImportedTrsChannel>,
}

/// A `weights` animation channel targeting a node's morph targets.
#[derive(Debug, Clone)]
pub struct ImportedWeightsChannel {
    pub node: usize,
    pub times: Vec<f32>,
    /// Flattened `times.len() * target_count` weights, row-major by
    /// keyframe (matches the glTF `weights` sampler output layout).
    pub weights: Vec<f32>,
    pub target_count: usize,
    pub interpolation: Interpolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrsProperty {
    Translation,
    Rotation,
    Scale,
}

/// A `translation`/`rotation`/`scale` animation channel targeting one node.
#[derive(Debug, Clone)]
pub struct ImportedTrsChannel {
    pub node: usize,
    pub property: TrsProperty,
    pub times: Vec<f32>,
    /// Translation/scale use `[x, y, z]`; rotation uses `[x, y, z, w]`.
    pub values: Vec<[f32; 4]>,
    pub interpolation: Interpolation,
}
