use binrw::binrw;

use super::{HeaderString, SizedString};

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NiHeader {
    pub header_string: HeaderString,
    pub version: u32,
    pub endianess: u8,
    pub user_version: u32,
    pub num_block: u32,
    pub num_block_types: u16,

    #[br(count = num_block_types)]
    pub block_types: Vec<SizedString>,

    #[br(count = num_block)]
    pub block_type_index: Vec<u16>,

    #[br(count = num_block)]
    pub block_size: Vec<u32>,

    pub num_strings: u32,
    pub max_str_length: u32,

    #[br(count = num_strings)]
    pub strings: Vec<SizedString>,
    pub num_groups: u32,

    #[br(count = num_groups)]
    pub groups: Vec<u32>,
}
