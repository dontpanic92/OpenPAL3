use core::ffi::c_void;
use core::fmt::{Result, Write};
use core::mem::size_of;
use core::ptr;
use std::convert::TryInto;

use psp2::display::sceDisplaySetFrameBuf;
use psp2::kernel::sysmem::{
    sceKernelAllocMemBlock, sceKernelFreeMemBlock, sceKernelGetMemBlockBase,
};
use psp2common::display::{SceDisplayFrameBuf, SceDisplaySetBufSync::SCE_DISPLAY_SETBUF_NEXTFRAME};
use psp2common::kernel::sysmem::SCE_KERNEL_MEMBLOCK_TYPE_USER_CDRAM_RW;
use psp2common::types::SceUID;
use vitasdk_sys::{psp2, psp2common};

use super::font::DEBUG_FONT;

const SCREEN_WIDTH: usize = 960;
const SCREEN_HEIGHT: usize = 544;
const SCREEN_PIXEL_COUNT: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const SCREEN_FB_WIDTH: usize = 960;
const SCREEN_FB_SIZE: usize = 2 * 1024 * 1024;
const SCREEN_TAB_SIZE: usize = 4; // Tab size in number of characters
const SCREEN_TAB_W: usize = DEBUG_FONT.size_w * SCREEN_TAB_SIZE;

const DEFAULT_FG: u32 = 0xFFFFFFFF;
const DEFAULT_BG: u32 = 0xFF000000;

pub struct DebugScreen {
    // TODO: rename to pixel array or something like that
    framebuffer: Framebuffer,
    coord_x: usize,
    coord_y: usize,
    color_fg: u32,
    color_bg: u32,
}

pub struct Framebuffer {
    buf: *mut u32,
    block_uid: SceUID,
}

impl Framebuffer {
    pub fn new() -> Framebuffer {
        // Allocate memory to use as display buffer
        let mut base: *mut c_void = ::core::ptr::null_mut();
        let block_uid = unsafe {
            let block_uid: SceUID = sceKernelAllocMemBlock(
                b"display\0".as_ptr() as *const i8,
                SCE_KERNEL_MEMBLOCK_TYPE_USER_CDRAM_RW,
                SCREEN_FB_SIZE as u32,
                ::core::ptr::null_mut(),
            );
            sceKernelGetMemBlockBase(block_uid, &mut base);
            block_uid
        };
        Framebuffer {
            buf: base as *mut u32,
            block_uid,
        }
    }

    pub fn set_display(&mut self) {
        // Sets buffer as current display frame
        let frame = SceDisplayFrameBuf {
            size: size_of::<SceDisplayFrameBuf>() as u32,
            base: self.buf as *mut c_void,
            pitch: SCREEN_FB_WIDTH as u32,
            pixelformat: 0,
            width: SCREEN_WIDTH as u32,
            height: SCREEN_HEIGHT as u32,
        };
        unsafe {
            sceDisplaySetFrameBuf(&frame, SCE_DISPLAY_SETBUF_NEXTFRAME);
        }
    }

    #[allow(unused)]
    pub fn get(&self, index: usize) -> u32 {
        if index > SCREEN_PIXEL_COUNT {
            panic!("Invalid framebuffer index");
        }
        unsafe { ptr::read_volatile(self.buf.offset(index.try_into().unwrap())) }
    }

    pub fn set(&mut self, index: usize, value: u32) {
        if index > SCREEN_PIXEL_COUNT {
            panic!("Invalid framebuffer index");
        }
        unsafe { ptr::write_volatile(self.buf.offset(index.try_into().unwrap()), value) }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        let _error_code = unsafe { sceKernelFreeMemBlock(self.block_uid) };
    }
}

impl Write for DebugScreen {
    fn write_str(&mut self, s: &str) -> Result {
        self.puts(s.as_bytes());
        Ok(())
    }
}

impl DebugScreen {
    pub fn new() -> Self {
        let mut framebuffer = Framebuffer::new();
        framebuffer.set_display();
        Self {
            framebuffer,
            coord_x: 0,
            coord_y: 0,
            color_fg: DEFAULT_FG,
            color_bg: DEFAULT_BG,
        }
    }

    #[allow(unused)]
    fn clear(&mut self, from_h: usize, to_h: usize, from_w: usize, to_w: usize) {
        for h in from_h..to_h {
            for w in from_w..to_w {
                self.framebuffer.set(h * SCREEN_FB_WIDTH + w, self.color_bg);
            }
        }
    }

    fn puts(&mut self, text: &[u8]) {
        let bytes_per_glyph = DEBUG_FONT.width * DEBUG_FONT.height / 8;

        for &chr in text.iter() {
            if chr == b'\t' {
                self.coord_x += SCREEN_TAB_W - (self.coord_x % SCREEN_TAB_W);
                continue;
            }

            // Go to next line at the end of the current line
            if self.coord_x + DEBUG_FONT.width > SCREEN_WIDTH {
                self.coord_y += DEBUG_FONT.size_h;
                self.coord_x = 0;
            }

            // Go to screen top when at the bottom of the screen
            if self.coord_y + DEBUG_FONT.height > SCREEN_HEIGHT {
                self.coord_x = 0;
                self.coord_y = 0;
            }

            if chr == b'\n' {
                self.coord_x = 0;
                self.coord_y += DEBUG_FONT.size_h;
                continue;
            } else if chr == b'\r' {
                self.coord_x = 0;
                continue;
            }

            let current_offset = self.coord_x + self.coord_y * SCREEN_FB_WIDTH;
            let mut font =
                &DEBUG_FONT.glyphs[(chr - DEBUG_FONT.first) as usize * bytes_per_glyph..];
            let mut mask = 1 << 7;

            for row in 0..DEBUG_FONT.height {
                for col in 0..DEBUG_FONT.width {
                    if mask == 0 {
                        font = &font[1..];
                        mask = 1 << 7;
                    }

                    self.framebuffer.set(
                        current_offset + row * SCREEN_FB_WIDTH + col,
                        if font[0] & mask == 0 {
                            self.color_bg
                        } else {
                            self.color_fg
                        },
                    );

                    mask >>= 1;
                }

                for col in DEBUG_FONT.width..DEBUG_FONT.size_w {
                    self.framebuffer
                        .set(current_offset + row * SCREEN_FB_WIDTH + col, self.color_bg)
                }
            }

            for row in DEBUG_FONT.height..DEBUG_FONT.size_h {
                for col in 0..DEBUG_FONT.size_w {
                    self.framebuffer
                        .set(current_offset + row * SCREEN_FB_WIDTH + col, self.color_bg)
                }
            }

            self.coord_x += DEBUG_FONT.size_w;
        }
    }
}
