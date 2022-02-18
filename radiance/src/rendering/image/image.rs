use super::{ImageFormat, RgbaImage};

pub fn load_from_memory_with_format(buf: &[u8], format: ImageFormat) -> Option<RgbaImage> {
    let f = match format {
        ImageFormat::Bmp => image::ImageFormat::Bmp,
        ImageFormat::Tga => image::ImageFormat::Tga,
        ImageFormat::Dds => image::ImageFormat::Dds,
        ImageFormat::Png => image::ImageFormat::Png,
    };

    let img = image::load_from_memory_with_format(&buf, f).or_else(|err| {
        log::error!("Error while loading texture: {}", err);
        Err(err)
    }).ok()?;
    let rgba_img = img.to_rgba8();
    Some(RgbaImage::new(rgba_img.to_vec(), rgba_img.width(), rgba_img.height()))
}

pub fn load_from_memory(buffer: &[u8], file_name: &str) -> Option<RgbaImage> {
    let img= image::load_from_memory(buffer).ok()?;
    let rgba_img = img.to_rgba8();
    Some(RgbaImage::new(rgba_img.to_vec(), rgba_img.width(), rgba_img.height()))
}
