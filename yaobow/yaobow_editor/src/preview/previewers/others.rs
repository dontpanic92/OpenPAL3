use shared::{
    loaders::anm::load_anm,
    openpal3::loaders::{
        nav_loader::nav_load_from_file, sce_loader::sce_load_from_file,
        scn_loader::scn_load_from_file,
    },
};

use super::{
    get_extension, jsonify,
    text::{TextContentLoader, TextPreviewer},
};

pub struct OthersPreviewer;
impl OthersPreviewer {
    pub fn create() -> TextPreviewer {
        TextPreviewer::new_with_loader(Box::new(OthersTextContentLoader {}))
    }
}

struct OthersTextContentLoader;

impl TextContentLoader for OthersTextContentLoader {
    fn is_supported(&self, path: &std::path::Path) -> bool {
        let extension = get_extension(path);
        match extension.as_deref() {
            Some("scn" | "nav" | "sce" | "anm") => true,
            _ => false,
        }
    }

    fn load(&self, vfs: &mini_fs::MiniFs, path: &std::path::Path) -> String {
        let result = try_load(vfs, path);
        match result {
            Ok(text) => text,
            Err(e) => e.to_string(),
        }
    }
}

fn try_load(vfs: &mini_fs::MiniFs, path: &std::path::Path) -> anyhow::Result<String> {
    let extension = get_extension(path);
    let text = match extension.as_deref() {
        Some("scn") => jsonify(&scn_load_from_file(vfs, path)),
        Some("nav") => jsonify(&nav_load_from_file(vfs, path)),
        Some("sce") => jsonify(&sce_load_from_file(vfs, path)),
        Some("anm") => jsonify(&load_anm(vfs, path)?),
        _ => "Unsupported".to_string(),
    };

    Ok(text)
}
