//! Scriptable wrapper for [`RayCaster`].
//!
//! Adapts a shared `Rc<RayCaster>` into an `IRayCaster` ComRc so the
//! protosept runtime can invoke ray-cast queries via the
//! `[protosept(scriptable)]` bridge generated from `radiance.idl`.
//!
//! ABI note: the IDL returns `?float`, which uses `f32::NAN` as the
//! `None` sentinel over the crosscom C ABI (see proto_ccw's
//! `RetKind::OptionalFloat` and the dispatcher's NaN-decode arm).
//! The underlying collider math never produces NaN for a
//! non-degenerate mesh, so the sentinel is unambiguous in practice.

use std::rc::Rc;

use crosscom::ComRc;

use crate::comdef::{IRayCaster, IRayCasterImpl};
use crate::math::Vec3;
use crate::utils::ray_casting::{AARayDirection, RayCaster};

pub struct ScriptRayCaster {
    inner: Rc<RayCaster>,
}

ComObject_ScriptRayCaster!(crate::utils::ray_casting::ScriptRayCaster);

impl ScriptRayCaster {
    pub fn create(inner: Rc<RayCaster>) -> ComRc<IRayCaster> {
        ComRc::from_object(Self { inner })
    }
}

impl IRayCasterImpl for ScriptRayCaster {
    fn cast_aa_ny(&self, ox: f32, oy: f32, oz: f32) -> Option<f32> {
        self.inner
            .cast_aaray(&Vec3::new(ox, oy, oz), AARayDirection::NY)
    }
}

/// Convenience wrapper mirroring the `wrap_<i>` family from the
/// script-bridge codegen: returns a CCW handing the script a foreign
/// `IRayCaster` backed by `inner`.
pub fn wrap_ray_caster(inner: Rc<RayCaster>) -> ComRc<IRayCaster> {
    ScriptRayCaster::create(inner)
}
