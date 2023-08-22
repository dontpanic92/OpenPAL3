pub mod main_content;
pub mod main_director;
pub mod welcome_page;

use crosscom::ComRc;
pub use main_director::DevToolsDirector;
use radiance::comdef::IEntity;

#[allow(dead_code)]
#[derive(Clone)]
pub enum DevToolsState {
    MainWindow,
    PreviewEntity(ComRc<IEntity>),
    PreviewScene { cpk_name: String, scn_name: String },
}
