use crate::directors::sce_director::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandCmp<FCmp: Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> {
    var: i16,
    value: i32,
    cmp: FCmp,
}

impl<FCmp: Clone + for<'a, 'b> Fn(&'a i32, &'b i32) -> bool> SceCommand for SceCommandCmp<FCmp> {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let lhs = if self.var < 0 {
            state
                .shared_state_mut()
                .persistent_state_mut()
                .get_global(self.var)
                .unwrap_or(0)
        } else {
            state.vm_context_mut().get_local(self.var).unwrap_or(0)
        };

        let value = (self.cmp)(&lhs, &self.value);
        state.fop_state_mut().push_value(value);

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
