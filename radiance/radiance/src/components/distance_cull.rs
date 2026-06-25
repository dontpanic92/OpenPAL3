//! Camera distance-cull component.
//!
//! [`DistanceCullComponent`] toggles its owning entity's visibility every
//! frame: the entity is shown only while the active scene camera is within
//! `radius` world units of a fixed `center`. This reproduces the original
//! engines' draw-distance culling — PAL5's `GrassDist` is the motivating
//! case. Detail that stacks badly in the distance (grass blades drawn as
//! upright cards) is partitioned into spatial chunks, each chunk entity
//! carrying one of these components anchored at the chunk centre with a
//! radius covering the chunk's own extent plus the desired draw distance.
//! Only the chunks near the viewer are then submitted, so the field reads
//! as grass around the player and fades to bare ground beyond, instead of
//! walling up to the horizon.
//!
//! The component self-ticks via [`IComponent::on_updating`], dispatched by
//! the scene *before* `update_world_transform`, so the visibility set here
//! takes effect in the same frame's render. The camera position is the
//! process-global value published once per frame by
//! `CoreRadianceEngine::update` (shared with [`super::billboard`]).

use crosscom::ComRc;

use crate::comdef::{IComponentImpl, IDistanceCullComponentImpl, IEntity};
use crate::math::Vec3;

use super::billboard::camera_position;

ComObject_DistanceCullComponent!(super::DistanceCullComponent);

pub struct DistanceCullComponent {
    entity: ComRc<IEntity>,
    center: Vec3,
    radius_sq: f32,
}

impl DistanceCullComponent {
    /// Cull `entity` when the camera is farther than `radius` from `center`.
    pub fn create(
        entity: ComRc<IEntity>,
        center: Vec3,
        radius: f32,
    ) -> ComRc<crate::comdef::IDistanceCullComponent> {
        ComRc::from_object(Self {
            entity,
            center,
            radius_sq: radius * radius,
        })
    }

    fn apply(&self) {
        let cam = camera_position();
        let d = Vec3::sub(&cam, &self.center);
        let visible = d.norm2() <= self.radius_sq;
        if self.entity.visible() != visible {
            self.entity.set_visible(visible);
        }
    }
}

impl IDistanceCullComponentImpl for DistanceCullComponent {}

impl IComponentImpl for DistanceCullComponent {
    fn on_loading(&self) -> crosscom::Void {
        self.apply();
    }

    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {
        self.apply();
    }

    fn on_unloading(&self) {}
}
