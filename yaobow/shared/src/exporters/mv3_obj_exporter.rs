use super::obj_exporter::{Geometry, ObjSet, Object, Shape, TVertex, Vertex};
use fileformats::mv3::Mv3File;
use wavefront_obj::{
    mtl::{Color, Material, MtlSet},
    obj::Primitive,
};

pub fn export_mv3_to_obj(mv3_file: Option<&Mv3File>, name: &str) -> Option<(ObjSet, MtlSet)> {
    if mv3_file.is_none() {
        return None;
    }

    let mv3_file = mv3_file.unwrap();
    let mtllib_name = name.to_string() + ".mtl";
    let objects = convert_obj(mv3_file);
    let materials = convert_mtl(mv3_file);

    Some((
        ObjSet {
            material_library: Some(mtllib_name),
            objects,
        },
        MtlSet { materials },
    ))
}

fn convert_mtl(mv3_file: &Mv3File) -> Vec<Material> {
    let name = get_texture_name(&mv3_file);
    vec![Material {
        name: name.clone(),
        specular_coefficient: 0.078431,
        color_ambient: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        },
        color_diffuse: Color {
            r: 0.64,
            g: 0.64,
            b: 0.64,
        },
        color_specular: Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
        },
        color_emissive: None,
        optical_density: Some(1.0),
        alpha: 1.0,
        illumination: wavefront_obj::mtl::Illumination::AmbientDiffuseSpecular,
        uv_map: Some(name),
    }]
}

fn convert_obj(mv3_file: &Mv3File) -> Vec<Object> {
    let v = &mv3_file.models[0].frames[0].vertices;
    let i = &mv3_file.models[0].meshes[0].triangles;
    let t = &mv3_file.models[0].texcoords;

    let vertices = v
        .iter()
        .map(|v| Vertex {
            x: v.x as f64 * 0.01562,
            y: v.y as f64 * 0.01562,
            z: v.z as f64 * 0.01562,
        })
        .collect();

    let tex_vertices = t
        .iter()
        .map(|t| TVertex {
            u: t.u as f64,
            v: t.v as f64,
            w: 0.,
        })
        .collect();

    let shapes = i
        .iter()
        .map(|t| Shape {
            primitive: Primitive::Triangle(
                (
                    t.indices[0] as usize,
                    Some(t.texcoord_indices[0] as usize),
                    None,
                ),
                (
                    t.indices[1] as usize,
                    Some(t.texcoord_indices[1] as usize),
                    None,
                ),
                (
                    t.indices[2] as usize,
                    Some(t.texcoord_indices[2] as usize),
                    None,
                ),
            ),
            groups: vec![],
            smoothing_groups: vec![],
        })
        .collect();

    let geometry = vec![Geometry {
        material_name: Some(get_texture_name(&mv3_file)),
        shapes,
    }];

    vec![Object {
        name: "".to_string(),
        vertices,
        tex_vertices,
        normals: vec![],
        geometry,
    }]
}

fn get_texture_name(mv3_file: &Mv3File) -> String {
    mv3_file.textures[0].names[0]
        .to_string()
        .unwrap()
        .to_owned()
}
