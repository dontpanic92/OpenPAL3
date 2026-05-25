//! Scriptable adapter exposing a `Scene` camera's transform.
//!
//! Wraps a `ComRc<IScene>` and routes `ICameraControl` calls through
//! `Scene::camera_mut().transform_mut()`. Read-side basis accessors
//! report the camera's right (`mat[*][0]`) and forward (`mat[*][2]`)
//! axes so script-side movement math can transform local-space
//! direction vectors into world space without owning the camera
//! matrix.

use crosscom::ComRc;

use crate::comdef::{ICameraControl, ICameraControlImpl, IScene};
use crate::math::Vec3;
use crate::scene::ISceneExt;

pub struct SceneCameraControl {
    scene: ComRc<IScene>,
}

ComObject_SceneCameraControl!(crate::scene::scene_camera_control::SceneCameraControl);

impl SceneCameraControl {
    pub fn create(scene: ComRc<IScene>) -> ComRc<ICameraControl> {
        ComRc::from_object(Self { scene })
    }
}

impl ICameraControlImpl for SceneCameraControl {
    fn set_position(&self, x: f32, y: f32, z: f32) {
        self.scene
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(x, y, z));
    }

    fn look_at(&self, x: f32, y: f32, z: f32) {
        self.scene
            .camera_mut()
            .transform_mut()
            .look_at(&Vec3::new(x, y, z));
    }

    fn forward_x(&self) -> f32 {
        self.scene.camera().transform().matrix()[0][2]
    }
    fn forward_y(&self) -> f32 {
        self.scene.camera().transform().matrix()[1][2]
    }
    fn forward_z(&self) -> f32 {
        self.scene.camera().transform().matrix()[2][2]
    }
    fn right_x(&self) -> f32 {
        self.scene.camera().transform().matrix()[0][0]
    }
    fn right_y(&self) -> f32 {
        self.scene.camera().transform().matrix()[1][0]
    }
    fn right_z(&self) -> f32 {
        self.scene.camera().transform().matrix()[2][0]
    }
}

/// Convenience wrapper mirroring the `wrap_<i>` family from the
/// script-bridge codegen: hands a script a foreign-`ICameraControl`
/// view of `scene`'s camera.
pub fn wrap_scene_camera(scene: ComRc<IScene>) -> ComRc<ICameraControl> {
    SceneCameraControl::create(scene)
}
