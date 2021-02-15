use radiance::math::{Vec2, Vec3};
use radiance::rendering::{RenderObject, SimpleMaterial, VertexBuffer, VertexComponents};
use radiance::scene::{CoreEntity, CoreScene, Entity, EntityCallbacks, SceneCallbacks};

pub struct ModelEntity {}

impl EntityCallbacks for ModelEntity {
    fn on_loading<T: EntityCallbacks>(&mut self, _entity: &mut CoreEntity<T>) {}

    fn on_updating<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>, delta_sec: f32) {
        entity
            .transform_mut()
            .rotate(&Vec3::new(0., 1., 0.), -delta_sec * std::f32::consts::PI);
    }
}

pub struct ModelViewerScene {}

impl SceneCallbacks for ModelViewerScene {
    fn on_loading<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>) {
        let mut texture_path = std::env::current_exe().unwrap();
        texture_path.pop();
        texture_path.push("data/test.jpg");

        let mut entity1 = CoreEntity::new(ModelEntity {});

        let mut vertices =
            VertexBuffer::new(VertexComponents::POSITION | VertexComponents::TEXCOORD, 4);
        vertices.set_data(
            0,
            Some(&Vec3::new(-0.5, -0.5, 0.)),
            None,
            Some(&Vec2::new(0., 1.)),
            None,
        );
        vertices.set_data(
            1,
            Some(&Vec3::new(0.5, -0.5, 0.)),
            None,
            Some(&Vec2::new(1., 1.)),
            None,
        );
        vertices.set_data(
            2,
            Some(&Vec3::new(0.5, 0.5, 0.)),
            None,
            Some(&Vec2::new(1., 0.)),
            None,
        );
        vertices.set_data(
            3,
            Some(&Vec3::new(-0.5, 0.5, 0.)),
            None,
            Some(&Vec2::new(0., 0.)),
            None,
        );

        entity1.add_component(RenderObject::new_with_data(
            vertices,
            vec![0, 1, 2, 2, 3, 0],
            Box::new(SimpleMaterial::new(&texture_path)),
        ));

        let mut vertices2 =
            VertexBuffer::new(VertexComponents::POSITION | VertexComponents::TEXCOORD, 4);
        vertices2.set_data(
            0,
            Some(&Vec3::new(-0.5, -0.5, -1.)),
            None,
            Some(&Vec2::new(0., 1.)),
            None,
        );
        vertices2.set_data(
            1,
            Some(&Vec3::new(0.5, -0.5, -1.)),
            None,
            Some(&Vec2::new(1., 1.)),
            None,
        );
        vertices2.set_data(
            2,
            Some(&Vec3::new(0.5, 0.5, -1.)),
            None,
            Some(&Vec2::new(1., 0.)),
            None,
        );
        vertices2.set_data(
            3,
            Some(&Vec3::new(-0.5, 0.5, -1.)),
            None,
            Some(&Vec2::new(0., 0.)),
            None,
        );
        let mut entity2 = CoreEntity::new(ModelEntity {});
        entity2.add_component(RenderObject::new_with_data(
            vertices2,
            vec![0, 1, 2, 2, 3, 0],
            Box::new(SimpleMaterial::new(&texture_path)),
        ));

        scene.add_entity(entity1);
        scene.add_entity(entity2);
    }
}
