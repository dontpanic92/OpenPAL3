use binrw::BinRead;

use crate::{
    rwbs::{ChunkHeader, Vec3f},
    utils::SizedString,
};

#[derive(Debug, BinRead)]
#[brw(little)]
pub struct EvfFile {
    pub count: u32,

    #[br(count = count)]
    pub events: Vec<EvfEvent>,
}

#[derive(Debug, BinRead)]
#[brw(little)]
pub struct EvfEvent {
    pub name: SizedString,
    pub unknown: u32,
    pub unknown2: u32,
    pub unknown3: SizedString,
    pub function: EvfFunctionInfo,

    pub trigger_count: u32,

    #[br(count = trigger_count)]
    pub triggers: Vec<EvfTrigger>,

    pub unknown4: u32,
    pub unknown5: u32,
    pub chunk: EvfClump,
}

#[derive(Debug, BinRead)]
#[brw(little)]
pub struct EvfFunctionInfo {
    pub unknown: u32,
    pub unknown2: u32,
    pub scene: SizedString,
    pub block: SizedString,
    pub function: SizedString,
}

#[derive(Debug, BinRead)]
#[brw(little)]
pub struct EvfTrigger {
    pub center: Vec3f,
    pub half_size: Vec3f,
    pub max: Vec3f,
    pub min: Vec3f,
    pub unknown: Vec3f,
    pub unknown2: Vec3f,
    pub unknown3: u32,
    pub unknown4: u32,
}

#[derive(Debug)]
pub struct EvfClump {
    pub header: ChunkHeader,
}

impl BinRead for EvfClump {
    type Args<'a> = ();

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        let header = ChunkHeader::read_options(reader, endian, args)?;

        // Looks like the clump is truncated and short by 0x20 bytes.
        reader.seek(std::io::SeekFrom::Current(
            header.length as i64 - 0xC - 0x20,
        ))?;

        Ok(Self { header })
    }
}
