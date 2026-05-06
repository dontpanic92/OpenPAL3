use std::collections::HashMap;
use std::mem::{align_of, size_of};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::AtomicU32;

use crosscom::ComRc;
use imgui::TextureId;
use radiance::rendering::{ComponentFactory, Texture as RenderingTexture};

use crate::comdef::services::ITexture;
use crate::ui_walker::TextureResolver;

use super::texture::Texture;

struct CachedTexture {
    _texture: Box<dyn RenderingTexture>,
    id: TextureId,
}

pub struct ImguiTextureCache {
    factory: Rc<dyn ComponentFactory>,
    cache: HashMap<i64, CachedTexture>,
}

impl ImguiTextureCache {
    pub fn new(factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            factory,
            cache: HashMap::new(),
        }
    }

    pub fn upload(&mut self, com_id: i64, tex: ComRc<ITexture>) -> Option<TextureId> {
        if let Some(entry) = self.cache.get(&com_id) {
            return Some(entry.id);
        }

        let tex = concrete_texture(&tex);
        let (width, height) = tex.extent();
        let pixels = tex.pixels();
        if width == 0 || height == 0 || pixels.len() < width as usize * height as usize * 4 {
            log::warn!(
                "refusing to upload invalid scripted texture: com_id={}, extent={}x{}, bytes={}",
                com_id,
                width,
                height,
                pixels.len()
            );
            return None;
        }

        let uploaded = catch_unwind(AssertUnwindSafe(|| {
            self.factory
                .create_imgui_texture(pixels, 0, width, height, None)
        }));

        match uploaded {
            Ok((texture, id)) => {
                self.cache.insert(
                    com_id,
                    CachedTexture {
                        _texture: texture,
                        id,
                    },
                );
                Some(id)
            }
            Err(_) => {
                log::warn!("failed to upload scripted texture: com_id={}", com_id);
                None
            }
        }
    }

    pub fn resolve(&mut self, com_id: i64) -> Option<TextureId> {
        self.cache.get(&com_id).map(|entry| entry.id)
    }
}

impl TextureResolver for ImguiTextureCache {
    fn resolve(&mut self, com_id: i64) -> Option<TextureId> {
        self.resolve(com_id)
    }
}

fn concrete_texture(tex: &ComRc<ITexture>) -> &Texture {
    let inner_offset = align_up(
        size_of::<ITexture>() + size_of::<AtomicU32>(),
        align_of::<Texture>(),
    );
    unsafe { &*((tex.ptr_value() as *const u8).add(inner_offset) as *const Texture) }
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}
