use std::io::SeekFrom;

use binrw::{BinRead, BinResult};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::utils::SizedString;

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobFile {
    pub header: GobHeader,

    // #[br(count = header.count)]
    #[br(count = 30)]
    pub entries: Vec<GobEntry>,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobHeader {
    pub count: u32,

    #[br(count = count)]
    pub object_types: Vec<u32>,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobEntry {
    pub name: SizedString,
    pub folder: SizedString,
    pub file_name: SizedString,
    pub file_name2: SizedString,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub research_function: SizedString,
    pub unknown6: [u32; 3],
    pub unknown7: f32,
    pub unknown8: u32,

    pub game_object_magic: u32,
    pub game_object: GobPropertyI32,

    #[br(count = game_object.value)]
    pub game_object_properties: Vec<GobProperty>,

    pub prameters_magic: u32,
    pub prameters: GobPropertyI32,

    #[br(parse_with = parse_properties)]
    pub properties: Vec<GobProperty>,
}

#[binrw::parser(reader, endian)]
fn parse_properties() -> BinResult<Vec<GobProperty>> {
    let mut properties = vec![];

    loop {
        let ty = reader.read_u32_le()?;
        if ty == 0 {
            break;
        } else {
            reader.seek(SeekFrom::Current(-4))?;
        }

        let property = GobProperty::read_options(reader, endian, ())?;
        properties.push(property);
    }

    Ok(properties)
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub enum GobProperty {
    #[br(magic(0x1u32))]
    GobPropertyI32(GobPropertyI32),

    #[br(magic(0x2u32))]
    GobPropertyF32(GobPropertyF32),

    #[br(magic(0x3u32))]
    GobPropertyString(GobPropertyString),

    GobPropertyObjectArray(GobPropertyObjectArray),
}

#[derive(Debug, Serialize)]
pub struct GobPropertyObjectArray(pub Vec<GobObject>);

impl BinRead for GobPropertyObjectArray {
    type Args<'a> = ();

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut properties = vec![];

        let count = reader.read_u32_le()?;
        reader.seek(SeekFrom::Current(-4))?;
        for _ in 0..count {
            let _ = reader.read_u32_le()?;

            let obj = GobObject::read_options(reader, endian, ())?;
            properties.push(obj);
        }

        Ok(Self(properties))
    }
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyI32 {
    pub name: SizedString,
    pub value: i32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyF32 {
    pub name: SizedString,
    pub value: f32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyString {
    pub name: SizedString,
    pub value: SizedString,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobObject {
    pub name: SizedString,
    pub prop_count: u32,

    #[br(count = prop_count)]
    pub properties: Vec<GobProperty>,
}
