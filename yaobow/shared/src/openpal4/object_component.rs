//! Host-implemented `IPal4ObjectComponent`.
//!
//! Per-object metadata carried on each loaded GOB object entity.
//! Replaces the old index-parallel `objects_gob_indices` /
//! `objects_initial_transforms` Vecs on [`Pal4Scene`]: object entities
//! are now discovered via the engine tag query (`TAG_OBJECT`) instead
//! of a hand-kept handle Vec, so the bits that aren't recoverable from
//! the entity itself — its index into the block `GobFile` and the
//! authored load-time transform `giGOBReset` restores to — ride on the
//! entity as a component.
//!
//! The component is read back through the inherent Rust impl via
//! `query_interface().inner::<Pal4ObjectComponent>()` (the same pattern
//! used by `TriggerVolumeComponent`); the COM interface itself carries
//! no methods.
//!
//! [`Pal4Scene`]: super::scene::Pal4Scene

use crosscom::ComRc;
use radiance::math::Mat44;

use super::comdef::{IPal4ObjectComponent, IPal4ObjectComponentImpl};

pub struct Pal4ObjectComponent {
    /// Index of the authoring entry in the block `GobFile::entries`.
    /// Used to recover the object's `folder` for sibling `.anm`/`.dff`
    /// lookups (`get_object_folder`).
    gob_index: usize,

    /// The object's full `Transform` matrix captured at scene load.
    /// `giGOBReset` restores the entity to this matrix after any number
    /// of `giGOBSetPosition` / `giGOBMovment` / `giGOBScale` calls. We
    /// snapshot the whole `Mat44` (not a decomposed pos/rot/scale) to
    /// sidestep the multiplicative load-time transform chain and gimbal
    /// lock — see the historical note in `Pal4Scene`.
    initial_transform: Mat44,
}

ComObject_Pal4ObjectComponent!(super::Pal4ObjectComponent);

impl Pal4ObjectComponent {
    pub fn create(gob_index: usize, initial_transform: Mat44) -> ComRc<IPal4ObjectComponent> {
        ComRc::from_object(Self {
            gob_index,
            initial_transform,
        })
    }

    pub fn gob_index(&self) -> usize {
        self.gob_index
    }

    pub fn initial_transform(&self) -> Mat44 {
        self.initial_transform
    }
}

impl IPal4ObjectComponentImpl for Pal4ObjectComponent {}

impl radiance::comdef::IComponentImpl for Pal4ObjectComponent {
    fn on_loading(&self) -> crosscom::Void {}
    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {}
    fn on_unloading(&self) {}
}
