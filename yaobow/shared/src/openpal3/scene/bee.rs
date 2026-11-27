use crosscom::ComRc;
use radiance::{
    comdef::{IEntity, IEntityExt},
    math::Vec3,
    scene::CoreEntity,
};
use std::rc::Rc;

use super::RoleController;
use crate::openpal3::{asset_manager::AssetManager, comdef::IRoleController};

pub(super) struct BeeGuide {
    pub entity: ComRc<IEntity>,
    origin: Vec3,
    target: Vec3,
    elapsed: f32,
    duration: f32,
}

impl BeeGuide {
    pub fn load(assets: &Rc<AssetManager>) -> Option<Self> {
        let entity = CoreEntity::create("GUIDE_BEE".into(), false);
        let controller = RoleController::new(entity.clone(), assets.clone(), "DXA", "dxa").ok()?;
        controller.set_active(true);
        entity.add_component(IRoleController::uuid(), ComRc::from_object(controller));
        entity.set_visible(false);
        Some(Self {
            entity,
            origin: Vec3::new(0., 0., 0.),
            target: Vec3::new(0., 0., 0.),
            elapsed: 0.,
            duration: 0.,
        })
    }

    pub fn launch(&mut self, origin: Vec3, target: Vec3) {
        // Native 0x4198c0 selects scene.txt's NPC WalkSpeed for the DXA actor.
        const WALK_SPEED: f32 = 80.;
        self.origin = origin;
        self.target = target;
        self.elapsed = 0.;
        self.duration = Vec3::sub(&target, &origin).norm() / WALK_SPEED;
        self.entity.set_visible(true);
        self.place(origin);
    }

    pub fn update(&mut self, delta_sec: f32) {
        if !self.entity.visible() {
            return;
        }
        if self.elapsed >= self.duration {
            self.entity.set_visible(false);
            return;
        }
        if !delta_sec.is_finite() || delta_sec < 0. {
            return;
        }
        self.elapsed = (self.elapsed + delta_sec).min(self.duration);
        if self.elapsed == self.duration {
            self.place(self.target);
            return;
        }
        let position = Vec3::add(
            &self.origin,
            &Vec3::scalar_mul(
                self.elapsed / self.duration,
                &Vec3::sub(&self.target, &self.origin),
            ),
        );
        self.place(position);
    }

    fn place(&self, position: Vec3) {
        // The original updates the independent actor, then adds 80 to draw Y.
        let position = Vec3::new(position.x, position.y + 80., position.z);
        let facing = Vec3::new(self.target.x, position.y, self.target.z);
        let transform = self.entity.transform();
        let mut transform = transform.borrow_mut();
        transform.set_position(&position);
        if (self.target.x - position.x).hypot(self.target.z - position.z) > 0. {
            transform.look_at(&facing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bee_flies_independently_with_draw_lift_and_one_visible_endpoint_frame() {
        let mut bee = BeeGuide {
            entity: CoreEntity::create("GUIDE_BEE".into(), false),
            origin: Vec3::new(0., 0., 0.),
            target: Vec3::new(0., 0., 0.),
            elapsed: 0.,
            duration: 0.,
        };
        bee.launch(Vec3::new(10., 5., 20.), Vec3::new(170., 5., 20.));
        bee.update(0.5);
        assert_eq!(bee.entity.transform().borrow().position().x, 50.);
        assert_eq!(bee.entity.transform().borrow().position().y, 85.);
        bee.update(f32::NAN);
        bee.update(-1.);
        assert_eq!(bee.entity.transform().borrow().position().x, 50.);
        bee.update(10.);
        assert!(bee.entity.visible());
        assert_eq!(bee.entity.transform().borrow().position().x, 170.);
        bee.update(0.1);
        assert!(!bee.entity.visible());
        bee.launch(Vec3::new(30., 9., 40.), Vec3::new(30., 9., 40.));
        bee.update(0.1);
        assert!(!bee.entity.visible());
        assert!(bee.entity.transform().borrow().position().y.is_finite());
    }
}
