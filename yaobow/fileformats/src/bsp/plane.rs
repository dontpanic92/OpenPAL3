use std::io::Read;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Plane {
    pub extension: Vec<u8>,
}

impl Plane {
    pub fn read(cursor: &mut Read) -> anyhow::Result<Self> {}
}
