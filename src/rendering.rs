mod engine_impl;
mod backend;

use engine_impl::EngineImpl;

pub trait Engine {
    fn render(&mut self);
}

pub fn create_engine() -> Result<Box<dyn Engine>, Box<dyn std::error::Error>>
{
    Ok(Box::new(EngineImpl::new()?))
}
