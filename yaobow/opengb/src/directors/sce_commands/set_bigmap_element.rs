use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

/*
 * q05 唐家堡
 * q06 德阳
 * q08 蜀山
 * q09 雷州
 * q10 神界
 * q11 蛮州
 * q12 古城镇
 * q13 酆都
 * q15 雪岭镇
 * q16 安溪
 * m23 海底城
 * m24 剑冢
 * m25 新仙界
 */

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SceCommandSetBigMapElement {
    id: i32,
    option: i32,
}

impl SceCommand for SceCommandSetBigMapElement {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        true
    }
}

impl SceCommandSetBigMapElement {
    pub fn new(id: i32, option: i32) -> Self {
        Self { id, option }
    }
}
