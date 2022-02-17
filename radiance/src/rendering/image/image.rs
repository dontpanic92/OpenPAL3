use super::ImageFormat;

pub fn load_from_memory_with_format(buffer: Vec<u8>, format: ImageFormat) {
    let f = match format {
        ImageFormat::Bmp => image::ImageFormat::Bmp,
        ImageFormat::Tga => image::ImageFormat::Tga,
        ImageFormat::Dds => image::ImageFormat::Dds,
    };


}
