//! Throwaway: decode a .dds to rgb.png + alpha.png for visual inspection.
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let data = std::fs::read(&path).unwrap();
    let img = image::load_from_memory(&data).unwrap().to_rgba8();
    let (w, h) = (img.width(), img.height());
    let mut rgb = image::RgbImage::new(w, h);
    let mut alpha = image::GrayImage::new(w, h);
    let mut a_min = 255u8;
    let mut a_max = 0u8;
    for (x, y, p) in img.enumerate_pixels() {
        rgb.put_pixel(x, y, image::Rgb([p.0[0], p.0[1], p.0[2]]));
        alpha.put_pixel(x, y, image::Luma([p.0[3]]));
        a_min = a_min.min(p.0[3]);
        a_max = a_max.max(p.0[3]);
    }
    let base = std::env::args().nth(2).unwrap();
    rgb.save(format!("{base}_rgb.png")).unwrap();
    alpha.save(format!("{base}_alpha.png")).unwrap();
    println!("{w}x{h} alpha range {a_min}..{a_max} -> {base}_rgb.png {base}_alpha.png");
}
