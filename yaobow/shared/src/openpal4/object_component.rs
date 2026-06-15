//! Host-implemented `IPal4ObjectComponent`.
//!
//! Per-object metadata carried on each loaded GOB object entity.
//! Replaces the old index-parallel `objects_gob_indices` /
//! `objects_initial_transforms` Vecs on [`Pal4Scene`] *and* the
//! `objects_gob` lookup table: object entities are discovered via the
//! engine tag query (`TAG_OBJECT`), and the bits that aren't recoverable
//! from the entity itself — the authoring [`GobEntry`], its per-index
//! object-type tag (from `GobHeader::object_types`), and the load-time
//! transform `giGOBReset` restores to — ride on the entity as a
//! component. Nothing external indexes back into the parsed `GobFile`.
//!
//! The component is read back through the inherent Rust impl via
//! `query_interface().inner::<Pal4ObjectComponent>()` (the same pattern
//! used by `TriggerVolumeComponent`); the COM interface itself carries
//! no methods.
//!
//! [`Pal4Scene`]: super::scene::Pal4Scene

use crosscom::ComRc;
use fileformats::pal4::gob::GobEntry;
use radiance::math::Mat44;

use super::comdef::{IPal4ObjectComponent, IPal4ObjectComponentImpl};

pub struct Pal4ObjectComponent {
    /// The authoring `GobEntry` (cloned at load) that produced this
    /// object. Carries everything callers used to recover via
    /// `objects_gob.entries.get(gob_index)`: `folder` for sibling
    /// `.anm`/`.dff` lookups, `research_function`/`trigger_distance`
    /// for the proximity-interaction trigger, the authored
    /// position/rotation, etc.
    entry: GobEntry,

    /// The object's GOB type tag (`GobObjectType::*`), sourced from
    /// `GobHeader::object_types[i]`. It is stored per-index in the
    /// header rather than on `GobEntry`, so it is captured separately
    /// here for the `/v1/state` `kind` field.
    object_type: u32,

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
    pub fn create(
        entry: GobEntry,
        object_type: u32,
        initial_transform: Mat44,
    ) -> ComRc<IPal4ObjectComponent> {
        ComRc::from_object(Self {
            entry,
            object_type,
            initial_transform,
        })
    }

    /// The full authoring `GobEntry`.
    pub fn entry(&self) -> &GobEntry {
        &self.entry
    }

    /// The object's GOB type tag (`GobObjectType::*`).
    pub fn object_type(&self) -> u32 {
        self.object_type
    }

    /// The raw GOB `folder` (e.g. `gamedata\PALObject\OM01\`) that
    /// authored this object, used to locate its sibling `.anm`/`.dff`.
    /// Empty string if the field fails to decode.
    pub fn folder(&self) -> String {
        self.entry.folder.to_string().unwrap_or_default()
    }

    /// The script function invoked on the player's "Examine"/"Research"
    /// action; empty string means no handler.
    pub fn research_function(&self) -> String {
        self.entry.research_function.to_string().unwrap_or_default()
    }

    /// Trigger / culling distance in world units.
    pub fn trigger_distance(&self) -> f32 {
        self.entry.trigger_distance
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
