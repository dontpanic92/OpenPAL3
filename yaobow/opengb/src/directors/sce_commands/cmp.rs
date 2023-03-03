use crate::directors::sce_vm::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Clone)]
pub struct SceCommandCmp<FCmp: Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
    var: i16,
    value: i32,
    cmp: FCmp,
}

impl<FCmp: Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> std::fmt::Debug
    for SceCommandCmp<FCmp>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceCommandCmp")
            .field("var", &self.var)
            .field("value", &self.value)
            .finish()
    }
}

impl<FCmp: Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> SceCommand for SceCommandCmp<FCmp> {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let lhs = if self.var < 0 {
            state
                .global_state_mut()
                .persistent_state_mut()
                .get_global(self.var)
                .unwrap_or(0)
        } else {
            state.context_mut().get_local(self.var).unwrap_or(0)
        };

        let value = (self.cmp)(&lhs, &self.value);
        state.global_state_mut().fop_state_mut().push_value(value);

        true
    }
}

impl<FCmp: Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> SceCommandCmp<FCmp> {
    pub fn new(var: i16, value: i32, cmp: FCmp) -> Self {
        Self { var, value, cmp }
    }
}

#[allow(non_snake_case)]
pub mod SceCommandEq {
    use super::SceCommandCmp;

    pub fn new(
        var: i16,
        value: i32,
    ) -> SceCommandCmp<impl Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
        SceCommandCmp::new(var, value, i32::eq)
    }
}

#[allow(non_snake_case)]
pub mod SceCommandNeq {
    use super::SceCommandCmp;

    pub fn new(
        var: i16,
        value: i32,
    ) -> SceCommandCmp<impl Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
        SceCommandCmp::new(var, value, i32::ne)
    }
}

#[allow(non_snake_case)]
pub mod SceCommandLs {
    use super::SceCommandCmp;

    pub fn new(
        var: i16,
        value: i32,
    ) -> SceCommandCmp<impl Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
        SceCommandCmp::new(var, value, i32::lt)
    }
}

#[allow(non_snake_case)]
pub mod SceCommandGt {
    use super::SceCommandCmp;

    pub fn new(
        var: i16,
        value: i32,
    ) -> SceCommandCmp<impl Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
        SceCommandCmp::new(var, value, i32::gt)
    }
}

#[allow(non_snake_case)]
pub mod SceCommandLeq {
    use super::SceCommandCmp;

    pub fn new(
        var: i16,
        value: i32,
    ) -> SceCommandCmp<impl Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
        SceCommandCmp::new(var, value, i32::le)
    }
}

#[allow(non_snake_case)]
pub mod SceCommandGeq {
    use super::SceCommandCmp;

    pub fn new(
        var: i16,
        value: i32,
    ) -> SceCommandCmp<impl Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
        SceCommandCmp::new(var, value, i32::ge)
    }
}

// TODO: Fix it
#[allow(non_snake_case)]
pub mod SceCommandGeq2 {
    use super::SceCommandCmp;

    pub fn new(
        var: i16,
        var2: i16,
    ) -> SceCommandCmp<impl Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
        SceCommandCmp::new(var, 0, i32::ge)
    }
}
