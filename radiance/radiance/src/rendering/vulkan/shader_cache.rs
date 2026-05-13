use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rendering::ShaderProgram;

use super::device::Device;
use super::shader::VulkanShader;

/// Engine-level cache of `VulkanShader` instances keyed by
/// [`ShaderProgram`]. Each program's two `vk::ShaderModule` objects are
/// created exactly once for the lifetime of the cache.
///
/// Before this cache existed every `VulkanMaterial::new` call constructed
/// a fresh `VulkanShader` (two `vkCreateShaderModule` calls). A PAL3 scene
/// with thousands of render objects would create thousands of redundant
/// shader modules over identical SPIR-V.
pub struct VulkanShaderCache {
    device: Rc<Device>,
    shaders: RefCell<HashMap<ShaderProgram, Rc<VulkanShader>>>,
}

impl VulkanShaderCache {
    pub fn new(device: Rc<Device>) -> Self {
        Self {
            device,
            shaders: RefCell::new(HashMap::new()),
        }
    }

    pub fn get_or_create(&self, program: ShaderProgram) -> Rc<VulkanShader> {
        let mut shaders = self.shaders.borrow_mut();
        if let Some(shader) = shaders.get(&program) {
            return shader.clone();
        }

        let shader = Rc::new(VulkanShader::new(program, self.device.clone()).unwrap());
        shaders.insert(program, shader.clone());
        shader
    }
}
