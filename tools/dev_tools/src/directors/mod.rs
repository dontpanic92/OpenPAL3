mod components;
mod main_content;
mod main_director;
mod preview_director;

use std::path::PathBuf;

pub use main_director::DevToolsDirector;
pub use preview_director::PreviewDirector;

#[derive(Debug, Clone)]
pub enum DevToolsState {
    MainWindow,
    Preview(PathBuf),
}
