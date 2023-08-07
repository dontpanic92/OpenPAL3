pub mod blocks;
pub mod footer;
pub mod header;

use std::{
    fmt::Debug,
    io::{Cursor, Read, Seek, Write},
};

use binrw::{binrw, binwrite, meta::ReadEndian, BinRead, BinResult, BinWrite, Endian};

use self::{
    blocks::{NiBlocks, NiBlocksArgs, NiObjectArgs},
    footer::NiFooter,
    header::NiHeader,
};

/**
 * Reference:
 *      https://github.com/niftools/nifskope
 */

#[binwrite]
#[bw(little)]
#[derive(Debug)]
pub struct NifModel {
    header: NiHeader,
    blocks: NiBlocks,
    footer: NiFooter,
}

impl ReadEndian for NifModel {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::None;
}

impl BinRead for NifModel {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _: Endian,
        _: Self::Args<'_>,
    ) -> BinResult<Self> {
        let header = NiHeader::read(reader)?;
        let blocks = NiBlocks::read_args(
            reader,
            NiBlocksArgs {
                block_sizes: &header.block_size,
                block_types: &header.block_types,
                block_type_index: &header.block_type_index,
            },
        )?;
        let footer = NiFooter::read(reader)?;

        Ok(Self {
            header,
            blocks,
            footer,
        })
    }
}

#[derive(Clone)]
pub struct NiType {
    pub name: &'static str,
    pub read: fn(&mut Cursor<Vec<u8>>, NiObjectArgs) -> BinResult<Box<dyn NiObject>>,
    pub write: fn(&dyn NiObject, &mut Cursor<Vec<u8>>) -> BinResult<()>,
}

pub trait NiObject: std::any::Any + Debug {
    fn ni_type(&self) -> &'static NiType;
    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct HeaderString {
    string: Vec<u8>,
}

impl BinRead for HeaderString {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        _: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut values = vec![];

        loop {
            let val = <u8>::read_options(reader, endian, ())?;
            if val == 0x0a {
                return Ok(Self { string: values });
            }
            values.push(val);
        }
    }
}

impl BinWrite for HeaderString {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        self.string.write_options(writer, endian, args)?;
        0x0au8.write_options(writer, endian, args)?;

        Ok(())
    }
}

impl std::fmt::Debug for HeaderString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HeaderString(\"{}\")",
            String::from_utf8_lossy(&self.string)
        )
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct Vector3 {
    x: f32,
    y: f32,
    z: f32,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct Matrix33 {
    m11: f32,
    m21: f32,
    m31: f32,

    m12: f32,
    m22: f32,
    m32: f32,

    m13: f32,
    m23: f32,
    m33: f32,
}
