use binrw::{binrw, NullString};

#[binrw]
#[brw(little, magic = 0x87654321u32)]
#[derive(Debug)]
pub struct RoleBinFile {
    pub version: u32,
    pub item_count: u32,

    // version < 105 is not supported
    #[br(if(version >= 105), count = item_count)]
    pub items: Vec<AssetItem>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct AssetItem {
    pub id: u32,
    pub unknown: [u32; 17],
    pub unknown_f32: [f32; 3],

    pub file_path: NullString,
    pub folder_folder: NullString,
    pub empty_string: NullString,
    pub empty_string2: NullString,
}
