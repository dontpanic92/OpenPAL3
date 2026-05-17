//! Result bundle returned by [`crate::openpal4::debug_bootstrap::install`].
//!
//! Keeps the `ScriptHost` alive alongside the wrapped overlay handles
//! so the application loader can hand the bundle (and a `Rc` clone of
//! the script host) into the `OpenPAL4Director` without re-bootstrapping.

use std::rc::Rc;

use radiance_scripting::ScriptHost;
use shared::openpal4::director::Pal4DebugBundle;

pub struct Pal4DebugBootstrap {
    pub host: Rc<ScriptHost>,
    pub bundle: Pal4DebugBundle,
}
