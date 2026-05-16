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
    pub mod immediate_director {
        include!(concat!(env!("OUT_DIR"), "/immediate_director_comdef.rs"));
    }
}

pub mod proxies;
pub mod runtime;
pub mod services;

pub use proxies::{
    wrap_director, wrap_im_director, ImguiImmediateDirectorPump, ScriptedImmediateDirector,
};
pub use runtime::{RuntimeServices, ScriptDirectorHandle, ScriptHost};
pub use services::HostContext;
