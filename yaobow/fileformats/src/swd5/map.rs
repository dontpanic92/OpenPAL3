use binrw::{binrw, NullString};

use super::Sized32Big5String;

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Map {
    pub header: MapFileHeader,
    pub texture_chunk: MapTextureChunk,
    pub terrain_chunk: MapTerrainChunk,
    pub model_chunk: MapModelChunk,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct MapFileHeader {
    pub terraform: NullString,
    pub unknown: u32,
    pub terrain: NullString,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct ChunkHeader {
    pub unknown1: u32,
    pub unknown2: u32,
    pub unknown3: u32,
    pub unknown4: u32,
    pub unknown5: u32,
    pub unknown6: f32,
    pub chunk_size: u32,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct MapTextureChunk {
    pub header: ChunkHeader,

    #[br(count = header.chunk_size)]
    pub data: Vec<u8>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct MapTerrainChunk {
    pub header: ChunkHeader,

    #[br(count = header.chunk_size)]
    pub data: Vec<u8>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct MapModelChunk {
    pub unknown1: u32,
    pub unknown2: [f32; 8],
    pub unknown3: u8,
    pub unknown4: [f32; 6],
    pub unknown5: u32,
    pub model_file: Sized32Big5String,
}
