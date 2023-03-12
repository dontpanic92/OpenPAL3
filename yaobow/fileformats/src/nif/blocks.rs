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
    add(&NiTypeNiDataStream);

    map
}

#[derive(Debug)]
pub struct NiBlocks(pub Vec<Box<dyn NiObject>>);

impl ReadEndian for NiBlocks {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::None;
}

impl WriteEndian for NiBlocks {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::None;
}

impl BinRead for NiBlocks {
    type Args<'a> = NiBlocksArgs<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let mut blocks = vec![];

        for i in 0..args.block_sizes.len() {
            let size = args.block_sizes[i];
            let full_name: String = args.block_types[args.block_type_index[i] as usize]
                .clone()
                .into();

            let mut full_name = full_name.split('\x01');

            let name = full_name.next().unwrap();
            let generic_value1 = full_name.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let generic_value2 = full_name.next().and_then(|s| s.parse().ok()).unwrap_or(0);

            let ty = NI_OBJECT_TYPES
                .get(name)
                .map_or(&NiTypeUnknownBlock, |v| v.value());

            let data = reader.read_u8_vec(size as usize)?;
            let block = (ty.read)(
                &mut Cursor::new(data),
                NiObjectArgs {
                    block_size: size,
                    generic_value1,
                    generic_value2,
                },
            )?;
            blocks.push(block);
        }

        Ok(Self(blocks))
    }
}

impl BinWrite for NiBlocks {
    type Args<'a> = ();

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

#[derive(Clone, Default, NamedArgs)]
pub struct NiBlocksArgs<'a> {
    pub block_sizes: &'a [u32],
    pub block_types: &'a [SizedString],
    pub block_type_index: &'a [u16],
}

#[derive(Clone, Default, NamedArgs)]
pub struct NiObjectArgs {
    pub block_size: u32,
    pub generic_value1: u32,
    pub generic_value2: u32,
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
    type Args<'a> = NiObjectArgs;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        Ok(Self {
            data: reader.read_u8_vec(args.block_size as usize)?,
        })
    }
}

impl BinWrite for UnknownBlock {
    type Args<'a> = NiObjectArgs;

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
#[br(import_raw(_args: NiObjectArgs))]
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
#[br(import_raw(_args: NiObjectArgs))]
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
#[br(import_raw(_args: NiObjectArgs))]
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
#[br(import_raw(_args: NiObjectArgs))]
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

#[binrw]
#[brw(little)]
#[derive(Debug, Default)]
pub struct DataStreamUsage(pub u32);
impl DataStreamUsage {
    pub const USAGE_VERTEX_INDEX: Self = Self(0);
    pub const USAGE_VERTEX: Self = Self(1);
    pub const USAGE_SHADER_CONSTANT: Self = Self(2);
    pub const USAGE_USER: Self = Self(3);
}

#[binrw]
#[brw(little)]
#[derive(Debug, Default)]
pub struct DataStreamAccess(pub u32);
impl DataStreamAccess {
    pub const CPU_READ: Self = Self(1 << 0);
    pub const CPU_WRITE_STATIC: Self = Self(1 << 1);
    pub const CPU_WRITE_MUTABLE: Self = Self(1 << 2);
    pub const CPU_WRITE_VOLATILE: Self = Self(1 << 3);
    pub const GPU_READ: Self = Self(1 << 4);
    pub const GPU_WRITE: Self = Self(1 << 5);
    pub const CPU_WRITE_STATIC_INITIALIZED: Self = Self(1 << 6);
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct CloningBehavior(pub u32);
impl CloningBehavior {
    pub const CLONING_SHARE: Self = Self(0);
    pub const CLONING_COPY: Self = Self(1);
    pub const CLONING_BLANK_COPY: Self = Self(2);
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct ComponentFormat(pub u32);
impl ComponentFormat {
    pub const UNKNOWN: Self = Self(0x00000000);
    pub const INT8_1: Self = Self(0x00010101);
    pub const INT8_2: Self = Self(0x00020102);
    pub const INT8_3: Self = Self(0x00030103);
    pub const INT8_4: Self = Self(0x00040104);
    pub const UINT8_1: Self = Self(0x00010105);
    pub const UINT8_2: Self = Self(0x00020106);
    pub const UINT8_3: Self = Self(0x00030107);
    pub const UINT8_4: Self = Self(0x00040108);
    pub const NORMINT8_1: Self = Self(0x00010109);
    pub const NORMINT8_2: Self = Self(0x0002010A);
    pub const NORMINT8_3: Self = Self(0x0003010B);
    pub const NORMINT8_4: Self = Self(0x0004010C);
    pub const NORMUINT8_1: Self = Self(0x0001010D);
    pub const NORMUINT8_2: Self = Self(0x0002010E);
    pub const NORMUINT8_3: Self = Self(0x0003010F);
    pub const NORMUINT8_4: Self = Self(0x00040110);
    pub const INT16_1: Self = Self(0x00010211);
    pub const INT16_2: Self = Self(0x00020212);
    pub const INT16_3: Self = Self(0x00030213);
    pub const INT16_4: Self = Self(0x00040214);
    pub const UINT16_1: Self = Self(0x00010215);
    pub const UINT16_2: Self = Self(0x00020216);
    pub const UINT16_3: Self = Self(0x00030217);
    pub const UINT16_4: Self = Self(0x00040218);
    pub const NORMINT16_1: Self = Self(0x00010219);
    pub const NORMINT16_2: Self = Self(0x0002021A);
    pub const NORMINT16_3: Self = Self(0x0003021B);
    pub const NORMINT16_4: Self = Self(0x0004021C);
    pub const NORMUINT16_1: Self = Self(0x0001021D);
    pub const NORMUINT16_2: Self = Self(0x0002021E);
    pub const NORMUINT16_3: Self = Self(0x0003021F);
    pub const NORMUINT16_4: Self = Self(0x00040220);
    pub const INT32_1: Self = Self(0x00010421);
    pub const INT32_2: Self = Self(0x00020422);
    pub const INT32_3: Self = Self(0x00030423);
    pub const INT32_4: Self = Self(0x00040424);
    pub const UINT32_1: Self = Self(0x00010425);
    pub const UINT32_2: Self = Self(0x00020426);
    pub const UINT32_3: Self = Self(0x00030427);
    pub const UINT32_4: Self = Self(0x00040428);
    pub const NORMINT32_1: Self = Self(0x00010429);
    pub const NORMINT32_2: Self = Self(0x0002042A);
    pub const NORMINT32_3: Self = Self(0x0003042B);
    pub const NORMINT32_4: Self = Self(0x0004042C);
    pub const NORMUINT32_1: Self = Self(0x0001042D);
    pub const NORMUINT32_2: Self = Self(0x0002042E);
    pub const NORMUINT32_3: Self = Self(0x0003042F);
    pub const NORMUINT32_4: Self = Self(0x00040430);
    pub const FLOAT16_1: Self = Self(0x00010231);
    pub const FLOAT16_2: Self = Self(0x00020232);
    pub const FLOAT16_3: Self = Self(0x00030233);
    pub const FLOAT16_4: Self = Self(0x00040234);
    pub const FLOAT32_1: Self = Self(0x00010435);
    pub const FLOAT32_2: Self = Self(0x00020436);
    pub const FLOAT32_3: Self = Self(0x00030437);
    pub const FLOAT32_4: Self = Self(0x00040438);
    pub const UINT_10_10_10_L1: Self = Self(0x00010439);
    pub const NORMINT_10_10_10_L1: Self = Self(0x0001043A);
    pub const NORMINT_11_11_10: Self = Self(0x0001043B);
    pub const NORMUINT8_4_BGRA: Self = Self(0x0004013C);
    pub const NORMINT_10_10_10_2: Self = Self(0x0001043D);
    pub const UINT_10_10_10_2: Self = Self(0x0001043E);
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Region {
    start_index: u32,
    num_indices: u32,
}

#[binrw]
#[brw(little)]
#[br(import_raw(args: NiObjectArgs))]
#[derive(Debug, NiObjectType)]
pub struct NiDataStream {
    #[br(calc(DataStreamUsage(args.generic_value1)))]
    usage: DataStreamUsage,

    #[br(calc(DataStreamAccess(args.generic_value2)))]
    access: DataStreamAccess,

    num_bytes: u32,
    cloning_behavior: CloningBehavior,
    num_regions: u32,

    #[br(count = num_regions)]
    regions: Vec<Region>,

    num_components: u32,

    #[br(count = num_components)]
    component_formats: Vec<ComponentFormat>,

    #[br(count = num_bytes)]
    data: Vec<u8>,

    streamable: u8,
}
