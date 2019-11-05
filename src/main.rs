mod backend;

use backend::Backend;

fn main() {
    let backend = backend::vulkan::VulkanBackend::new();
}
