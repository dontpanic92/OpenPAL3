use std::sync::{Arc, RwLock};

use image::RgbaImage;
use lru::LruCache;

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

lazy_static::lazy_static! {
    static ref TEXTURE_STORE: RwLock<LruCache<String, Arc<TextureDef>>> = RwLock::new(LruCache::new(10000));
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
