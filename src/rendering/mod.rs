use engine::Radiance;

mod engine;
mod backend;

pub trait Engine {
}

pub fn create() -> Box<dyn Engine>
{
    Box::new(Radiance::new())
}
