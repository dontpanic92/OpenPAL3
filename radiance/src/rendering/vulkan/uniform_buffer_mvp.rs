use crate::math::Mat44;

#[repr(C)]
pub struct UniformBufferMvp {
    model: Mat44,
    view: Mat44,
    projection: Mat44,
}

impl UniformBufferMvp {
    pub fn new(model: &Mat44, view: &Mat44, projection: &Mat44) -> Self {
        Self {
            model: *model,
            view: *view,
            projection: *projection,
        }
    }
}
