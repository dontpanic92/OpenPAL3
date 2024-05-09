use std::io::SeekFrom;

use binrw::{BinRead, BinResult};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::utils::{parse_sized_string, SizedString};

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobFile {
    pub header: GobHeader,

    #[br(count = header.count)]
    pub entries: Vec<GobEntry>,
}

pub struct GobObjectType;
impl GobObjectType {
    pub const ITEM: u32 = 0;
    pub const EFFECT: u32 = 0x8;
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
    pub properties: Vec<GobProperty>,

    pub prameters_magic: u32,
    pub prameters_begin: GobPropertyI32,

    #[br(parse_with = parse_properties)]
    pub parameters: Vec<GobProperty>,
}

pub enum GobCommonProperties {
    Scale,
    ResearchNum,
    AutoDisappear,
}

pub enum GobCommonParameters {
    EffectName,
    EffectTimes,
}

impl GobEntry {
    pub fn get_property(&self, name: &str) -> Option<&GobProperty> {
        self.properties
            .iter()
            .find_map(|p| if p.name() == name { Some(p) } else { None })
    }

    pub fn get_parameter(&self, name: &str) -> Option<&GobProperty> {
        self.parameters
            .iter()
            .find_map(|p| if name == p.name() { Some(p) } else { None })
    }

    pub fn get_common_property(&self, property: GobCommonProperties) -> Option<&GobProperty> {
        match property {
            GobCommonProperties::Scale => self.get_property("PAL4-GameObject-object-scale"),
            GobCommonProperties::ResearchNum => {
                self.get_property("PAL4-GameObject-object-research-num")
            }
            GobCommonProperties::AutoDisappear => {
                self.get_property("PAL4-GameObject-object-auto-disappear")
            }
        }
    }

    pub fn get_common_parameter(&self, parameter: GobCommonParameters) -> Option<&GobProperty> {
        match parameter {
            GobCommonParameters::EffectName => self.get_parameter("PAL4_GameObject-effect-name"),
            GobCommonParameters::EffectTimes => self.get_parameter("PAL4_GameObject-effect-times"),
        }
    }
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

impl GobProperty {
    pub fn name(&self) -> &str {
        match self {
            Self::GobPropertyI32(v) => &v.name,
            Self::GobPropertyF32(v) => &v.name,
            Self::GobPropertyString(v) => &v.name,
            Self::GobPropertyObjectArray(v) => &v.0[0].name,
        }
    }

    pub fn value_i32(&self) -> Option<i32> {
        if let Self::GobPropertyI32(v) = self {
            Some(v.value)
        } else {
            None
        }
    }

    pub fn value_f32(&self) -> Option<f32> {
        if let Self::GobPropertyF32(v) = self {
            Some(v.value)
        } else {
            None
        }
    }

    pub fn value_string(&self) -> Option<&str> {
        if let Self::GobPropertyString(v) = self {
            Some(&v.value)
        } else {
            None
        }
    }
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
        println!(
            "GobPropertyObjectArray::read_options cursor position: {}",
            reader.stream_position()?
        );
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
    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub value: i32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyF32 {
    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub value: f32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyString {
    #[br(parse_with = parse_sized_string)]
    pub name: String,

    #[br(parse_with = parse_sized_string)]
    pub value: String,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobObject {
    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub prop_count: u32,

    #[br(count = prop_count)]
    pub properties: Vec<GobProperty>,
}
