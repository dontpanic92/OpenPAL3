use std::path::Path;

use mini_fs::MiniFs;
use serde::Serialize;

use crate::directors::main_content::ContentTab;

pub mod audio;
pub mod image;
pub mod models;
pub mod others;
pub mod text;
pub mod video;

pub trait Previewer {
    fn open(&self, vfs: &MiniFs, path: &Path) -> Option<ContentTab>;
}

fn get_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
}

fn jsonify<T: ?Sized + Serialize>(t: &T) -> String {
    serde_json::to_string_pretty(t).unwrap_or("Cannot serialize into Json".to_string())
}
