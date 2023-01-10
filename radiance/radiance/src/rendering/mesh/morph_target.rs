use std::rc::Rc;

use crate::rendering::{ComponentFactory, Geometry};

pub struct MorphTarget {
    pub component_factory: Rc<dyn ComponentFactory>,
    pub geometries: Vec<Geometry>,
    pub timestamp: f32,
}

impl MorphTarget {
    pub fn new(
        geometries: Vec<Geometry>,
        timestamp: f32,
        component_factory: Rc<dyn ComponentFactory>,
    ) -> Self {
        Self {
            component_factory,
            geometries,
            timestamp,
        }
    }
}
