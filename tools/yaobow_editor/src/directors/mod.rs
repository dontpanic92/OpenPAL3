mod components;
mod main_content;
mod main_director;

use std::path::PathBuf;

pub use main_director::DevToolsDirector;

#[derive(Debug, Clone)]
pub enum DevToolsState {
    _MainWindow,
    Preview(PathBuf),
}
