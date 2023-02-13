use std::io::Cursor;

use crate::rwbs::{ChunkHeader, ChunkType};

use self::world::World;

pub mod sector;
pub mod world;

pub fn read_bsp(data: &[u8]) -> anyhow::Result<Vec<World>> {
    let mut cursor = Cursor::new(data);
    let mut world = vec![];

    while !cursor.is_empty() {
        let chunk = ChunkHeader::read(&mut cursor)?;
        match chunk.ty {
            ChunkType::WORLD => world.push(World::read(&mut cursor)?),
            _ => cursor.set_position(cursor.position() + chunk.length as u64),
        }
    }

    Ok(world)
}
