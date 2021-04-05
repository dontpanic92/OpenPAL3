mod adv_director;
mod persistent_state;
mod sce_commands;
mod sce_director;
mod shared_state;

use crate::scene::ScnScene;
pub use adv_director::AdventureDirector;
pub use persistent_state::PersistentState;
use radiance::scene::{CoreScene, SceneManager};
pub use sce_director::SceDirector;
pub use shared_state::SharedState;

pub trait SceneManagerExtensions: SceneManager {
    fn core_scene_mut(&mut self) -> Option<&mut CoreScene<ScnScene>> {
        self.scene_mut()
            .expect("No scene loaded. Probably a bug in Sce procedures.")
            .downcast_mut::<CoreScene<ScnScene>>()
    }

    fn core_scene_mut_or_fail(&mut self) -> &mut CoreScene<ScnScene> {
        self.core_scene_mut().unwrap()
    }
}

impl<T: SceneManager + ?Sized> SceneManagerExtensions for T {}
