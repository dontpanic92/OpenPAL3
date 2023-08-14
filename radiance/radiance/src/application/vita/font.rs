pub struct DebugFont {
    pub glyphs: &'static [u8],
    pub width: usize,
    pub height: usize,
    pub first: u8,
    pub last: u8,
    pub size_w: usize,
    pub size_h: usize,
}

pub const DEBUG_FONT: DebugFont = DebugFont {
    glyphs: include_bytes!("font.bin"),
    width: 8,
    height: 8,
    first: 0,
    last: 255,
    size_w: 8,
    size_h: 8,
};
