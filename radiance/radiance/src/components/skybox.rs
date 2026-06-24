//! Camera-locked skybox component.
//!
//! [`SkyboxComponent`] rewrites its owning entity's *local* translation
//! every frame so the skybox model — a large dome/box authored around the
//! origin — stays centred on the active scene camera. PAL5 maps span tens
//! of thousands of world units (terrain blocks at `r*5120`,`c*5120`), so a
//! skybox left at the origin would fall off-screen as the camera travels;
//! re-centring it on the camera each frame keeps the viewer permanently
//! enclosed, which is exactly how the original engine renders it.
//!
//! Only the translation is touched (`Transform::set_position` writes just
//! the matrix translation column), so the entity keeps whatever rotation
//! and scale it was loaded with — any fixed sky yaw (e.g. PAL5 `SkyRot`)
//! is baked into the model transform once at load time, not here.
//!
//! The component self-ticks via [`IComponent::on_updating`], which the
//! scene dispatches *before* `update_world_transform`, so the transform
//! set here is reflected in the same frame's render. The camera position
//! is the process-global value published once per frame by
//! `CoreRadianceEngine::update` (shared with [`super::billboard`]).

use crosscom::ComRc;

use crate::comdef::{IComponentImpl, IEntity, IEntityExt, ISkyboxComponentImpl};

use super::billboard::camera_position;

ComObject_SkyboxComponent!(super::SkyboxComponent);

pub struct SkyboxComponent {
    entity: ComRc<IEntity>,
}

impl SkyboxComponent {
    pub fn create(entity: ComRc<IEntity>) -> ComRc<crate::comdef::ISkyboxComponent> {
        ComRc::from_object(Self { entity })
    }

    fn apply(&self) {
        let cam = camera_position();
        self.entity.transform().borrow_mut().set_position(&cam);
    }
}

impl ISkyboxComponentImpl for SkyboxComponent {}

impl IComponentImpl for SkyboxComponent {
    fn on_loading(&self) -> crosscom::Void {
        self.apply();
    }

    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {
        self.apply();
    }

    fn on_unloading(&self) {}
}
