use alloc::string::String;

use super::image::RgbaImage;

pub trait Texture: downcast_rs::Downcast {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

downcast_rs::impl_downcast!(Texture);

pub struct TextureDef {
    name: String,
    image: Option<RgbaImage>,
}

impl TextureDef {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn image(&self) -> Option<&RgbaImage> {
        self.image.as_ref()
    }
}

pub use internal::TextureStore;

#[cfg(feature = "std")]
mod internal {
    use crate::rendering::{image::RgbaImage, TextureDef};
    use lru::LruCache;
    use std::sync::Arc;

    lazy_static::lazy_static! {
        static ref TEXTURE_STORE: std::sync::RwLock<LruCache<String, Arc<TextureDef>>> = std::sync::RwLock::new(LruCache::new(100));
    }

    pub struct TextureStore;
    impl TextureStore {
        pub fn get_or_update(
            name: &str,
            update: impl FnOnce() -> Option<RgbaImage>,
        ) -> Arc<TextureDef> {
            let mut store = TEXTURE_STORE.write().unwrap();

            if let Some(t) = store.get(name) {
                t.clone()
            } else {
                let image = update();
                let t = Arc::new(TextureDef {
                    name: name.to_string(),
                    image,
                });
                store.put(name.to_string(), t.clone());
                t
            }
        }
    }
}

#[cfg(feature = "no_std")]
mod internal {
    lazy_static::lazy_static! {
        static ref TEXTURE_STORE: spin::RwLock<LruCache<String, Arc<TextureDef>>> = spin::RwLock::new(LruCache::new(100));
    }

    pub struct TextureStore;
    impl TextureStore {
        pub fn get_or_update(
            name: &str,
            update: impl FnOnce() -> Option<RgbaImage>,
        ) -> Arc<TextureDef> {
            let mut store = TEXTURE_STORE.write();

            if let Some(t) = store.get(name) {
                t.clone()
            } else {
                let image = update();
                let t = Arc::new(TextureDef {
                    name: name.to_string(),
                    image,
                });
                store.put(name.to_string(), t.clone());
                t
            }
        }
    }
}
