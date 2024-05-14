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

    pub game_object: GobPropertyI32,

    #[br(count = game_object.value)]
    pub properties: Vec<GobProperty>,

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

        println!(
            "   parse_properties cursor position: {}",
            reader.stream_position()?
        );

        let property = GobProperty::read_options(reader, endian, ())?;
        println!(
            "   parse_properties completed cursor position: {}",
            reader.stream_position()?
        );
        properties.push(property);
    }

    Ok(properties)
}

#[derive(Debug, Serialize)]
pub enum GobProperty {
    GobPropertyI32(GobPropertyI32),
    GobPropertyF32(GobPropertyF32),
    GobPropertyString(GobPropertyString),
    GobPropertyObjectArray(GobPropertyObjectArray),
}

impl BinRead for GobProperty {
    type Args<'a> = ();

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> BinResult<Self> {
        let start_position = reader.stream_position()?;
        println!(
            "GobProperty::read_options cursor position: {}",
            start_position
        );
        let ty = reader.read_u32_le()?;
        let name = SizedString::read_options(reader, endian, ())?;
        reader.seek(SeekFrom::Start(start_position))?;

        if name == "PAL4_GameObject-machine-condition" || name == "PAL4-GOMTask-[ 0 ]" {
            return Ok(Self::GobPropertyObjectArray(
                GobPropertyObjectArray::read_options(reader, endian, ())?,
            ));
        } else {
            println!(
                "GobProperty::read_options reading ty {} at cursor position: {}",
                ty, start_position
            );
            match ty {
                1 => Ok(Self::GobPropertyI32(GobPropertyI32::read_options(
                    reader,
                    endian,
                    (),
                )?)),
                2 => Ok(Self::GobPropertyF32(GobPropertyF32::read_options(
                    reader,
                    endian,
                    (),
                )?)),
                3 => Ok(Self::GobPropertyString(GobPropertyString::read_options(
                    reader,
                    endian,
                    (),
                )?)),
                _ => {
                    unreachable!(
                        "Unknown array name: {:?} at position {}",
                        name.to_string(),
                        start_position
                    );
                }
            }
        }
    }
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
        for i in 0..count {
            println!(
                "   GobPropertyObjectArray::read_options {} cursor position: {}",
                i,
                reader.stream_position()?
            );
            let _ = reader.read_u32_le()?;

            let obj = GobObject::read_options(reader, endian, ())?;
            properties.push(obj);
            println!(
                "   GobPropertyObjectArray::read_options {} completed cursor position: {}",
                i,
                reader.stream_position()?
            );
        }

        Ok(Self(properties))
    }
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyI32 {
    pub ty: u32,

    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub value: i32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyF32 {
    pub ty: u32,

    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub value: f32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyString {
    pub ty: u32,

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

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::BufReader;

    use super::*;

    #[test]
    fn test_gob() {
        let file =
            File::open("F:\\PAL4\\gamedata\\scenedata\\scenedata\\M01\\1\\GameObjs.gob").unwrap();
        let mut reader = BufReader::new(file);
        let gob_file = GobFile::read(&mut reader).unwrap();

        println!("{:#?}", gob_file);
    }
}
