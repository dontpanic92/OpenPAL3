#[cfg_attr(feature = "std", path = "image.rs")]
#[cfg(feature = "std")]
mod internal;

#[cfg_attr(feature = "no_std", path = "rimg.rs")]
#[cfg(feature = "no_std")]
mod internal;

#[cfg(feature = "no_std")]
use alloc::{string::ToString, vec::Vec};

pub enum ImageFormat {
    Bmp,
    Png,
    Tga,
    Dds,
}

pub struct RgbaImage {
    buffer: Vec<u8>,
    width: u32,
    height: u32,
}

impl RgbaImage {
    pub fn new(buffer: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            buffer,
            width,
            height,
        }
    }

    pub fn as_raw(&self) -> &[u8] {
        &self.buffer
    }

    pub fn load_from_memory(buffer: &[u8], file_name: &str) -> Option<Self> {
        internal::load_from_memory(buffer, file_name)
    }

    pub fn load_from_memory_with_format(buffer: &[u8], format: ImageFormat) -> Option<Self> {
        internal::load_from_memory_with_format(buffer, format)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
