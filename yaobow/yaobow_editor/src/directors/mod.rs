pub mod main_content;
mod main_director;

use crosscom::ComRc;
pub use main_director::DevToolsDirector;
use radiance::comdef::IEntity;

#[derive(Clone)]
pub enum DevToolsState {
    MainWindow,
    PreviewEntity(ComRc<IEntity>),
    PreviewScene { cpk_name: String, scn_name: String },
}
