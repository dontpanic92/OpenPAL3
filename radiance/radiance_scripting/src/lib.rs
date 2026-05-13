#[macro_use]
pub mod comdef {
    #[macro_use]
    pub mod scripting {
        include!(concat!(env!("OUT_DIR"), "/scripting_comdef.rs"));
    }

    #[macro_use]
    pub mod services {
        include!(concat!(env!("OUT_DIR"), "/services_comdef.rs"));
    }
}

pub mod proxies;
pub mod runtime;
pub mod services;
pub mod ui_walker;

pub use proxies::ScriptedDirector;
pub use runtime::{RuntimeServices, ScriptDirectorHandle, ScriptHost};
pub use services::HostContext;
pub use ui_walker::{
    walk, CommandSink, LocalCommandQueue, OwnedNode, TextureResolver, UiAdapter, UiVisitor,
    WalkContext, WalkError,
};
