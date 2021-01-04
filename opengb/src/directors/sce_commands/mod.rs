mod _let;
mod camera_default;
mod camera_set;
mod dlg;
mod idle;
mod music;
mod nop;
mod play_sound;
mod role_act_auto_stand;
mod role_active;
mod role_face_role;
mod role_path_to;
mod role_set_face;
mod role_set_pos;
mod role_show_action;
mod role_turn_face;
mod script_run_mode;

pub use _let::SceCommandLet;
pub use camera_default::SceCommandCameraDefault;
pub use camera_set::SceCommandCameraSet;
pub use dlg::SceCommandDlg;
pub use idle::SceCommandIdle;
pub use music::SceCommandMusic;
pub use nop::SceCommandNop;
pub use play_sound::SceCommandPlaySound;
pub use role_act_auto_stand::SceCommandRoleActAutoStand;
pub use role_active::SceCommandRoleActive;
pub use role_face_role::SceCommandRoleFaceRole;
pub use role_path_to::SceCommandRolePathTo;
pub use role_set_face::SceCommandRoleSetFace;
pub use role_set_pos::SceCommandRoleSetPos;
pub use role_show_action::SceCommandRoleShowAction;
pub use role_turn_face::{SceCommandRoleTurnFace, SceCommandRoleTurnFaceA};
pub use script_run_mode::SceCommandScriptRunMode;

use crate::scene::{RoleEntity, ScnScene};
use radiance::{
    math::Vec3,
    scene::{CoreEntity, CoreScene, Scene, SceneManager},
};

struct Direction;
impl Direction {
    const NORTH: Vec3 = Vec3 {
        x: 0.,
        y: 0.,
        z: -1.,
    };
    const NORTHEAST: Vec3 = Vec3 {
        x: 1.,
        y: 0.,
        z: -1.,
    };
    const EAST: Vec3 = Vec3 {
        x: 1.,
        y: 0.,
        z: 0.,
    };
    const SOUTHEAST: Vec3 = Vec3 {
        x: 1.,
        y: 0.,
        z: 1.,
    };
    const SOUTH: Vec3 = Vec3 {
        x: 0.,
        y: 0.,
        z: 1.,
    };
    const SOUTHWEST: Vec3 = Vec3 {
        x: -1.,
        y: 0.,
        z: 1.,
    };
    const WEST: Vec3 = Vec3 {
        x: -1.,
        y: 0.,
        z: 0.,
    };
    const NORTHWEST: Vec3 = Vec3 {
        x: -1.,
        y: 0.,
        z: -1.,
    };
}

pub fn map_role_id(role_id: i32) -> i32 {
    match role_id {
        -1 => 101,
        1 => 104,
        x => x,
    }
}

const BLOCK_SIZE: f32 = 12.5;
pub fn nav_coord_to_scene_coord(scene: &CoreScene<ScnScene>, nav_x: f32, nav_y: f32) -> Vec3 {
    let ext = scene.extension().borrow();
    let origin = ext.nav_origin();
    Vec3::new(
        nav_x * BLOCK_SIZE + origin.x,
        origin.y,
        nav_y * BLOCK_SIZE + origin.z,
    )
}

trait SceneRoleExtensions {
    fn get_role_entity(&mut self, name: &str) -> &mut CoreEntity<RoleEntity>;
}

impl SceneRoleExtensions for CoreScene<ScnScene> {
    fn get_role_entity(&mut self, name: &str) -> &mut CoreEntity<RoleEntity> {
        let pos = self
            .entities_mut()
            .iter()
            .position(|e| e.name() == name)
            .unwrap();
        self.entities_mut()
            .get_mut(pos)
            .unwrap()
            .as_mut()
            .downcast_mut::<CoreEntity<RoleEntity>>()
            .unwrap()
    }
}

pub trait SceneManagerExtensions: SceneManager {
    fn scene_mut_or_fail(&mut self) -> &mut CoreScene<ScnScene> {
        self.scene_mut()
            .expect("No scene loaded. Probably a bug in Sce procedures.")
            .downcast_mut::<CoreScene<ScnScene>>()
            .unwrap()
    }
}

impl<T: SceneManager + ?Sized> SceneManagerExtensions for T {}
