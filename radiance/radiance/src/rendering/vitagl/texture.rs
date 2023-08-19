use crate::rendering::Texture;

pub struct VitaGLTexture {
    texture_id: u32,
    width: u32,
    height: u32,
}

impl Texture for VitaGLTexture {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

impl VitaGLTexture {
    pub fn new(width: u32, height: u32, pixels: &[u8]) -> Self {
        let mut texture_id = 0;

        unsafe {
            use vitagl_sys::*;
            glGenTextures(1, &mut texture_id);
            glBindTexture(GL_TEXTURE_2D, texture_id);

            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                GL_RGBA as i32,
                width as i32,
                height as i32,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                pixels.as_ptr() as *const _,
            );
        }

        Self {
            texture_id,
            width,
            height,
        }
    }

    pub fn texture_id(&self) -> u32 {
        self.texture_id
    }
}

impl Drop for VitaGLTexture {
    fn drop(&mut self) {
        unsafe { vitagl_sys::glDeleteTextures(1, &self.texture_id) };
    }
}
