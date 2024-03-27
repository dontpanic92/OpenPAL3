use binrw::{BinRead, BinWrite};

use super::Sized32Big5String;

#[derive(Debug, BinRead, BinWrite)]
#[brw(little)]
pub struct Fld {
    pub name: Sized32Big5String,
    pub map_file: Sized32Big5String,
    // TODO: parse all the fields
}
