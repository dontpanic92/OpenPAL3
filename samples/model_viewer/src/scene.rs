use radiance::scene::{Scene, CoreScene, SceneCallbacks, Entity, CoreEntity, EntityCallbacks};
use radiance::rendering::{RenderObject, Vertex};
use radiance::math::{Vec2, Vec3};

pub struct ModelEntity {}

impl EntityCallbacks for ModelEntity {
    fn on_loading<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>) {
        
    }

    fn on_updating<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>, delta_sec: f32) {
        entity.transform_mut().rotate(&Vec3::new(0., 1., 0.), -delta_sec * std::f32::consts::PI);
    }
}

pub struct ModelViewerScene {}

impl SceneCallbacks for ModelViewerScene {
    fn on_loading<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>) {
        println!("onloading");
        let mut entity1 = CoreEntity::new(ModelEntity{});
        entity1.add_component(RenderObject::new_with_data(
            vec![
                Vertex::new(
                    Vec3::new(-0.5, -0.5, 0.),
                    Vec3::new(1., 0., 0.),
                    Vec2::new(0., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, -0.5, 0.),
                    Vec3::new(0., 1., 0.),
                    Vec2::new(1., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, 0.5, 0.),
                    Vec3::new(0., 0., 1.),
                    Vec2::new(1., 0.),
                ),
                Vertex::new(
                    Vec3::new(-0.5, 0.5, 0.),
                    Vec3::new(1., 1., 1.),
                    Vec2::new(0., 0.),
                ),
            ],
            vec![0, 1, 2, 2, 3, 0],
        ));

        let mut entity2 = CoreEntity::new(ModelEntity{});
        entity2.add_component(RenderObject::new_with_data(
            vec![
                Vertex::new(
                    Vec3::new(-0.5, -0.5, -1.),
                    Vec3::new(1., 0., 0.),
                    Vec2::new(0., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, -0.5, -1.),
                    Vec3::new(0., 1., 0.),
                    Vec2::new(1., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, 0.5, -1.),
                    Vec3::new(0., 0., 1.),
                    Vec2::new(1., 0.),
                ),
                Vertex::new(
                    Vec3::new(-0.5, 0.5, -1.),
                    Vec3::new(1., 1., 1.),
                    Vec2::new(0., 0.),
                ),
            ],
            vec![0, 1, 2, 2, 3, 0],
        ));

        scene.add_entity(entity1);
        scene.add_entity(entity2);
    }
}
