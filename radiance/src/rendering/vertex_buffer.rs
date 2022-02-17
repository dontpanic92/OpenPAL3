use alloc::{vec, vec::Vec};

use crate::math::*;

bitflags! {
    pub struct VertexComponents: u32 {
        const POSITION = 0x1;
        const NORMAL = 0x2;
        const TEXCOORD = 0x4;
        const TEXCOORD2 = 0x8;
    }
}

impl VertexComponents {
    const NUM_OF_SUPPORTED_COMPONENTS: usize = 4;

    fn get_supported_components() -> [VertexComponents; 4] {
        [
            VertexComponents::POSITION,
            VertexComponents::NORMAL,
            VertexComponents::TEXCOORD,
            VertexComponents::TEXCOORD2,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct VertexComponentsLayout {
    components: VertexComponents,
    size: usize,
    offsets: [usize; VertexComponents::NUM_OF_SUPPORTED_COMPONENTS],
}

impl VertexComponentsLayout {
    pub fn from_components(components: VertexComponents) -> VertexComponentsLayout {
        #[cfg(feature = "no_std")]
        {
            internal::from_components(components)
        }

        #[cfg(feature = "std")]
        {
            internal::from_components(components)
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn get_offset(&self, component: VertexComponents) -> Option<usize> {
        self.components
            .contains(component)
            .then_some(self.offsets[Self::component_index(component)])
    }

    fn component_index(component: VertexComponents) -> usize {
        match component {
            VertexComponents::POSITION => 0,
            VertexComponents::NORMAL => 1,
            VertexComponents::TEXCOORD => 2,
            VertexComponents::TEXCOORD2 => 3,
            _ => unreachable!(),
        }
    }

    fn component_size(component: VertexComponents) -> usize {
        match component {
            VertexComponents::POSITION => core::mem::size_of::<Vec3>(),
            VertexComponents::NORMAL => core::mem::size_of::<Vec3>(),
            VertexComponents::TEXCOORD => core::mem::size_of::<Vec2>(),
            VertexComponents::TEXCOORD2 => core::mem::size_of::<Vec2>(),
            _ => unreachable!(),
        }
    }

    fn calc_layout(components: VertexComponents) -> Self {
        let mut layout = Self {
            components,
            size: 0,
            offsets: [0usize; VertexComponents::NUM_OF_SUPPORTED_COMPONENTS],
        };

        let supported_components = VertexComponents::get_supported_components();
        for component in supported_components {
            if components.contains(component) {
                layout.offsets[Self::component_index(component)] = layout.size;
                layout.size += Self::component_size(component);
            }
        }

        layout
    }
}

#[derive(Debug, Clone)]
pub struct VertexBuffer {
    layout: VertexComponentsLayout,
    data: Vec<u8>,
    count: usize,
}

impl VertexBuffer {
    pub fn new(components: VertexComponents, count: usize) -> Self {
        let layout = VertexComponentsLayout::from_components(components);
        let size = layout.size;
        let data = vec![0u8; size * count];
        Self {
            layout: layout,
            data,
            count,
        }
    }

    pub fn new_with_data_blob(components: VertexComponents, data: Vec<u8>) -> Self {
        let layout = VertexComponentsLayout::from_components(components);
        let size = layout.size;
        let len = data.len();
        if len % size != 0 {
            panic!("Vertex len mismatch when creating vertex with data");
        }

        Self {
            layout,
            data,
            count: len / size,
        }
    }

    pub fn set_data(
        &mut self,
        index: usize,
        position: Option<&Vec3>,
        normal: Option<&Vec3>,
        tex_coord: Option<&Vec2>,
        tex_coord2: Option<&Vec2>,
    ) {
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

        if components != self.layout.components {
            panic!("Vertex component mismatch when setting vertex data");
        }

        self.set_vertex_blob(index, |v: &mut [u8]| {
            v.copy_from_slice(&data);
        });
    }

    pub fn position(&self, index: usize) -> Option<&Vec3> {
        self.get_component(index, VertexComponents::POSITION)
    }

    pub fn tex_coord(&self, index: usize) -> Option<&Vec2> {
        self.get_component(index, VertexComponents::TEXCOORD)
    }

    fn get_component<TData>(&self, index: usize, component: VertexComponents) -> Option<&TData> {
        let vertex_size = self.layout.size;
        match self.layout.get_offset(component) {
            None => None,
            Some(offset) => Some(unsafe {
                &*(self
                    .data
                    .as_ptr()
                    .offset((index * vertex_size + offset) as isize)
                    as *const TData)
            }),
        }
    }

    pub fn set_component<TData, F: Fn(&mut TData)>(
        &mut self,
        index: usize,
        component: VertexComponents,
        update: F,
    ) {
        let component_size = VertexComponentsLayout::component_size(component);
        if component_size != core::mem::size_of::<TData>() {
            panic!(
                "Wrong size when set vertex data: component size {}, TData.size {}",
                component_size,
                core::mem::size_of::<TData>()
            );
        }

        if index >= self.count {
            panic!("Index out of range: {}", index);
        }

        let offset = self.layout.get_offset(component).unwrap();
        let vertex_size = self.layout.size;
        let data: &mut TData = unsafe {
            &mut *(self
                .data
                .as_mut_ptr()
                .offset((index * vertex_size + offset) as isize) as *mut TData)
        };
        update(data);
    }

    pub fn set_vertex_blob<F: Fn(&mut [u8])>(&mut self, index: usize, update: F) {
        let vertex_size = self.layout.size;
        let data: &mut [u8] = unsafe {
            core::slice::from_raw_parts_mut(
                self.data
                    .as_mut_ptr()
                    .offset((index * vertex_size) as isize) as *mut u8,
                vertex_size,
            )
        };
        update(data);
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

#[cfg(feature = "std")]
mod internal {
    use super::{VertexComponents, VertexComponentsLayout};
    use dashmap::DashMap;

    lazy_static::lazy_static! {
        static ref LAYOUT_CACHE: DashMap<VertexComponents, VertexComponentsLayout> = DashMap::new();
    }

    pub fn from_components(components: VertexComponents) -> VertexComponentsLayout {
        LAYOUT_CACHE
            .entry(components)
            .or_insert_with(|| VertexComponentsLayout::calc_layout(components))
            .downgrade()
            .clone()
    }
}

#[cfg(feature = "no_std")]
mod internal {
    use super::{VertexComponents, VertexComponentsLayout};
    use hashbrown::HashMap;
    use spin::RwLock;

    lazy_static::lazy_static! {
        static ref LAYOUT_CACHE: RwLock<HashMap<VertexComponents, VertexComponentsLayout>> = RwLock::new(HashMap::new());
    }

    pub fn from_components(components: VertexComponents) -> VertexComponentsLayout {
        let mut cache = LAYOUT_CACHE.write();
        if let Some(layout) = cache.get(&components) {
            layout.clone()
        } else {
            let layout = VertexComponentsLayout::calc_layout(components);
            cache.insert(components, layout.clone());
            layout
        }
    }
}
