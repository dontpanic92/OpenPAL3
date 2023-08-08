use std::borrow::Cow;

use binrw::binrw;

use crate::utils::{Pal4NodeSection, SizedString};

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NpcInfoFile {
    count: u32,

    #[br(count = count)]
    pub data: Vec<NpcInfo>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NpcInfo {
    pub name: SizedString,
    pub model_name: SizedString,
    pub unknown_name: SizedString,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    unknown: i32,
    unknown2: i32,
    unknown3: f32,

    behaviour: Pal4NodeSection,
    buffer_cache: Pal4NodeSection,
}

impl NpcInfo {
    pub fn get_default_act(&self) -> Option<Cow<str>> {
        Some(
            self.buffer_cache
                .root
                .as_ref()?
                .get_child_by_name("NPCINFO_BufferCache_Attr")?
                .get_property_by_name("NPCINFO_BufferCache_Attr_defaultAct")?
                .string()?,
        )
    }
}
