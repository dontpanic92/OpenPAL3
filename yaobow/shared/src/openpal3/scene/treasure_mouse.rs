use crosscom::ComRc;
use radiance::{
    comdef::{IEntity, IEntityExt},
    math::Vec3,
    scene::CoreEntity,
};
use std::rc::Rc;

use super::RoleController;
use crate::openpal3::{asset_manager::AssetManager, comdef::IRoleController};

pub(super) struct TreasureMouse {
    pub entity: ComRc<IEntity>,
    tour: Option<CameraTour>,
}

struct CameraTour {
    origin: Vec3,
    target: Vec3,
    offset: Vec3,
    restore: Vec3,
    elapsed: f32,
    duration: f32,
}

impl TreasureMouse {
    pub fn load(assets: &Rc<AssetManager>) -> Option<Self> {
        let entity = CoreEntity::create("TREASURE_MOUSE".into(), false);
        let controller = RoleController::new(entity.clone(), assets.clone(), "DXD", "dxd").ok()?;
        controller.set_active(true);
        entity.add_component(IRoleController::uuid(), ComRc::from_object(controller));
        entity.set_visible(false);
        Some(Self { entity, tour: None })
    }

    pub fn active(&self) -> bool {
        self.tour.is_some()
    }

    pub fn launch(&mut self, eye: Vec3, leader: Vec3, treasure: Vec3, draw_position: Vec3) {
        let offset = self
            .tour
            .as_ref()
            .map_or_else(|| Vec3::sub(&eye, &leader), |tour| tour.offset);
        let target = Vec3::add(&treasure, &offset);
        // Native 0x42f09f uses distance * 0.02, with cosine easing.
        self.tour = Some(CameraTour {
            origin: eye,
            target,
            offset,
            restore: Vec3::add(&leader, &offset),
            elapsed: 0.,
            duration: Vec3::sub(&target, &eye).norm() * 0.02,
        });
        if let Some(controller) = RoleController::get_role_controller(self.entity.clone()) {
            controller.inner::<RoleController>().replay_anim();
        }
        self.entity
            .transform()
            .borrow_mut()
            .set_position(&draw_position);
        self.entity.set_visible(true);
    }

    pub fn update(&mut self, delta_sec: f32) -> Option<Vec3> {
        let tour = self.tour.as_mut()?;
        if !delta_sec.is_finite() || delta_sec < 0. {
            return Some(tour.position());
        }
        tour.elapsed = (tour.elapsed + delta_sec).min(tour.duration);
        if tour.elapsed >= tour.duration {
            // The second native camera point is an immediate restore, not a return tween.
            let restore = tour.restore;
            self.tour = None;
            self.entity.set_visible(false);
            return Some(restore);
        }
        Some(tour.position())
    }
}

impl CameraTour {
    fn position(&self) -> Vec3 {
        let fraction = if self.duration == 0. {
            1.
        } else {
            0.5 * (1. - (std::f32::consts::PI * self.elapsed / self.duration).cos())
        };
        Vec3::add(
            &self.origin,
            &Vec3::scalar_mul(fraction, &Vec3::sub(&self.target, &self.origin)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_stays_at_treasure_while_camera_eases_out_and_restores_immediately() {
        let mut mouse = TreasureMouse {
            entity: CoreEntity::create("TREASURE_MOUSE".into(), false),
            tour: None,
        };
        let eye = Vec3::new(400., 400., 400.);
        let leader = Vec3::new(0., 0., 0.);
        let treasure = Vec3::new(100., 0., 0.);
        mouse.launch(eye, leader, treasure, Vec3::new(100., 101., 0.));
        let position = mouse.update(0.5).unwrap();
        assert!((position.x - 414.64466).abs() < 0.0001);
        assert_eq!(mouse.entity.transform().borrow().position().x, 100.);
        assert_eq!(mouse.entity.transform().borrow().position().y, 101.);
        assert!(mouse.entity.visible());
        assert_eq!(mouse.update(f32::NAN).unwrap().x, position.x);
        assert_eq!(mouse.update(-1.).unwrap().x, position.x);
        assert_eq!(mouse.update(1.5).unwrap().x, eye.x);
        assert!(!mouse.entity.visible());
        assert!(!mouse.active());
        assert!(mouse.update(0.1).is_none());
        mouse.launch(eye, leader, leader, leader);
        assert_eq!(mouse.update(0.1).unwrap().x, eye.x);
        assert!(!mouse.active());
    }
}
