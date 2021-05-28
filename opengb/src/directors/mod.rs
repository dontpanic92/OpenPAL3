mod adv_director;
mod global_state;
mod persistent_state;
mod sce_commands;
mod sce_vm;

use self::sce_vm::SceState;
use crate::scene::{RoleEntity, ScnScene};
pub use adv_director::AdventureDirector;
pub use global_state::GlobalState;
pub use persistent_state::PersistentState;
use radiance::scene::{CoreEntity, CoreScene, SceneManager};

pub trait SceneManagerExtensions: SceneManager {
    fn core_scene_mut(&mut self) -> Option<&mut CoreScene<ScnScene>> {
        self.scene_mut()
            .expect("No scene loaded. Probably a bug in Sce procedures.")
            .downcast_mut::<CoreScene<ScnScene>>()
    }

    fn core_scene_mut_or_fail(&mut self) -> &mut CoreScene<ScnScene> {
        self.core_scene_mut().unwrap()
    }

    fn get_resolved_role_entity(
        &mut self,
        state: &SceState,
        role_id: i32,
    ) -> &CoreEntity<RoleEntity> {
        self.core_scene_mut_or_fail()
            .get_role_entity(resolve_role_id(state, role_id))
    }

    fn get_resolved_role_entity_mut(
        &mut self,
        state: &SceState,
        role_id: i32,
    ) -> &mut CoreEntity<RoleEntity> {
        let resolved_role_id = if role_id == -1 {
            state.global_state().role_controlled()
        } else {
            role_id
        };
        self.core_scene_mut_or_fail()
            .get_role_entity_mut(resolve_role_id(state, role_id))
    }
}

impl<T: SceneManager + ?Sized> SceneManagerExtensions for T {}

fn resolve_role_id(state: &SceState, role_id: i32) -> i32 {
    if role_id == -1 {
        state.global_state().role_controlled()
    } else {
        role_id
    }
}
