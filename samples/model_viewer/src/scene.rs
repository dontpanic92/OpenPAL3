use radiance::scene::{CoreScene, SceneCallbacks, Entity};
use radiance::rendering::{RenderObject, Vertex};
use radiance::math::{Vec2, Vec3};

pub struct ModelViewerScene {}

impl SceneCallbacks for ModelViewerScene {
    fn on_loading<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>) {
        println!("onloading");
        let mut entity1 = Entity::new();
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

        let mut entity2 = Entity::new();
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
