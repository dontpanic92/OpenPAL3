use binrw::binrw;

use crate::utils::{Pal4NodeSection, SizedString};

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NpcInfoFile {
    count: u32,

    #[br(count = count)]
    data: Vec<NpcInfo>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NpcInfo {
    name: SizedString,
    model_name: SizedString,
    unknown_name: SizedString,
    position: [f32; 3],
    rotation: [f32; 3],
    unknown: i32,
    unknown2: i32,
    unknown3: f32,

    behaviour: Pal4NodeSection,
    buffer_cache: Pal4NodeSection,
}
