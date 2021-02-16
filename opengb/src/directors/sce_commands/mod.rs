mod _let;
mod call;
mod camera_default;
mod camera_move;
mod camera_set;
mod cmp;
mod dlg;
mod fop;
mod get_appr;
mod goto;
mod hy_fly;
mod idle;
mod load_scene;
mod music;
mod nop;
mod object_active;
mod play_sound;
mod rnd;
mod role_act_auto_stand;
mod role_active;
mod role_face_role;
mod role_move_back;
mod role_move_to;
mod role_path_out;
mod role_path_to;
mod role_set_face;
mod role_set_pos;
mod role_show_action;
mod role_turn_face;
mod script_run_mode;
mod stop_music;
mod testgoto;

pub use _let::SceCommandLet;
pub use call::SceCommandCall;
pub use camera_default::SceCommandCameraDefault;
pub use camera_move::SceCommandCameraMove;
pub use camera_set::SceCommandCameraSet;
pub use cmp::{
    SceCommandEq, SceCommandGeq, SceCommandGt, SceCommandLeq, SceCommandLs, SceCommandNeq,
};
pub use dlg::SceCommandDlg;
pub use fop::SceCommandFop;
pub use get_appr::SceCommandGetAppr;
pub use goto::SceCommandGoto;
pub use hy_fly::SceCommandHyFly;
pub use idle::SceCommandIdle;
pub use load_scene::SceCommandLoadScene;
pub use music::SceCommandMusic;
pub use nop::SceCommandNop;
pub use object_active::SceCommandObjectActive;
pub use play_sound::SceCommandPlaySound;
pub use rnd::SceCommandRnd;
pub use role_act_auto_stand::SceCommandRoleActAutoStand;
pub use role_active::SceCommandRoleActive;
pub use role_face_role::SceCommandRoleFaceRole;
pub use role_move_back::SceCommandRoleMoveBack;
pub use role_move_to::SceCommandRoleMoveTo;
pub use role_path_out::SceCommandRolePathOut;
pub use role_path_to::SceCommandRolePathTo;
pub use role_set_face::SceCommandRoleSetFace;
pub use role_set_pos::SceCommandRoleSetPos;
pub use role_show_action::SceCommandRoleShowAction;
pub use role_turn_face::SceCommandRoleTurnFace;
pub use script_run_mode::SceCommandScriptRunMode;
pub use stop_music::SceCommandStopMusic;
pub use testgoto::SceCommandTestGoto;

use radiance::math::Vec3;

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
