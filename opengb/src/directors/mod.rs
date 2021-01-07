mod exp_director;
mod persistence_state;
mod sce_commands;
mod sce_director;
mod sce_state;
mod shared_state;

use crate::scene::ScnScene;
pub use exp_director::ExplorationDirector;
pub use persistence_state::PersistenceState;
use radiance::{
    math::Vec3,
    scene::{CoreScene, SceneManager},
};
pub use sce_director::SceDirector;
pub use shared_state::SharedState;

pub trait SceneManagerExtensions: SceneManager {
    fn scene_mut_or_fail(&mut self) -> &mut CoreScene<ScnScene> {
        self.scene_mut()
            .expect("No scene loaded. Probably a bug in Sce procedures.")
            .downcast_mut::<CoreScene<ScnScene>>()
            .unwrap()
    }
}

impl<T: SceneManager + ?Sized> SceneManagerExtensions for T {}
