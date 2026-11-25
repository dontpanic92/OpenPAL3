use crate::openpal3::comdef::ICvdModel;
use crate::openpal3::loaders::cvd_loader::{
    CvdMaterial, CvdModelNode, CvdPositionKeyFrames, CvdRotationKeyFrames, CvdScaleKeyFrames,
    CvdVertex, cvd_load_from_file,
};
use crosscom::ComRc;
use mini_fs::{MiniFs, StoreExt};
use radiance::comdef::{IComponentImpl, IEntity, IEntityExt};
use radiance::math::{Quaternion, Transform, Vec2, Vec3};
use radiance::rendering::{
    ComponentFactory, MaterialDef, Pal3PropMaterialDef, VertexBuffer, VertexComponents,
};
use radiance::scene::CoreEntity;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

pub fn create_entity_from_cvd_model<P: AsRef<Path>>(
    component_factory: Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    visible: bool,
) -> ComRc<IEntity> {
    let entity = CoreEntity::create(name, visible);
    let mesh_component = CvdModelComponent::new(entity.clone(), component_factory.clone());
    entity.add_component(
        ICvdModel::uuid(),
        crosscom::ComRc::from_object(mesh_component),
    );

    let cvd = cvd_load_from_file(vfs, path.as_ref()).unwrap();
    for (_i, node) in cvd.models.iter().enumerate() {
        entity.attach(new_from_cvd_model_node(
            component_factory.clone(),
            vfs,
            path.as_ref(),
            node,
            visible,
        ));
    }

    entity
}

fn new_from_cvd_model_node<P: AsRef<Path>>(
    component_factory: Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    node: &CvdModelNode,
    visible: bool,
) -> ComRc<IEntity> {
    let mut scale_factor = 1.;
    let mut position_keyframes = None;
    let mut rotation_keyframes = None;
    let mut scale_keyframes = None;
    let mut meshes = vec![];
    if let Some(model) = &node.model {
        position_keyframes = model.position_keyframes.clone();
        rotation_keyframes = model.rotation_keyframes.clone();
        scale_keyframes = model.scale_keyframes.clone();

        for material in &model.mesh.materials {
            if material.triangles.is_none() {
                continue;
            }

            for v in &model.mesh.frames {
                let mesh = CvdMesh::new(v, material, load_texture(material, vfs, path.as_ref()));
                meshes.push(mesh);

                // TODO: Support multiple frames
                break;
            }
        }

        scale_factor = model.scale_factor;
    }

    let entity = CoreEntity::create("cvd_obj".to_string(), true);

    let mesh_component = CvdModelComponent {
        entity: entity.clone(),
        component_factory: component_factory.clone(),
        position_keyframes,
        rotation_keyframes,
        scale_keyframes,
        scale_factor,
        meshes,
    };

    mesh_component.setup_transform(entity.clone(), scale_factor);

    entity.add_component(
        ICvdModel::uuid(),
        crosscom::ComRc::from_object(mesh_component),
    );

    if let Some(children) = &node.children {
        for child in children {
            entity.attach(new_from_cvd_model_node(
                component_factory.clone(),
                vfs,
                path.as_ref(),
                &child,
                visible,
            ));
        }
    }

    entity
}

fn load_texture<P: AsRef<Path>>(
    material: &CvdMaterial,
    vfs: &MiniFs,
    model_path: P,
) -> MaterialDef {
    let mut model_dir = model_path.as_ref().to_owned();
    model_dir.pop();
    let dds_path = model_dir.join(PathBuf::from(&material.texture_name).with_extension("dds"));
    let texture_path = if vfs.open(&dds_path).is_ok() {
        dds_path
    } else {
        model_dir.join(&material.texture_name)
    };

    // CVD props carry no baked lightmap; like the original engine, shade them
    // with the scene's dynamic `.lgt` lights via the lit material (CVD vertices
    // ship normals). Keep the default `BlendMode::AlphaTest` cutout fallback,
    // matching the legacy renderer's "always cutout" behavior.
    Pal3PropMaterialDef::create(texture_path.to_str().unwrap(), |name| vfs.open(name).ok())
}

pub struct CvdModelComponent {
    entity: ComRc<IEntity>,
    component_factory: Rc<dyn ComponentFactory>,
    position_keyframes: Option<CvdPositionKeyFrames>,
    rotation_keyframes: Option<CvdRotationKeyFrames>,
    scale_keyframes: Option<CvdScaleKeyFrames>,
    scale_factor: f32,
    meshes: Vec<CvdMesh>,
}

ComObject_CvdModel!(super::CvdModelComponent);

impl IComponentImpl for CvdModelComponent {
    fn on_loading(&self) -> crosscom::Void {
        let mut objects = vec![];
        for mesh in &self.meshes {
            let ro = self.component_factory.create_render_object(
                mesh.vertices.clone(),
                mesh.indices.clone(),
                &mesh.material,
                false,
            );

            objects.push(ro);
        }

        let component = self.component_factory.create_rendering_component(objects);
        self.entity
            .set_rendering_component(Some(Rc::new(component)));
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {}
}

impl CvdModelComponent {
    pub fn new(entity: ComRc<IEntity>, component_factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            entity,
            component_factory: component_factory.clone(),
            position_keyframes: None,
            rotation_keyframes: None,
            scale_keyframes: None,
            scale_factor: 1.,
            meshes: vec![],
        }
    }

    pub fn setup_transform(&self, entity: ComRc<IEntity>, scale_factor: f32) {
        if let Some(p) = self
            .position_keyframes
            .as_ref()
            .and_then(|frame| frame.frames.get(0))
            .and_then(|f| Some(f.position))
        {
            entity.transform().borrow_mut().translate_local(&p);
        }

        entity.transform().borrow_mut().scale_local(&Vec3::new(
            scale_factor,
            scale_factor,
            scale_factor,
        ));

        if let Some(q) = self
            .rotation_keyframes
            .as_ref()
            .and_then(|frame| frame.frames.get(0))
            .and_then(|f| Some(f.quaternion))
        {
            entity.transform().borrow_mut().rotate_quaternion_local(&q);
        }

        if let Some(frame) = self
            .scale_keyframes
            .as_ref()
            .and_then(|frame| frame.frames.get(0))
        {
            let scale = frame.scale;
            let q2 = frame.quaternion;
            let mut q3 = q2;
            q3.inverse();

            entity
                .transform()
                .borrow_mut()
                .rotate_quaternion_local(&q2)
                .scale_local(&scale)
                .rotate_quaternion_local(&q3);
        }
    }

    fn duration(&self) -> f32 {
        [
            self.position_keyframes
                .as_ref()
                .map(|k| track_duration(&k.frames, |k| k.timestamp)),
            self.rotation_keyframes
                .as_ref()
                .map(|k| track_duration(&k.frames, |k| k.timestamp)),
            self.scale_keyframes
                .as_ref()
                .map(|k| track_duration(&k.frames, |k| k.timestamp)),
        ]
        .into_iter()
        .flatten()
        .fold(0., f32::max)
    }

    fn set_time(&self, time: f32) {
        // Linear tracks/slerp are used until the CVD spline parameters are decoded.
        let mut transform = Transform::new();
        if let Some(keys) = &self.position_keyframes {
            if let Some((a, b, t)) = sample_pair(&keys.frames, time, |k| k.timestamp) {
                transform.translate_local(&Vec3::lerp(&a.position, &b.position, t));
            }
        }
        transform.scale_local(&Vec3::new(
            self.scale_factor,
            self.scale_factor,
            self.scale_factor,
        ));
        if let Some(keys) = &self.rotation_keyframes {
            if let Some((a, b, t)) = sample_pair(&keys.frames, time, |k| k.timestamp) {
                transform.rotate_quaternion_local(&cvd_slerp(&a.quaternion, &b.quaternion, t));
            }
        }
        if let Some(keys) = &self.scale_keyframes {
            if let Some((a, b, t)) = sample_pair(&keys.frames, time, |k| k.timestamp) {
                let q = cvd_slerp(&a.quaternion, &b.quaternion, t);
                let mut inverse = q;
                inverse.inverse();
                transform
                    .rotate_quaternion_local(&q)
                    .scale_local(&Vec3::lerp(&a.scale, &b.scale, t))
                    .rotate_quaternion_local(&inverse);
            }
        }
        self.entity
            .transform()
            .borrow_mut()
            .set_matrix(*transform.matrix());
    }
}

fn cvd_slerp(a: &Quaternion, b: &Quaternion, t: f32) -> Quaternion {
    // gbQuaternion::Slerp (gbengine.dll 0x10026c40) uses a tighter linear
    // fallback than radiance's general-purpose slerp and does not normalize
    // that fallback. CVD rotation and scale tracks call it directly.
    let mut dot = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
    let mut b = *b;
    if dot < 0. {
        dot = -dot;
        b = Quaternion::new(-b.x, -b.y, -b.z, -b.w);
    }

    let (a_weight, b_weight) = if 1. - dot > 1e-5 {
        let angle = dot.acos();
        let inverse_sin = 1. / angle.sin();
        (
            ((1. - t) * angle).sin() * inverse_sin,
            (t * angle).sin() * inverse_sin,
        )
    } else {
        (1. - t, t)
    };

    Quaternion::new(
        a.x * a_weight + b.x * b_weight,
        a.y * a_weight + b.y * b_weight,
        a.z * a_weight + b.z * b_weight,
        a.w * a_weight + b.w * b_weight,
    )
}

fn sample_pair<T>(keys: &[T], time: f32, timestamp: impl Fn(&T) -> f32) -> Option<(&T, &T, f32)> {
    let first = keys.first()?;
    // gbGeomNode's original track loader subtracts each track's first timestamp.
    let time = time + timestamp(first);
    let upper = keys.partition_point(|k| timestamp(k) <= time);
    if upper == 0 {
        return Some((first, first, 0.));
    }
    let a = &keys[upper - 1];
    let Some(b) = keys.get(upper) else {
        return Some((a, a, 0.));
    };
    let span = timestamp(b) - timestamp(a);
    Some((
        a,
        b,
        if span > 0. {
            (time - timestamp(a)) / span
        } else {
            0.
        },
    ))
}

fn track_duration<T>(keys: &[T], timestamp: impl Fn(&T) -> f32) -> f32 {
    keys.first()
        .zip(keys.last())
        .map(|(first, last)| timestamp(last) - timestamp(first))
        .unwrap_or(0.)
}

pub(super) fn animation_duration(entity: &ComRc<IEntity>) -> f32 {
    let own = entity
        .get_component(ICvdModel::uuid())
        .map(|c| c.inner::<CvdModelComponent>().duration())
        .unwrap_or(0.);
    entity
        .children()
        .iter()
        .map(animation_duration)
        .fold(own, f32::max)
}

pub(super) fn set_animation_time(entity: &ComRc<IEntity>, time: f32) {
    // Keep the scene object's placement; only animate its CVD-local children.
    for child in entity.children() {
        if let Some(component) = child.get_component(ICvdModel::uuid()) {
            component.inner::<CvdModelComponent>().set_time(time);
        }
        set_animation_time(&child, time);
    }
}

pub(super) fn model_bounds(entity: &ComRc<IEntity>) -> Option<(Vec3, Vec3)> {
    let mut bounds = vec![];
    if let Some(component) = entity.get_component(ICvdModel::uuid()) {
        for mesh in &component.inner::<CvdModelComponent>().meshes {
            bounds.extend(mesh.vertices.aabb_min_max());
        }
    }
    for child in entity.children() {
        if let Some((min, max)) = model_bounds(&child) {
            bounds.push(radiance::math::transform_aabb(
                [min.x, min.y, min.z],
                [max.x, max.y, max.z],
                child.transform().borrow().matrix(),
            ));
        }
    }
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (a, b) in &bounds {
        min = Vec3::new(min.x.min(a[0]), min.y.min(a[1]), min.z.min(a[2]));
        max = Vec3::new(max.x.max(b[0]), max.y.max(b[1]), max.z.max(b[2]));
    }
    (!bounds.is_empty()).then_some((min, max))
}

#[cfg(test)]
mod animation_tests {
    use super::*;

    #[test]
    fn mechanism_tracks_normalize_time_and_clamp_at_endpoints() {
        let keys = [5., 6., 8.];
        assert_eq!(track_duration(&keys, |t| *t), 3.);
        assert_eq!(sample_pair(&keys, -1., |t| *t), Some((&5., &5., 0.)));
        assert_eq!(sample_pair(&keys, 2., |t| *t), Some((&6., &8., 0.5)));
        assert_eq!(sample_pair(&keys, 10., |t| *t), Some((&8., &8., 0.)));
        assert!(sample_pair::<f32>(&[], 0., |t| *t).is_none());
    }

    fn assert_quaternion_close(actual: Quaternion, expected: [f32; 4]) {
        let actual = [actual.x, actual.y, actual.z, actual.w];
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 2e-6, "{actual} != {expected}");
        }
    }

    #[test]
    fn cvd_slerp_matches_native_trigonometric_path() {
        let a = Quaternion::new(0.10259784, -0.20519568, 0.30779353, 0.9233805);
        let b = Quaternion::new(-0.27216554, 0.4082483, 0.5443311, 0.68041384);
        assert_quaternion_close(
            cvd_slerp(&a, &b, 0.37),
            [-0.04186401, 0.028156843, 0.43011978, 0.901361],
        );
    }

    #[test]
    fn cvd_slerp_preserves_native_linear_fallback() {
        let a = Quaternion::new(0., 0., 0., 1.);
        let b = Quaternion::new(0.004, 0., 0., 0.999992);
        let result = cvd_slerp(&a, &b, 0.25);
        assert_quaternion_close(result, [0.001, 0., 0., 0.999998]);
        assert!(result.x * result.x + result.w * result.w < 1.);

        // Native midpoint result for shipped M17/_A1.CVD scale track 2,
        // segment 5. This segment takes the linear fallback.
        let a = Quaternion::new(6.8251054e-9, -6.539721e-9, -0.0040366095, 0.99999183);
        let b = Quaternion::new(2.9920837e-8, -2.2871445e-8, -0.008368583, 0.9999649);
        assert_quaternion_close(
            cvd_slerp(&a, &b, 0.5),
            [1.8372988e-8, -1.4705595e-8, -0.0062025995, 0.99997836],
        );
    }
}

struct CvdMesh {
    material: MaterialDef,
    vertices: VertexBuffer,
    indices: Vec<u32>,
}

impl CvdMesh {
    pub fn new(
        all_vertices: &Vec<CvdVertex>,
        cvd_material: &CvdMaterial,
        material: MaterialDef,
    ) -> Self {
        let components =
            VertexComponents::POSITION | VertexComponents::NORMAL | VertexComponents::TEXCOORD;

        let mut index_map = std::collections::HashMap::new();
        let mut reversed_index = vec![];
        let mut get_new_index = |index: u16| -> u32 {
            if index_map.contains_key(&index) {
                index_map[&index]
            } else {
                let new_index = reversed_index.len() as u32;
                reversed_index.push(index as usize);
                index_map.insert(index, new_index);
                new_index
            }
        };

        let mut indices: Vec<u32> = vec![];
        for t in cvd_material.triangles.as_ref().unwrap() {
            indices.push(get_new_index(t.indices[0]));
            indices.push(get_new_index(t.indices[1]));
            indices.push(get_new_index(t.indices[2]));
        }

        let mut vertices = VertexBuffer::new(components, reversed_index.len());
        for i in 0..reversed_index.len() {
            let vert = &all_vertices[reversed_index[i]];
            vertices.set_data(
                i,
                Some(&Vec3::new(
                    vert.position.x,
                    vert.position.y,
                    vert.position.z,
                )),
                Some(&Vec3::new(vert.normal.x, vert.normal.y, vert.normal.z)),
                Some(&Vec2::new(vert.tex_coord.x, vert.tex_coord.y)),
                None,
            );
        }

        CvdMesh {
            material,
            vertices,
            indices,
        }
    }
}
