use std::io::Cursor;

use binrw::{
    binrw,
    meta::{ReadEndian, WriteEndian},
    BinRead, BinResult, BinWrite, NamedArgs,
};
use common::read_ext::ReadExt;
use dashmap::DashMap;
use yaobow_macros::NiObjectType;

use crate::nif::NiType;

use super::{Matrix33, NiObject, SizedString, Vector3};

#[derive(Debug)]
pub struct NiBlocks(pub Vec<Box<dyn NiObject>>);

impl ReadEndian for NiBlocks {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::None;
}

impl WriteEndian for NiBlocks {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::None;
}

impl BinRead for NiBlocks {
    type Args<'a> = NiBlockArgs<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let mut blocks = vec![];

        for i in 0..args.block_sizes.len() {
            let size = args.block_sizes[i];
            let name: String = args.block_types[args.block_type_index[i] as usize]
                .clone()
                .into();
            let ty = NI_OBJECT_TYPES
                .get(name.as_str())
                .map_or(&NiTypeUnknownBlock, |v| v.value());

            let data = reader.read_u8_vec(size as usize)?;
            let block = (ty.read)(&mut Cursor::new(data), size)?;
            blocks.push(block);
        }

        Ok(Self(blocks))
    }
}

impl BinWrite for NiBlocks {
    type Args<'a> = NiBlockArgs<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        _: binrw::Endian,
        _: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        for block in &self.0 {
            let mut w = Cursor::new(vec![]);
            (block.ni_type().write)(block.as_ref(), &mut w)?;

            writer.write(w.get_ref())?;
        }

        Ok(())
    }
}

lazy_static::lazy_static! {
    static ref NI_OBJECT_TYPES: DashMap<&'static str, &'static NiType> = init_ni_object_types();
}

fn init_ni_object_types() -> DashMap<&'static str, &'static NiType> {
    let map = DashMap::new();
    let add = |ty: &'static NiType| {
        map.insert(ty.name, ty);
    };

    add(&NiTypeUnknownBlock);
    add(&NiTypeNiNode);
    add(&NiTypeNiMesh);

    map
}

#[derive(Clone, Default, NamedArgs)]
pub struct NiBlockArgs<'a> {
    pub block_sizes: &'a [u32],
    pub block_types: &'a [SizedString],
    pub block_type_index: &'a [u16],
}

#[derive(Debug, NiObjectType)]
pub struct UnknownBlock {
    data: Vec<u8>,
}

impl ReadEndian for UnknownBlock {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::None;
}

impl WriteEndian for UnknownBlock {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::None;
}

impl BinRead for UnknownBlock {
    type Args<'a> = u32;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        Ok(Self {
            data: reader.read_u8_vec(args as usize)?,
        })
    }
}

impl BinWrite for UnknownBlock {
    type Args<'a> = NiBlockArgs<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        _: binrw::Endian,
        _: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        let _ = writer.write(&self.data)?;
        Ok(())
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NiObjectNET {
    name: u32,
    num_extra_data: u32,

    #[br(count = num_extra_data)]
    extra_data: Vec<u32>,

    controller: u32,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NiAVObject {
    object_net: NiObjectNET,
    flags: u16,
    translation: Vector3,
    rotation: Matrix33,
    scale: f32,
    num_properties: u32,

    #[br(count = num_properties)]
    properties: Vec<u32>,

    collision_object: i32,
}

#[binrw]
#[brw(little)]
#[br(import_raw(_size: u32))]
#[derive(Debug, NiObjectType)]
pub struct NiNode {
    av_object: NiAVObject,
    num_children: u32,

    #[br(count = num_children)]
    children: Vec<u32>,

    num_effects: u32,

    #[br(count = num_effects)]
    effects: Vec<u32>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct MaterialData {
    num_materials: u32,

    #[br(count = num_materials)]
    material_names: Vec<u32>,

    #[br(count = num_materials)]
    material_extra_data: Vec<i32>,

    active_material: i32,
    always_update: u8,
}

#[binrw]
#[brw(little)]
#[br(import_raw(_size: u32))]
#[derive(Debug, NiObjectType)]
pub struct NiRenderObject {
    av_object: NiAVObject,
    material_data: MaterialData,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct MeshPrimitiveType(pub u32);
impl MeshPrimitiveType {
    pub const MESH_PRIMITIVE_TRIANGLES: Self = Self(0);
    pub const MESH_PRIMITIVE_TRISTRIPS: Self = Self(1);
    pub const MESH_PRIMITIVE_LINE: Self = Self(2);
    pub const MESH_PRIMITIVE_LINESTRIPS: Self = Self(3);
    pub const MESH_PRIMITIVE_QUAD: Self = Self(4);
    pub const MESH_PRIMITIVE_POINT: Self = Self(5);
}

#[binrw]
#[brw(little)]
#[br(import_raw(_size: u32))]
#[derive(Debug, NiObjectType)]
pub struct NiBound {
    center: Vector3,
    radius: f32,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct SemanticData {
    name: i32,
    index: u32,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct DataStreamRef {
    stream: i32,
    per_instance: u8,
    num_sub_meshes: u16,

    #[br(count = num_sub_meshes)]
    sub_mesh_to_region_map: Vec<u16>,

    num_components: u32,

    #[br(count = num_components)]
    semantic_data: Vec<SemanticData>,
}

#[binrw]
#[brw(little)]
#[br(import_raw(_size: u32))]
#[derive(Debug, NiObjectType)]
pub struct NiMesh {
    render_object: NiRenderObject,
    primitive_type: MeshPrimitiveType,
    num_sub_meshes: u16,
    instancing: u8,
    bound: NiBound,
    num_data_streams: u32,

    #[br(count = num_data_streams)]
    data_stream_ref: Vec<DataStreamRef>,

    num_modifiers: u32,

    #[br(count = num_modifiers)]
    modifiers: Vec<i32>,
}
