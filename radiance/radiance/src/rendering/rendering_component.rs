use super::RenderObject;

pub struct RenderingComponent {
    objects: Vec<Box<dyn RenderObject>>,
}

impl RenderingComponent {
    pub fn new() -> Self {
        RenderingComponent { objects: vec![] }
    }

    pub fn push_render_object(&mut self, object: Box<dyn RenderObject>) {
        self.objects.push(object);
    }

    pub fn render_objects(&self) -> &[Box<dyn RenderObject>] {
        &self.objects
    }

    pub fn render_objects_mut(&mut self) -> &mut [Box<dyn RenderObject>] {
        &mut self.objects
    }
}
