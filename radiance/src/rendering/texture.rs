use image::RgbaImage;
use std::path::PathBuf;

pub trait Texture: downcast_rs::Downcast {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

downcast_rs::impl_downcast!(Texture);

pub enum TextureDef {
    PathTextureDef(PathBuf),
    ImageTextureDef(RgbaImage),
}
