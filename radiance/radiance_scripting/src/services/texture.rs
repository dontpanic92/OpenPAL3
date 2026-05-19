use std::ffi::c_void;
use std::io::{BufReader, Cursor, Read};
use std::rc::Rc;
use std::time::Duration;

use crosscom::{ComRc, IObjectArray, IUnknown, ObjectArray};
use image::AnimationDecoder;
use mini_fs::{MiniFs, StoreExt};
use radiance::rendering::{ComponentFactory, Texture as RenderingTexture};

use crate::comdef::services::{
    IGifAnimation, IGifAnimationImpl, ITexture, ITextureImpl, ITextureService, ITextureServiceImpl,
};

pub struct TextureService {
    factory: Rc<dyn ComponentFactory>,
    vfs: Rc<MiniFs>,
}

ComObject_TextureService!(super::TextureService);

impl TextureService {
    pub fn create(factory: Rc<dyn ComponentFactory>, vfs: Rc<MiniFs>) -> ComRc<ITextureService> {
        ComRc::from_object(Self { factory, vfs })
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
        Some(Texture::create_uploaded(
            &self.factory,
            width,
            height,
            image.into_raw(),
        ))
    }

    fn load_gif_frames(&self, vfs_path: &str) -> Option<ComRc<IObjectArray>> {
        let bytes = self.read(vfs_path)?;
        let decoder = image::gif::GifDecoder::new(Cursor::new(bytes)).ok()?;
        let frames = decoder.into_frames().collect_frames().ok()?;
        let mut objects: Vec<ComRc<IUnknown>> = Vec::new();
        for frame in frames {
            let buffer = frame.into_buffer();
            let (width, height) = buffer.dimensions();
            let texture = Texture::create_uploaded(&self.factory, width, height, buffer.into_raw());
            objects.push(texture.query_interface::<IUnknown>()?);
        }
        let raw: *const *const c_void = ObjectArray::<IUnknown>::new(objects).into();
        Some(ComRc::<IObjectArray>::from(raw))
    }

    fn load_gif_animation(&self, vfs_path: &str) -> Option<ComRc<IGifAnimation>> {
        let bytes = self.read(vfs_path)?;
        let decoder = image::gif::GifDecoder::new(Cursor::new(bytes)).ok()?;
        let raw_frames = decoder.into_frames().collect_frames().ok()?;
        let mut frames: Vec<ComRc<ITexture>> = Vec::with_capacity(raw_frames.len());
        let mut delays_ms: Vec<i32> = Vec::with_capacity(raw_frames.len());
        for frame in raw_frames {
            let delay: Duration = frame.delay().into();
            // Saturating cast: even a 24-day delay still fits, but a
            // pathological value shouldn't underflow the i32 the IDL
            // surfaces.
            let ms = delay.as_millis().min(i32::MAX as u128) as i32;
            let buffer = frame.into_buffer();
            let (width, height) = buffer.dimensions();
            frames.push(Texture::create_uploaded(
                &self.factory,
                width,
                height,
                buffer.into_raw(),
            ));
            delays_ms.push(ms);
        }
        Some(GifAnimation::create(frames, delays_ms))
    }
}

pub struct Texture {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    imgui_id: u32,
    _imgui_texture: Option<Box<dyn RenderingTexture>>,
}

ComObject_Texture!(super::Texture);

impl Texture {
    pub fn create(width: u32, height: u32, pixels: Vec<u8>, imgui_id: u32) -> ComRc<ITexture> {
        ComRc::from_object(Self {
            width,
            height,
            pixels,
            imgui_id,
            _imgui_texture: None,
        })
    }

    pub fn create_uploaded(
        factory: &Rc<dyn ComponentFactory>,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> ComRc<ITexture> {
        let (imgui_texture, imgui_id) =
            factory.create_imgui_texture(&pixels, 0, width, height, None);
        ComRc::from_object(Self {
            width,
            height,
            pixels,
            imgui_id: imgui_id.id() as u32,
            _imgui_texture: Some(imgui_texture),
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
        self.imgui_id as i32
    }
}

/// Frame-by-frame GIF animation with per-frame delays. Each frame is a
/// separate `ITexture` with its own imgui upload. Delays are reported
/// in milliseconds.
pub struct GifAnimation {
    frames: Vec<ComRc<ITexture>>,
    delays_ms: Vec<i32>,
}

ComObject_GifAnimation!(super::GifAnimation);

impl GifAnimation {
    pub fn create(frames: Vec<ComRc<ITexture>>, delays_ms: Vec<i32>) -> ComRc<IGifAnimation> {
        ComRc::from_object(Self { frames, delays_ms })
    }
}

impl IGifAnimationImpl for GifAnimation {
    fn frame_count(&self) -> i32 {
        self.frames.len() as i32
    }

    fn frame_at(&self, i: i32) -> ComRc<ITexture> {
        let idx = (i as usize).min(self.frames.len().saturating_sub(1));
        self.frames[idx].clone()
    }

    fn delay_ms(&self, i: i32) -> i32 {
        if self.delays_ms.is_empty() {
            return 0;
        }
        let idx = (i as usize).min(self.delays_ms.len() - 1);
        self.delays_ms[idx]
    }
}
