mod camera_set;
mod dlg;
mod idle;
mod music;
mod play_sound;
mod role_active;
mod role_face_role;
mod role_path_to;
mod role_set_face;
mod role_set_pos;
mod role_show_action;
mod run_script_mode;

pub use camera_set::SceCommandCameraSet;
pub use dlg::SceCommandDlg;
pub use idle::SceCommandIdle;
pub use music::SceCommandMusic;
pub use play_sound::SceCommandPlaySound;
pub use role_active::SceCommandRoleActive;
pub use role_face_role::SceCommandRoleFaceRole;
pub use role_path_to::SceCommandRolePathTo;
pub use role_set_face::SceCommandRoleSetFace;
pub use role_set_pos::SceCommandRoleSetPos;
pub use role_show_action::SceCommandRoleShowAction;
pub use run_script_mode::SceCommandRunScriptMode;

use super::sce_state::SceState;
use crate::scene::{Mv3ModelEntity, ScnScene};
use radiance::{
    math::Vec3,
    scene::{CoreEntity, CoreScene, Scene},
};

struct RoleProperties;
impl RoleProperties {
    pub fn position(state: &mut SceState, role_id: &str) -> Vec3 {
        let key = RolePropertyNames::position(role_id);
        if !state.ext_mut().contains_key(&key) {
            state
                .ext_mut()
                .insert(key.clone(), Box::new(Vec3::new(0., 0., 0.)));
        }

        *state
            .ext_mut()
            .get(&key)
            .as_ref()
            .unwrap()
            .downcast_ref::<Vec3>()
            .unwrap()
    }

    pub fn set_position(state: &mut SceState, role_id: &str, position: &Vec3) {
        let key = RolePropertyNames::position(role_id);
        state.ext_mut().insert(key, Box::new(*position));
    }

    pub fn face_to(state: &mut SceState, role_id: &str) -> Vec3 {
        let key = RolePropertyNames::face_to(role_id);
        if !state.ext_mut().contains_key(&key) {
            state
                .ext_mut()
                .insert(key.clone(), Box::new(Vec3::new(0., 0., -1.)));
        }

        *state
            .ext_mut()
            .get(&key)
            .as_ref()
            .unwrap()
            .downcast_ref::<Vec3>()
            .unwrap()
    }

    pub fn set_face_to(state: &mut SceState, role_id: &str, face_to: &Vec3) {
        let key = RolePropertyNames::face_to(role_id);
        state.ext_mut().insert(key, Box::new(*face_to));
    }
}

struct RolePropertyNames;
impl RolePropertyNames {
    pub fn name(role_id: &str) -> String {
        format!("ROLE_{}", role_id)
    }

    pub fn position(role_id: &str) -> String {
        format!("ROLE_{}_POSITION", role_id)
    }

    pub fn face_to(role_id: &str) -> String {
        format!("ROLE_{}_FACE_TO", role_id)
    }
}

const BLOCK_SIZE: f32 = 12.5;
pub fn nav_coord_to_scene_coord(scene: &CoreScene<ScnScene>, nav_position: &Vec3) -> Vec3 {
    let ext = scene.extension().borrow();
    let origin = ext.nav_origin();
    Vec3::new(
        nav_position.x * BLOCK_SIZE + origin.x,
        nav_position.y + origin.y,
        nav_position.z * BLOCK_SIZE + origin.z,
    )
}

trait SceneMv3Extensions {
    fn get_mv3_entity(&mut self, name: &str) -> &mut CoreEntity<Mv3ModelEntity>;
}

impl SceneMv3Extensions for CoreScene<ScnScene> {
    fn get_mv3_entity(&mut self, name: &str) -> &mut CoreEntity<Mv3ModelEntity> {
        let pos = self
            .entities_mut()
            .iter()
            .position(|e| e.name() == name)
            .unwrap();
        self.entities_mut()
            .get_mut(pos)
            .unwrap()
            .as_mut()
            .downcast_mut::<CoreEntity<Mv3ModelEntity>>()
            .unwrap()
    }
}
