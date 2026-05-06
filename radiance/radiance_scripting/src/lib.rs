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

pub mod command_router;
pub mod proxies;
pub mod runtime;
pub mod services;
pub mod ui_walker;

pub use command_router::{CommandRouter, NullCommandRouter};
pub use proxies::{ScriptedDirector, ScriptedViewContent};
pub use runtime::{RuntimeServices, ScriptRuntime};
pub use services::{CommandBus, HostContext};
pub use ui_walker::{
    walk, CommandSink, LocalCommandQueue, OwnedNode, TextureResolver, UiAdapter, UiVisitor,
    WalkContext, WalkError,
};
