use std::ffi::c_void;
use std::io::{BufReader, Cursor, Read};
use std::rc::Rc;

use crosscom::{ComRc, IObjectArray, IUnknown, ObjectArray};
use image::AnimationDecoder;
use mini_fs::{MiniFs, StoreExt};
use radiance::rendering::ComponentFactory;

use crate::comdef::services::{ITexture, ITextureImpl, ITextureService, ITextureServiceImpl};

pub struct TextureService {
    _factory: Rc<dyn ComponentFactory>,
    vfs: Rc<MiniFs>,
}

ComObject_TextureService!(super::TextureService);

impl TextureService {
    pub fn create(factory: Rc<dyn ComponentFactory>, vfs: Rc<MiniFs>) -> ComRc<ITextureService> {
        ComRc::from_object(Self {
            _factory: factory,
            vfs,
        })
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let file = self.vfs.open(path).ok()?;
        let mut bytes = Vec::new();
        BufReader::new(file).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }
}

impl ITextureServiceImpl for TextureService {
    fn load_png(&self, vfs_path: &str) -> Option<ComRc<ITexture>> {
        let bytes = self.read(vfs_path)?;
        let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (width, height) = image.dimensions();
        Some(Texture::create(width, height, image.into_raw(), 0))
    }

    fn load_gif_frames(&self, vfs_path: &str) -> Option<ComRc<IObjectArray>> {
        let bytes = self.read(vfs_path)?;
        let decoder = image::gif::GifDecoder::new(Cursor::new(bytes)).ok()?;
        let frames = decoder.into_frames().collect_frames().ok()?;
        let mut objects: Vec<ComRc<IUnknown>> = Vec::new();
        for frame in frames {
            let buffer = frame.into_buffer();
            let (width, height) = buffer.dimensions();
            let texture = Texture::create(width, height, buffer.into_raw(), 0);
            objects.push(texture.query_interface::<IUnknown>()?);
        }
        let raw: *const *const c_void = ObjectArray::<IUnknown>::new(objects).into();
        Some(ComRc::<IObjectArray>::from(raw))
    }
}

pub struct Texture {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    imgui_id: u32,
}

ComObject_Texture!(super::Texture);

impl Texture {
    pub fn create(width: u32, height: u32, pixels: Vec<u8>, imgui_id: u32) -> ComRc<ITexture> {
        ComRc::from_object(Self {
            width,
            height,
            pixels,
            imgui_id,
        })
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn extent(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl ITextureImpl for Texture {
    fn width(&self) -> i32 {
        self.width as i32
    }
    fn height(&self) -> i32 {
        self.height as i32
    }
    fn imgui_id(&self) -> i32 {
        // Intentionally 0 until ImguiTextureCache uploads this texture externally.
        self.imgui_id as i32
    }
}
