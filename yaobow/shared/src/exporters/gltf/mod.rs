//! glTF 2.0 (`.glb`) exporters for PAL3 models.
//!
//! PAL3 uses **vertex animation** (per-frame position snapshots), not
//! skeletal animation, so the natural glTF representation is one base
//! mesh + one **morph target** per remaining frame, driven by a
//! STEP-interpolated `weights` animation that snaps to the original
//! per-frame timing. Static `pol` meshes degrade to a no-animation
//! single primitive per material group. `cvd` composes both: a node
//! hierarchy of geometry parts where each part may have its own
//! morph-target animation **and** TRS keyframes.
//!
//! All exporters embed referenced textures into the `.glb` (as PNG, or
//! pass-through for already-PNG/JPEG sources) so the output is fully
//! self-contained and round-trips cleanly through Blender / Maya /
//! Unity / Unreal.

pub mod cvd;
pub mod glb;
pub mod mv3;
pub mod pol;
pub mod textures;

pub use cvd::export_cvd_to_glb;
pub use mv3::export_mv3_to_glb;
pub use pol::export_pol_to_glb;
