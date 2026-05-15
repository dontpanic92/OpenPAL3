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

    #[macro_use]
    pub mod ui_host {
        include!(concat!(env!("OUT_DIR"), "/ui_host_comdef.rs"));
    }
}

pub mod proxies;
pub mod runtime;
pub mod services;

pub use proxies::ScriptedImmediateDirector;
pub use runtime::{RuntimeServices, ScriptDirectorHandle, ScriptHost};
pub use services::HostContext;
