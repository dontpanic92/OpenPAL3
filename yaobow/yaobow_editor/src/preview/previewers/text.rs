use std::path::Path;

use common::store_ext::StoreExt2;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use mini_fs::MiniFs;

use crate::{directors::main_content::ContentTab, preview::panes::TextPane};

use super::{get_extension, Previewer};

pub struct TextPreviewer {
    content_loader: Box<dyn TextContentLoader>,
}

impl TextPreviewer {
    pub fn new() -> Self {
        Self {
            content_loader: Box::new(PlainTextContentLoader {}),
        }
    }

    pub fn new_with_loader(content_loader: Box<dyn TextContentLoader>) -> Self {
        Self { content_loader }
    }
}

impl Previewer for TextPreviewer {
    fn open(&self, vfs: &MiniFs, path: &Path) -> Option<ContentTab> {
        if !self.content_loader.is_supported(path) {
            None
        } else {
            let content = self.content_loader.load(vfs, path);
            Some(ContentTab::new(
                path.to_string_lossy().to_string(),
                Box::new(TextPane::new(content, path.to_owned(), None, None)),
            ))
        }
    }
}

pub trait TextContentLoader {
    fn is_supported(&self, path: &Path) -> bool;
    fn load(&self, vfs: &MiniFs, path: &Path) -> String;
}

struct PlainTextContentLoader;

impl TextContentLoader for PlainTextContentLoader {
    fn load(&self, vfs: &MiniFs, path: &Path) -> String {
        vfs.read_to_end(path)
            .and_then(|v| {
                let result = chardet::detect(&v);
                let coder = encoding_from_whatwg_label(chardet::charset2encoding(&result.0))
                    .unwrap_or(encoding::all::GBK);
                Ok(coder
                    .decode(&v, DecoderTrap::Ignore)
                    .unwrap_or("Cannot read the file as GBK encoded text content".to_string()))
            })
            .unwrap_or("Cannot open this file".to_string())
    }

    fn is_supported(&self, path: &Path) -> bool {
        let extension = get_extension(path);
        match extension.as_deref() {
            Some("h" | "asm" | "ini" | "txt" | "conf" | "cfg" | "log") => true,
            _ => false,
        }
    }
}
