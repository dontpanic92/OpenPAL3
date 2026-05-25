//! Cross-cutting RNG service surfaced through `IHostContext.random()`.
//! Uniform integer in [0, max); returns 0 if max <= 0. Backed by
//! `rand::random::<u32>()`.

use crosscom::ComRc;

use crate::comdef::services::{IRandomService, IRandomServiceImpl};

pub struct RandomService;

ComObject_RandomService!(super::RandomService);

impl RandomService {
    pub fn create() -> ComRc<IRandomService> {
        ComRc::from_object(Self)
    }
}

impl IRandomServiceImpl for RandomService {
    fn next_int(&self, max: i32) -> i32 {
        if max <= 0 {
            return 0;
        }
        let r: u32 = rand::random();
        (r % max as u32) as i32
    }
}
