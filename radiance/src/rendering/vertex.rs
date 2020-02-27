use crate::math::*;
use std::cmp::Eq;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::Mutex;

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

lazy_static! {
    static ref METADATA_CACHE: Mutex<HashMap<VertexComponents, Arc<VertexMetadata>>> =
        Mutex::new(HashMap::new());
}

impl VertexMetadata {
    pub fn get(components: VertexComponents) -> Arc<Self> {
        let mut cache = METADATA_CACHE.lock().unwrap();
        if !cache.contains_key(&components) {
            cache.insert(components, Arc::new(Self::calc_metadata(components)));
        }

        Arc::clone(cache.get(&components).unwrap())
    }

    fn calc_metadata(components: VertexComponents) -> Self {
        let mut metadata = Self {
            size: 0,
            offsets: HashMap::new(),
        };

        if components.contains(VertexComponents::POSITION) {
            metadata
                .offsets
                .insert(VertexComponents::POSITION, metadata.size);
            metadata.size += std::mem::size_of::<Vec3>();
        }

        if components.contains(VertexComponents::NORMAL) {
            metadata
                .offsets
                .insert(VertexComponents::NORMAL, metadata.size);
            metadata.size += std::mem::size_of::<Vec3>();
        }

        if components.contains(VertexComponents::TEXCOORD) {
            metadata
                .offsets
                .insert(VertexComponents::TEXCOORD, metadata.size);
            metadata.size += std::mem::size_of::<Vec2>();
        }

        if components.contains(VertexComponents::TEXCOORD2) {
            metadata
                .offsets
                .insert(VertexComponents::TEXCOORD2, metadata.size);
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
        Self { components, data }
    }

    pub fn new_with_data_blob(components: VertexComponents, data: Vec<u8>) -> Self {
        let size = VertexMetadata::get(components).size;
        if size != data.len() {
            panic!("Vertex len mismatch when creating vertex with data");
        }

        Self { components, data }
    }

    pub fn new_with_data(
        position: Option<&Vec3>,
        normal: Option<&Vec3>,
        tex_coord: Option<&Vec2>,
        tex_coord2: Option<&Vec2>,
    ) -> Self {
        let mut data: Vec<u8> = vec![];
        let mut components = VertexComponents::empty();
        if let Some(p) = position {
            data.extend(p.as_slice());
            components |= VertexComponents::POSITION;
        }

        if let Some(n) = normal {
            data.extend(n.as_slice());
            components |= VertexComponents::NORMAL;
        }

        if let Some(t) = tex_coord {
            data.extend(t.as_slice());
            components |= VertexComponents::TEXCOORD;
        }

        if let Some(t) = tex_coord2 {
            data.extend(t.as_slice());
            components |= VertexComponents::TEXCOORD2;
        }

        Self::new_with_data_blob(components, data)
    }

    pub fn position(&self) -> Option<&Vec3> {
        self.get_component(VertexComponents::POSITION)
    }

    pub fn tex_coord(&self) -> Option<&Vec2> {
        self.get_component(VertexComponents::TEXCOORD)
    }

    pub fn get_component<TData>(&self, component: VertexComponents) -> Option<&TData> {
        let size = VertexMetadata::get(component).size;
        if size != std::mem::size_of::<TData>() {
            panic!("Wrong size when get vertex data");
        }

        let metadata = self.metadata();
        match metadata.offsets.get(&component) {
            None => None,
            Some(&offset) => {
                Some(unsafe { &*(self.data.as_ptr().offset(offset as isize) as *const TData) })
            }
        }
    }

    pub fn set_component<TData, F: Fn(&mut TData)>(
        &mut self,
        component: VertexComponents,
        update: F,
    ) {
        let size = VertexMetadata::get(component).size;
        if size != std::mem::size_of::<TData>() {
            panic!(
                "Wrong size when set vertex data: metadata.size {}, TData.size {}",
                size,
                std::mem::size_of::<TData>()
            );
        }

        let metadata = self.metadata();
        let offset = *metadata.offsets.get(&component).unwrap();
        let data: &mut TData =
            unsafe { &mut *(self.data.as_mut_ptr().offset(offset as isize) as *mut TData) };
        update(data);
    }

    pub fn components(&self) -> VertexComponents {
        self.components
    }

    pub fn metadata(&self) -> Arc<VertexMetadata> {
        VertexMetadata::get(self.components)
    }
}
