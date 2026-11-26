use std::cell::RefCell;

#[derive(Clone, Copy, Debug)]
pub struct AdditiveImage {
    pub texture: imgui::TextureId,
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

thread_local! {
    static IMAGES: RefCell<Vec<AdditiveImage>> = const { RefCell::new(Vec::new()) };
}

pub fn add_image(image: AdditiveImage) {
    IMAGES.with(|images| images.borrow_mut().push(image));
}

pub fn take_images() -> Vec<AdditiveImage> {
    IMAGES.with(|images| std::mem::take(&mut *images.borrow_mut()))
}
