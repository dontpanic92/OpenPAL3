use crate::math::*;
use std::hash::Hash;
use std::cmp::Eq;
use std::collections::HashMap;

bitflags! {
    pub struct VertexComponents: u32 {
        const POSITION = 0x1;
        const NORMAL = 0x2;
        const TEXCOORD = 0x4;
        const TEXCOORD2 = 0x8;
    }
}

pub struct VertexMetadata {
    pub size: usize,
    pub offsets: HashMap<VertexComponents, usize>,
}

lazy_static!
{
    static ref METADATA_CACHE: HashMap<VertexComponents, VertexMetadata> = HashMap::new();
}

impl VertexMetadata {
    pub fn get(components: VertexComponents) -> &'static VertexMetadata {
        if !METADATA_CACHE.contains_key(&components) {
            METADATA_CACHE.insert(components, VertexMetadata::calc_metadata(components));
        }

        METADATA_CACHE.get(&components).unwrap()
    }

    fn calc_metadata(components: VertexComponents) -> VertexMetadata {
        let mut metadata = VertexMetadata {
            size: 0,
            offsets: HashMap::new(),
        };

        if components.contains(VertexComponents::POSITION) {
            metadata.offsets.insert(VertexComponents::POSITION, metadata.size);
            metadata.size += std::mem::size_of::<Vec3>();
        }

        if components.contains(VertexComponents::NORMAL) {
            metadata.offsets.insert(VertexComponents::NORMAL, metadata.size);
            metadata.size += std::mem::size_of::<Vec3>();
        }

        if components.contains(VertexComponents::TEXCOORD) {
            metadata.offsets.insert(VertexComponents::TEXCOORD, metadata.size);
            metadata.size += std::mem::size_of::<Vec2>();
        }

        if components.contains(VertexComponents::TEXCOORD2) {
            metadata.offsets.insert(VertexComponents::TEXCOORD2, metadata.size);
            metadata.size += std::mem::size_of::<Vec2>();
        }

        metadata
    }
}

#[derive(Clone)]
pub struct Vertex {
    components: VertexComponents,
    data: Vec<u8>,
}

impl Vertex {
    pub fn new(components: VertexComponents) -> Self {
        let size = VertexMetadata::get(components).size;
        let data = vec![0u8; size];
        Vertex {
            components,
            data
        }
    }

    pub fn set_data<F: Fn(&mut Vec<u8>, &usize)>(&mut self, component: VertexComponents, update: F) {
        let offset = self.metadata().offsets.get(&component).unwrap();
        update(&mut self.data, offset);
    }

    pub fn components(&self) -> VertexComponents {
        self.components
    }

    pub fn metadata(&self) -> &'static VertexMetadata {
        VertexMetadata::get(self.components)
    }
}
