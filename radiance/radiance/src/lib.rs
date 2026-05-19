#![allow(unused_variables)]

#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/radiance_comdef.rs"));

    // Extension traits providing inherent-style access to concrete
    // engine structs from a `ComRc<I*>` handle. These took the place
    // of `[internal(), rust()]` accessors that previously bloated the
    // IDL. Re-exported here so existing `use radiance::comdef::*`
    // imports continue to find the formerly-IDL method names.
    pub use crate::application::IApplicationExt;
    pub use crate::components::mesh::{
        IAnimatedMeshComponentExt, IArmatureComponentExt, IHAnimBoneComponentExt,
    };
    pub use crate::scene::{IEntityExt, ISceneExt, ISceneManagerExt};
}
pub mod application;
pub mod audio;
pub mod components;
pub mod debug;
pub mod imgui;
pub mod input;
pub mod math;
pub mod perf;
pub mod radiance;
pub mod rendering;
pub mod scene;
pub mod utils;
pub mod video;

mod constants;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;
